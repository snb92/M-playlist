use std::sync::mpsc::{Receiver, channel, Sender};
use std::sync::Arc;
use std::thread;

use crate::playlist::Playlist;
use crate::audio_ring::AudioRingBuffer;
use crate::clock::MasterClock;
use crate::graphics::Dx11Compositor;
use crate::ndi_transmitter::NdiTransmitter;

#[derive(Clone)]
pub struct EngineCue {
    pub filepath: String,
    pub in_point_hnsecs: i64,
    pub out_point_hnsecs: i64,
    pub is_looping: bool,
    pub hold_last_frame: bool,
    pub transition_duration_hnsecs: i64,
}

pub enum EngineCommand {
    LoadCue(EngineCue),
    FireCue(u32, u32, i64, i64),
    SetAudioDevice(u32),
    Scrub(i64),
    SetNdiOutput(bool),
    SetGeometry([f32; 8]),
    Resize(u32, u32),
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
        
        let thread = thread::spawn(move || {
            let mut playlist = Playlist::new();
            let mut ndi_transmitter: Option<NdiTransmitter> = None;
            println!("M-Playlist [LOGIC]: App Logic Loop Started.");

            let frame_duration = std::time::Duration::from_nanos(16_666_666);
            let mut geometry_state: [[f32; 4]; 4] = [
                [-1.0, 1.0, 0.0, 0.0],  // top_left
                [ 1.0, 1.0, 0.0, 0.0],  // top_right
                [-1.0,-1.0, 0.0, 0.0],  // bottom_left
                [ 1.0,-1.0, 0.0, 0.0],  // bottom_right
            ];
            
            // Unified Broadcast Game Loop
            loop {
                let start_time = std::time::Instant::now();
                
                // 1. Process all pending commands
                while let Ok(command) = rx.try_recv() {
                    match command {
                        EngineCommand::LoadCue(cue) => {
                            playlist.load_cue(cue);
                        }
                        EngineCommand::FireCue(cue_index, transition_ms, in_point, out_point) => {
                            playlist.fire_cue(cue_index, transition_ms, in_point, out_point, ring_a.clone(), ring_b.clone(), blend_factor.clone(), clock.clone(), graphics.clone());
                        }
                        EngineCommand::SetAudioDevice(index) => {
                            println!("M-Playlist [LOGIC]: Changing Audio Device to index {}", index);
                            audio_engine.target_device_index.store(index, std::sync::atomic::Ordering::Relaxed);
                            audio_engine.pending_restart.store(true, std::sync::atomic::Ordering::Release);
                        }
                        EngineCommand::Scrub(target_hnsecs) => {
                            playlist.scrub(target_hnsecs);
                        }
                        EngineCommand::SetNdiOutput(enabled) => {
                            playlist.ndi_enabled = enabled;
                            if enabled {
                                if ndi_transmitter.is_none() {
                                    if let Some(transmitter) = NdiTransmitter::new() {
                                        let mut gfx_ndi = graphics.ndi_tx.lock().unwrap();
                                        *gfx_ndi = Some(transmitter.tx.clone());
                                        ndi_transmitter = Some(transmitter);
                                        println!("M-Playlist [LOGIC]: NDI Output enabled.");
                                    }
                                }
                            } else {
                                ndi_transmitter = None;
                                let mut gfx_ndi = graphics.ndi_tx.lock().unwrap();
                                *gfx_ndi = None;
                                println!("M-Playlist [LOGIC]: NDI Output disabled.");
                            }
                        }
                        EngineCommand::SetGeometry(c) => {
                            geometry_state = [
                                [c[0], c[1], 0.0, 0.0],
                                [c[2], c[3], 0.0, 0.0],
                                [c[4], c[5], 0.0, 0.0],
                                [c[6], c[7], 0.0, 0.0],
                            ];
                        }
                        EngineCommand::Resize(w, h) => {
                            if let Err(e) = graphics.resize(w, h) {
                                eprintln!("M-Playlist [RENDER ERROR]: Resize failed! {:?}", e);
                            } else {
                                println!("M-Playlist [RENDER INFO]: Resized Swapchain to {}x{}", w, h);
                            }
                        }
                        EngineCommand::Shutdown => {
                            println!("M-Playlist [LOGIC]: Shutting down Playlist.");
                            return; // Break thread
                        }
                    }
                }
                
                // 2. Unconditionally tick and render at 60Hz
                playlist.tick(&clock, &blend_factor, &graphics, &geometry_state);
                
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
