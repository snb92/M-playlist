use std::sync::Arc;
use std::thread;
use windows::core::{w, Result};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioClient, IAudioRenderClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED, CoUninitialize};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW};

use crate::audio_ring::AudioRingBuffer;
use crate::clock::MasterClock;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

// Storing absolute floats in Atomics via f32::to_bits()
pub static PEAK_L: AtomicU32 = AtomicU32::new(0);
pub static PEAK_R: AtomicU32 = AtomicU32::new(0);

pub struct WasapiEngine {
    is_running: Arc<AtomicBool>,
    pub target_device_index: Arc<std::sync::atomic::AtomicU32>,
    pub pending_restart: Arc<AtomicBool>,
    pub master_volume: Arc<std::sync::atomic::AtomicU32>,
    pub ndi_tx: Arc<std::sync::Mutex<Option<std::sync::mpsc::SyncSender<crate::ndi_transmitter::NdiPayload>>>>,
    pub audio_grave_rx: Arc<std::sync::Mutex<Option<std::sync::mpsc::Receiver<Vec<f32>>>>>,
    _thread: Option<thread::JoinHandle<()>>,
}

impl Drop for WasapiEngine {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Release);
        if let Some(thread) = self._thread.take() {
            let _ = thread.join();
        }
    }
}

impl WasapiEngine {
    pub fn start(ring_a: Arc<AudioRingBuffer>, ring_b: Arc<AudioRingBuffer>, clock: Arc<MasterClock>, blend_factor: Arc<std::sync::atomic::AtomicU32>) -> Result<Arc<Self>> {
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = is_running.clone();
        
        let target_device_index = Arc::new(std::sync::atomic::AtomicU32::new(u32::MAX)); // MAX = Default
        let target_device_index_clone = target_device_index.clone();
        
        let pending_restart = Arc::new(AtomicBool::new(false));
        let pending_restart_clone = pending_restart.clone();
        
        let master_volume = Arc::new(std::sync::atomic::AtomicU32::new(1.0_f32.to_bits()));
        let master_volume_clone = master_volume.clone();
        // We will pass an Arc to the thread so we can mutate it or we just clone the Arc of the whole engine?
        // Wait, WasapiEngine::start returns an Arc<Self>. The thread closure is created before we return the Arc.
        // Let's use a shared Arc<Mutex<Option<...>>> just for the thread.
        let thread_ndi_tx = Arc::new(std::sync::Mutex::new(None::<std::sync::mpsc::SyncSender<crate::ndi_transmitter::NdiPayload>>));
        let engine_ndi_tx = thread_ndi_tx.clone();
        
        let thread_audio_grave = Arc::new(std::sync::Mutex::new(None::<std::sync::mpsc::Receiver<Vec<f32>>>));
        let engine_audio_grave = thread_audio_grave.clone();

        let handle = thread::spawn(move || {
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED).expect("Failed to init COM");

                let mut task_index = 0;
                let mm_handle = AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut task_index)
                    .expect("Failed to set Pro Audio thread characteristics");

                // --- OUTER RECOVERY LOOP ---
                while is_running_clone.load(Ordering::Acquire) {
                    
                    let enumerator: IMMDeviceEnumerator = match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                        Ok(e) => e,
                        Err(_) => { thread::sleep(std::time::Duration::from_millis(100)); continue; }
                    };

                    let target_idx = target_device_index_clone.load(Ordering::Relaxed);
                    pending_restart_clone.store(false, Ordering::Relaxed);

                    let device = if target_idx == u32::MAX {
                        match enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                            Ok(d) => d,
                            Err(_) => { thread::sleep(std::time::Duration::from_millis(100)); continue; }
                        }
                    } else {
                        match enumerator.EnumAudioEndpoints(eRender, windows::Win32::Media::Audio::DEVICE_STATE_ACTIVE) {
                            Ok(collection) => match collection.Item(target_idx) {
                                Ok(d) => d,
                                Err(_) => { thread::sleep(std::time::Duration::from_millis(100)); continue; }
                            },
                            Err(_) => { thread::sleep(std::time::Duration::from_millis(100)); continue; }
                        }
                    };

                    let audio_client: IAudioClient = match device.Activate(CLSCTX_ALL, None) {
                        Ok(c) => c,
                        Err(_) => { thread::sleep(std::time::Duration::from_millis(100)); continue; }
                    };

                    let mix_format_ptr = match audio_client.GetMixFormat() {
                        Ok(ptr) => ptr,
                        Err(_) => continue,
                    };
                    let num_channels = (*mix_format_ptr).nChannels as usize;
                    
                    if audio_client.Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                        0, 0, mix_format_ptr, None
                    ).is_err() {
                        windows::Win32::System::Com::CoTaskMemFree(Some(mix_format_ptr as *const std::ffi::c_void));
                        continue;
                    }

                    let event_handle: HANDLE = CreateEventW(None, false, false, None).unwrap();
                    audio_client.SetEventHandle(event_handle).unwrap();
                    let render_client: IAudioRenderClient = audio_client.GetService().unwrap();

                    if audio_client.Start().is_err() {
                        CloseHandle(event_handle).ok();
                        windows::Win32::System::Com::CoTaskMemFree(Some(mix_format_ptr as *const std::ffi::c_void));
                        continue;
                    }

                    println!("M-Playlist [AUDIO]: Connected to Default Audio Device.");

                    // --- INNER REAL-TIME RENDER LOOP ---
                    let mut device_lost = false;
                    
                    loop {
                        if !is_running_clone.load(Ordering::Acquire) { break; }
                        if pending_restart_clone.load(Ordering::Acquire) { break; }

                        // Wait for OS signal, timeout drastically reduced to 250ms for lightning-fast detection
                        let wait_result = WaitForSingleObject(event_handle, 250);
                        if wait_result != WAIT_OBJECT_0 {
                            device_lost = true; 
                            break;
                        }

                        let padding = match audio_client.GetCurrentPadding() {
                            Ok(p) => p,
                            Err(_) => { device_lost = true; break; }
                        };
                        let buffer_frames = match audio_client.GetBufferSize() {
                            Ok(b) => b,
                            Err(_) => { device_lost = true; break; }
                        };
                        let frames_available = buffer_frames - padding;

                        if frames_available == 0 { continue; }

                        let byte_buffer = match render_client.GetBuffer(frames_available) {
                            Ok(buf) => buf,
                            Err(_) => { device_lost = true; break; }
                        };

                        let float_buffer = std::slice::from_raw_parts_mut(
                            byte_buffer as *mut f32,
                            (frames_available * num_channels as u32) as usize,
                        );

                        let is_paused = clock.is_paused.load(Ordering::Acquire);
                        let blend = f32::from_bits(blend_factor.load(Ordering::Acquire));
                        let vol = f32::from_bits(master_volume_clone.load(Ordering::Acquire));

                        let mut planar_opt = if crate::ffi::NDI_ENABLED.load(Ordering::Relaxed) {
                            let num_frames = frames_available as usize;
                            let num_floats = num_frames * 16; // STRICT 16 CHANNELS

                            let rx_lock = thread_audio_grave.lock().unwrap();
                            let mut planar = if let Some(rx) = rx_lock.as_ref() {
                                rx.try_recv().unwrap_or_else(|_| Vec::with_capacity(num_floats))
                            } else {
                                Vec::with_capacity(num_floats)
                            };
                            if planar.capacity() < num_floats { planar.reserve(num_floats - planar.capacity()); }
                            unsafe { planar.set_len(num_floats); }
                            Some(planar)
                        } else {
                            None
                        };

                        let mut local_max_l = 0.0f32;
                        let mut local_max_r = 0.0f32;

                        for (frame_idx, chunk) in float_buffer.chunks_exact_mut(num_channels).enumerate() {
                            if is_paused {
                                chunk[0] = 0.0;
                                if num_channels > 1 {
                                    chunk[1] = 0.0;
                                }
                            } else {
                                let mut deck_a_frame = [0.0f32; 16];
                                let mut deck_b_frame = [0.0f32; 16];

                                for i in 0..16 { deck_a_frame[i] = ring_a.pop().unwrap_or(0.0); }
                                for i in 0..16 { deck_b_frame[i] = ring_b.pop().unwrap_or(0.0); }

                                if let Some(planar) = planar_opt.as_mut() {
                                    let num_frames = frames_available as usize;
                                    for ch in 0..16 {
                                        planar[(ch * num_frames) + frame_idx] = deck_a_frame[ch] + deck_b_frame[ch];
                                    }
                                }

                                let offset_a = ring_a.routing_offset.load(Ordering::Relaxed) as usize * 2;
                                let offset_b = ring_b.routing_offset.load(Ordering::Relaxed) as usize * 2;

                                for i in 0..num_channels as usize {
                                    chunk[i] = 0.0;
                                }

                                let a_l = deck_a_frame[0] * (1.0 - blend) * vol;
                                let a_r = deck_a_frame[1] * (1.0 - blend) * vol;
                                let b_l = deck_b_frame[0] * blend * vol;
                                let b_r = deck_b_frame[1] * blend * vol;

                                if offset_a < num_channels as usize {
                                    chunk[offset_a] += a_l;
                                }
                                if offset_a + 1 < num_channels as usize {
                                    chunk[offset_a + 1] += a_r;
                                }

                                if offset_b < num_channels as usize {
                                    chunk[offset_b] += b_l;
                                }
                                if offset_b + 1 < num_channels as usize {
                                    chunk[offset_b + 1] += b_r;
                                }

                                if chunk[0].abs() > local_max_l { local_max_l = chunk[0].abs(); }
                                if num_channels > 1 && chunk[1].abs() > local_max_r { local_max_r = chunk[1].abs(); }
                            }
                        }

                        // Push to global atomics at the end of the buffer cycle using fetch_max
                        // (IEEE-754 positive floats maintain integer sorting order!)
                        PEAK_L.fetch_max(local_max_l.to_bits(), Ordering::Relaxed);
                        PEAK_R.fetch_max(local_max_r.to_bits(), Ordering::Relaxed);

                        if let Some(planar) = planar_opt {
                            if let Ok(tx_lock) = thread_ndi_tx.lock() {
                                if let Some(tx) = tx_lock.as_ref() {
                                    let _ = tx.try_send(crate::ndi_transmitter::NdiPayload::Audio(planar));
                                }
                            }
                        }

                        if render_client.ReleaseBuffer(frames_available, 0).is_err() {
                            device_lost = true; break;
                        }

                        clock.add_frames(frames_available as u64);
                    }

                    // --- CLEANUP BEFORE RESTARTING OUTER LOOP ---
                    audio_client.Stop().ok();
                    CloseHandle(event_handle).ok();
                    windows::Win32::System::Com::CoTaskMemFree(Some(mix_format_ptr as *const std::ffi::c_void));

                    if device_lost {
                        println!("M-Playlist [AUDIO]: Device Lost! Auto-recovering...");
                        // Sleep briefly before aggressively polling the OS for the new device
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                } // End Outer Loop

                AvRevertMmThreadCharacteristics(mm_handle).ok();
                CoUninitialize();
            }
        });

        Ok(Arc::new(Self {
            is_running,
            target_device_index,
            pending_restart,
            master_volume,
            ndi_tx: engine_ndi_tx,
            audio_grave_rx: engine_audio_grave,
            _thread: Some(handle),
        }))
    }
}
