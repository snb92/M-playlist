use std::sync::mpsc::{Receiver, channel, Sender};
use std::sync::Arc;
use std::thread;

use crate::playlist::Playlist;
use crate::audio_ring::AudioRingBuffer;
use crate::clock::MasterClock;
use crate::graphics::Dx11Compositor;

#[derive(Clone)]
pub struct EngineCue {
    pub filepath: String,
    pub in_point_hnsecs: i64,
    pub out_point_hnsecs: i64,
    pub is_looping: bool,
    pub hold_last_frame: bool,
    pub audio_routing: u8,
    pub transition_duration_hnsecs: i64,
    pub modality: u8,
    pub hardware_index: u8,
}
#[derive(Clone, Debug)]
pub struct OwnedCue {
    pub filepath: String,
    pub in_point_hnsecs: i64,
    pub out_point_hnsecs: i64,
    pub is_looping: u8,
    pub hold_last_frame: u8,
    pub audio_routing: u8,
    pub transition_duration_hnsecs: i64,
    pub modality: u8,
    pub hardware_index: u8,
}

pub enum EngineCommand {
    LoadCue(OwnedCue),
    FireCue(OwnedCue),
    SetAudioDevice(u32),
    Scrub(i64),
    SetGeometry([f32; 8]),
    SetCrop { left: f32, top: f32, right: f32, bottom: f32 },
    SetPanZoom { pan_x: f32, pan_y: f32, zoom: f32 },
    SetColor { brightness: f32, contrast: f32, saturation: f32 },
    Resize(u32, u32),
    SetVolume(f32),
    SetBlend(f32),
    UpdateSubtitle(String),
    Pause,
    Resume,
    Stop,
    SetAudioRoute { deck_id: i32, in_ch: i32, out_bus: i32, gain_db: f32 },
    Shutdown,
}

pub struct AppLogic {
    pub tx: Sender<EngineCommand>,
    _thread: thread::JoinHandle<()>,
}

impl AppLogic {
    pub fn start(
        ring_a: Arc<AudioRingBuffer>,
        ring_b: Arc<AudioRingBuffer>,
        blend_factor: Arc<std::sync::atomic::AtomicU32>,
        clock: Arc<MasterClock>,
        graphics: Arc<Dx11Compositor>,
        audio_engine: Arc<crate::audio_wasapi::WasapiEngine>,
    ) -> Self {
        let (tx, rx): (Sender<EngineCommand>, Receiver<EngineCommand>) = channel();
        let tx_clone = tx.clone();
        
        let thread = thread::spawn(move || {
            unsafe {
                let _ = windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_MULTITHREADED,
                );
            }
            let mut playlist = Playlist::new();
            println!("M-Playlist [LOGIC]: App Logic Loop Started.");

            let frame_duration = std::time::Duration::from_nanos(16_666_666);
            let mut geometry_state: [[f32; 4]; 4] = [
                [-1.0, 1.0, 0.0, 0.0],  // top_left
                [ 1.0, 1.0, 0.0, 0.0],  // top_right
                [-1.0,-1.0, 0.0, 0.0],  // bottom_left
                [ 1.0,-1.0, 0.0, 0.0],  // bottom_right
            ];
            let mut crop_state = [0.0, 0.0, 0.0, 0.0];
            let mut pan_zoom_state = [0.0, 0.0, 1.0];
            let mut color_state = [1.0, 1.0, 1.0];
            
            // STRIKE 1 & 3: Load NDI dynamically and spin up Sender thread
            match crate::ndi_ffi::NdiLibrary::load() {
                Ok(ndi) => unsafe {
                    println!("M-Playlist [NDI]: NdiLibrary::load() SUCCESS");
                    if (ndi.NDIlib_initialize)() {
                        println!("M-Playlist [NDI]: Processing.NDI.Lib.x64.dll initialized successfully!");
                        
                        let create_desc = crate::ndi_ffi::NDIlib_send_create_t {
                            p_ndi_name: b"M-Playlist Native\0".as_ptr() as *const i8,
                            p_groups: b"MySandbox\0".as_ptr() as *const i8, // ISOLATED private group
                            clock_video: false,
                            clock_audio: false,
                        };
                        let instance = (ndi.NDIlib_send_create)(&create_desc);
                        
                        if instance.ptr.is_null() {
                            println!("M-Playlist [NDI]: FATAL ERROR - NDIlib_send_create returned NULL pointer!");
                        } else {
                            println!("M-Playlist [NDI]: NDI Sender created successfully!");
                        }
                        
                        let (ndi_tx, ndi_rx) = std::sync::mpsc::sync_channel::<crate::ndi_transmitter::NdiPayload>(16);
                        
                        let (video_grave_tx, video_grave_rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(16);
                        let (audio_grave_tx, audio_grave_rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(128);
                        
                        let mut gfx_ndi = graphics.ndi_tx.lock().unwrap();
                        *gfx_ndi = Some(ndi_tx.clone());
                        
                        let mut gfx_grave = graphics.video_grave_rx.lock().unwrap();
                        *gfx_grave = Some(video_grave_rx);
                        
                        let mut aud_ndi = audio_engine.ndi_tx.lock().unwrap();
                        *aud_ndi = Some(ndi_tx);
                        
                        let mut aud_grave = audio_engine.audio_grave_rx.lock().unwrap();
                        *aud_grave = Some(audio_grave_rx);
                        
                        std::thread::spawn(move || {
                            println!("M-Playlist [NDI]: Worker thread started.");
                            while let Ok(payload) = ndi_rx.recv() {
                                match payload {
                                    crate::ndi_transmitter::NdiPayload::Video(mut frame) => {
                                        let video_data = crate::ndi_ffi::NDIlib_video_frame_v2_t {
                                            xres: frame.width,
                                            yres: frame.height,
                                            FourCC: crate::ndi_ffi::NDIlib_FourCC_video_type_BGRA,
                                            frame_rate_N: 60000,
                                            frame_rate_D: 1000,
                                            picture_aspect_ratio: frame.width as f32 / frame.height as f32,
                                            frame_format_type: 1, // progressive
                                            timecode: 0,
                                            p_data: frame.data.as_ptr() as *mut u8,
                                            line_stride_in_bytes: frame.stride,
                                            p_metadata: std::ptr::null(),
                                            timestamp: 0,
                                        };
                                        (ndi.NDIlib_send_send_video_v2)(instance, &video_data);
                                        
                                        // RECYCLE
                                        frame.data.clear();
                                        let _ = video_grave_tx.try_send(frame.data);
                                    }
                                    crate::ndi_transmitter::NdiPayload::Audio(mut data) => {
                                        let no_channels = 16;
                                        let no_samples = (data.len() / no_channels) as i32;
                                        
                                        let audio_data = crate::ndi_ffi::NDIlib_audio_frame_v2_t {
                                            sample_rate: 48000,
                                            no_channels: no_channels as i32, // STRICT 16-CHANNEL MATRIX OUTPUT
                                            no_samples,
                                            timecode: 0,
                                            p_data: data.as_mut_ptr(),
                                            channel_stride_in_bytes: no_samples * 4, // 4 bytes per float
                                            p_metadata: std::ptr::null(),
                                            timestamp: 0,
                                        };
                                        (ndi.NDIlib_send_send_audio_v2)(instance, &audio_data);
                                        
                                        // RECYCLE
                                        data.clear();
                                        let _ = audio_grave_tx.try_send(data);
                                    }
                                }
                            }
                        });
                    } else {
                        println!("M-Playlist [NDI]: FATAL ERROR - NDIlib_initialize() returned FALSE!");
                    }
                },
                Err(e) => {
                    eprintln!("M-Playlist [NDI]: Failed to load NDI library: {}", e);
                }
            }

            // Unified Broadcast Game Loop
            loop {
                let start_time = std::time::Instant::now();
                
                // 1. Process all pending commands
                while let Ok(command) = rx.try_recv() {
                    match command {
                        EngineCommand::LoadCue(cue) => {
                            playlist.load_cue(&cue, &graphics);
                        }
                        EngineCommand::FireCue(owned_cue) => {
                            playlist.fire_cue(&owned_cue, &tx_clone, ring_a.clone(), ring_b.clone(), blend_factor.clone(), clock.clone(), graphics.clone());
                        }
                        EngineCommand::SetAudioDevice(index) => {
                            println!("M-Playlist [LOGIC]: Changing Audio Device to index {}", index);
                            audio_engine.target_device_index.store(index, std::sync::atomic::Ordering::Relaxed);
                            audio_engine.pending_restart.store(true, std::sync::atomic::Ordering::Release);
                        }
                        EngineCommand::Scrub(target_hnsecs) => {
                            playlist.scrub(target_hnsecs);
                            ring_a.flush();
                            ring_b.flush();
                        }
                        EngineCommand::SetGeometry(c) => {
                            geometry_state = [
                                [c[0], c[1], 0.0, 0.0],
                                [c[2], c[3], 0.0, 0.0],
                                [c[4], c[5], 0.0, 0.0],
                                [c[6], c[7], 0.0, 0.0],
                            ];
                        }
                        EngineCommand::SetCrop { left, top, right, bottom } => {
                            crop_state = [left, top, right, bottom];
                        }
                        EngineCommand::SetPanZoom { pan_x, pan_y, zoom } => {
                            pan_zoom_state = [pan_x, pan_y, zoom];
                        }
                        EngineCommand::SetColor { brightness, contrast, saturation } => {
                            color_state = [brightness, contrast, saturation];
                        }
                        EngineCommand::Resize(w, h) => {
                            if let Err(e) = graphics.resize(w, h) {
                                eprintln!("M-Playlist [RENDER ERROR]: Resize failed! {:?}", e);
                            } else {
                                println!("M-Playlist [RENDER INFO]: Resized Swapchain to {}x{}", w, h);
                            }
                        }
                        EngineCommand::UpdateSubtitle(_s) => {
                              graphics.subtitle_dirty.store(true, std::sync::atomic::Ordering::Release);
                          }
                          EngineCommand::Pause => {
                            clock.is_paused.store(true, std::sync::atomic::Ordering::Release);
                            println!("M-Playlist [LOGIC]: Master Clock Paused.");
                        }
                        EngineCommand::Resume => {
                            clock.is_paused.store(false, std::sync::atomic::Ordering::Release);
                            println!("M-Playlist [LOGIC]: Master Clock Resumed.");
                        }
                        EngineCommand::Stop => {
                            playlist.stop();
                            ring_a.flush();
                            ring_b.flush();
                        }
                        EngineCommand::SetBlend(val) => {
                            // 🚨 STATE SEAL: Ignore manual UI/MIDI crossfader spam if an auto-transition is active
                            if playlist.transition_duration_hnsecs == 0 {
                                let u32_val = val.to_bits(); 
                                blend_factor.store(u32_val, std::sync::atomic::Ordering::Release);
                            }
                        }
                        EngineCommand::SetVolume(amp) => {
                            audio_engine.master_volume.store(amp.to_bits(), std::sync::atomic::Ordering::Relaxed);
                        }
                        EngineCommand::SetAudioRoute { deck_id, in_ch, out_bus, gain_db } => {
                            let deck_ring = if deck_id == 0 { &ring_a } else { &ring_b };
                            deck_ring.set_route(in_ch as usize, out_bus as usize, gain_db);
                        }
                        EngineCommand::Shutdown => {
                            println!("M-Playlist [LOGIC]: Shutting down Playlist.");
                            return; // Break thread
                        }
                    }
                }
                
                // 2. Unconditionally tick and render at 60Hz
                playlist.tick(&clock, &blend_factor, &graphics, &geometry_state, &crop_state, &pan_zoom_state, &color_state);
                
                // 3. Sleep for the remainder of the 16.6ms window
                let elapsed = start_time.elapsed();
                if elapsed < frame_duration {
                    std::thread::sleep(frame_duration - elapsed);
                }
            }
        });

        Self { tx, _thread: thread }
    }
}

impl Drop for AppLogic {
    fn drop(&mut self) {
        let _ = self.tx.send(EngineCommand::Shutdown);
    }
}
