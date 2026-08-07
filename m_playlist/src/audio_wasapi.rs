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

use std::sync::atomic::{AtomicBool, Ordering};

pub struct WasapiEngine {
    is_running: Arc<AtomicBool>,
    pub target_device_index: Arc<std::sync::atomic::AtomicU32>,
    pub pending_restart: Arc<AtomicBool>,
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

                        for chunk in float_buffer.chunks_exact_mut(num_channels) {
                            let blend = f32::from_bits(blend_factor.load(Ordering::Acquire));
                            let left_a = ring_a.pop().unwrap_or(0.0);
                            let left_b = ring_b.pop().unwrap_or(0.0);
                            chunk[0] = (left_a * (1.0 - blend)) + (left_b * blend);

                            if num_channels > 1 {
                                let right_a = ring_a.pop().unwrap_or(0.0);
                                let right_b = ring_b.pop().unwrap_or(0.0);
                                chunk[1] = (right_a * (1.0 - blend)) + (right_b * blend);
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
            _thread: Some(handle),
        }))
    }
}
