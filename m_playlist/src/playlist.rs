use std::sync::Arc;

use crate::audio_ring::AudioRingBuffer;
use crate::clock::MasterClock;
use crate::graphics::Dx11Compositor;
use crate::media_engine::MediaEngine;

pub struct Playlist {
    deck_a: Option<MediaEngine>,
    deck_b: Option<MediaEngine>,
    pub is_deck_a_active: bool,
    pub is_transitioning: bool,
    pub transition_start_time: f64,
    pub transition_duration_hnsecs: i64,
    pub ndi_enabled: bool,
    pub pending_fire: bool,
    pub incoming_is_static: bool,
    pub bg_worker_a: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub bg_worker_b: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Playlist {
    pub fn new() -> Self {
        Self {
            deck_a: None,
            deck_b: None,
            is_deck_a_active: false,
            is_transitioning: false,
            transition_start_time: 0.0,
            transition_duration_hnsecs: 0,
            ndi_enabled: false,
            pending_fire: false,
            incoming_is_static: false,
            bg_worker_a: None,
            bg_worker_b: None,
        }
    }

    pub fn load_cue(&mut self, target_cue: &crate::app_logic::OwnedCue, graphics_arc: &std::sync::Arc<crate::graphics::Dx11Compositor>) {
        let standby_deck = if self.is_deck_a_active { 1 } else { 0 };
        
        if target_cue.modality == 2 || target_cue.modality == 3 || target_cue.modality == 4 {
            // NDI/LocalCamera/DXGI: Do nothing during preload. The thread spins up natively inside fire_cue.
            return;
        } else if target_cue.modality == 1 {
            // WIC Image: Decode natively and blast into the standby VRAM immediately
            use std::os::windows::ffi::OsStrExt;
            let mut wpath: Vec<u16> = std::ffi::OsStr::new(&target_cue.filepath).encode_wide().collect();
            wpath.push(0);
            if let Ok(texture) = crate::wic::load_image_to_texture(&graphics_arc.device, wpath.as_ptr()) {
                let _ = graphics_arc.update_deck_static_texture(standby_deck, &texture);
            }
        }
    }

    pub fn fire_cue(
        &mut self,
        target_cue: &crate::app_logic::OwnedCue,
        _tx: &std::sync::mpsc::Sender<crate::app_logic::EngineCommand>,
        ring_a: Arc<AudioRingBuffer>,
        ring_b: Arc<AudioRingBuffer>,
        blend_factor: Arc<std::sync::atomic::AtomicU32>,
        clock: Arc<MasterClock>,
        graphics: Arc<Dx11Compositor>,
    ) {
        let engine_cue = crate::app_logic::EngineCue {
            filepath: target_cue.filepath.clone(),
            in_point_hnsecs: target_cue.in_point_hnsecs,
            out_point_hnsecs: target_cue.out_point_hnsecs,
            is_looping: target_cue.is_looping != 0,
            hold_last_frame: target_cue.hold_last_frame != 0,
            transition_duration_hnsecs: target_cue.transition_duration_hnsecs,
            modality: target_cue.modality,
            hardware_index: target_cue.hardware_index,
            audio_routing: target_cue.audio_routing,
        };
        let cue = &engine_cue;

        let is_static = cue.modality == 1 || cue.modality == 2 || cue.modality == 4 || cue.modality == 6;
        let transition_ms = target_cue.transition_duration_hnsecs / 10000;

        let has_active_deck = self.deck_a.is_some() || self.deck_b.is_some() || blend_factor.load(std::sync::atomic::Ordering::Acquire) != 0; 
        let transition_hnsecs = (transition_ms as i64) * 10_000;
        
        self.pending_fire = true;
        self.incoming_is_static = cue.modality != 0;

        if transition_hnsecs > 0 && has_active_deck {
            self.transition_duration_hnsecs = transition_hnsecs;
        } else {
            self.transition_duration_hnsecs = 0;
        }

        let is_deck_a = !self.is_deck_a_active;

        if is_deck_a {
            graphics.hardware_flush_a.store(true, std::sync::atomic::Ordering::Release);
            graphics.modality_a.store(cue.modality, std::sync::atomic::Ordering::Release);
            if let Some(flag) = self.bg_worker_a.take() { flag.store(false, std::sync::atomic::Ordering::Release); }
        } else {
            graphics.hardware_flush_b.store(true, std::sync::atomic::Ordering::Release);
            graphics.modality_b.store(cue.modality, std::sync::atomic::Ordering::Release);
            if let Some(flag) = self.bg_worker_b.take() { flag.store(false, std::sync::atomic::Ordering::Release); }
        }

        match cue.modality {
            0 => {
                println!("M-Playlist [WMF]: Intercepted Modality 0! Firing MediaEngine.");
                if is_deck_a {
                    self.deck_a = Some(MediaEngine::new(0).unwrap());
                    if let Some(deck_a) = self.deck_a.as_mut() {
                        deck_a.load_and_play(cue, ring_a.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                        deck_a.is_paused.store(false, std::sync::atomic::Ordering::Release);
                    }
                } else {
                    self.deck_b = Some(MediaEngine::new(1).unwrap());
                    if let Some(deck_b) = self.deck_b.as_mut() {
                        deck_b.load_and_play(cue, ring_b.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                        deck_b.is_paused.store(false, std::sync::atomic::Ordering::Release);
                    }
                }
            },
            1 => {
                use std::os::windows::ffi::OsStrExt;
                let mut wpath: Vec<u16> = std::ffi::OsStr::new(&cue.filepath).encode_wide().collect();
                wpath.push(0);
                if is_deck_a {
                    self.deck_a = None;
                    if let Ok(texture) = crate::wic::load_image_to_texture(&graphics.device, wpath.as_ptr()) {
                        let _ = graphics.update_deck_static_texture(0, &texture);
                    }
                } else {
                    self.deck_b = None;
                    if let Ok(texture) = crate::wic::load_image_to_texture(&graphics.device, wpath.as_ptr()) {
                        let _ = graphics.update_deck_static_texture(1, &texture);
                    }
                }
            },
            2 => {
                println!("M-Playlist [NDI]: Intercepted Modality 2! Ready to spawn receiver.");
                let new_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                if is_deck_a {
                    self.bg_worker_a = Some(new_flag.clone());
                } else {
                    self.bg_worker_b = Some(new_flag.clone());
                }
                
                let rx_buffer = if is_deck_a { graphics.ndi_rx_a.clone() } else { graphics.ndi_rx_b.clone() };
                crate::ndi_receiver::spawn_receiver(cue.filepath.clone(), rx_buffer, new_flag);
            },
            3 => {
                // Modality 3: Local Camera (WMF Hardware Capture)
                if is_deck_a {
                    self.deck_a = Some(MediaEngine::new(0).unwrap());
                    if let Some(deck_a) = self.deck_a.as_mut() {
                        deck_a.load_and_play(cue, ring_a.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                        deck_a.is_paused.store(false, std::sync::atomic::Ordering::Release);
                    }
                } else {
                    self.deck_b = Some(MediaEngine::new(1).unwrap());
                    if let Some(deck_b) = self.deck_b.as_mut() {
                        deck_b.load_and_play(cue, ring_b.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                        deck_b.is_paused.store(false, std::sync::atomic::Ordering::Release);
                    }
                }
            },
            4 => {
                println!("M-Playlist [DXGI]: Intercepted Modality 4! Ready to spawn receiver.");
                let new_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                if is_deck_a {
                    self.bg_worker_a = Some(new_flag.clone());
                } else {
                    self.bg_worker_b = Some(new_flag.clone());
                }
                
                let rx_buffer = if is_deck_a { graphics.dxgi_rx_a.clone() } else { graphics.dxgi_rx_b.clone() };
                crate::desktop_capture::spawn_receiver(cue.hardware_index, rx_buffer, new_flag, graphics.device.clone());
            },
            5 => {
                println!("M-Playlist [SDI]: Intercepted Modality 5! Ready to spawn receiver.");
                let new_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                if is_deck_a {
                    self.bg_worker_a = Some(new_flag.clone());
                } else {
                    self.bg_worker_b = Some(new_flag.clone());
                }
                
                let rx_buffer = if is_deck_a { graphics.sdi_rx_a.clone() } else { graphics.sdi_rx_b.clone() };
                crate::decklink_capture::spawn_receiver(cue.hardware_index, rx_buffer, new_flag);
            },
            6 => {
                println!("M-Playlist [WEBVIEW]: Intercepted Modality 6! Spawning WGC receiver.");
                let new_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                if is_deck_a {
                    self.bg_worker_a = Some(new_flag.clone());
                } else {
                    self.bg_worker_b = Some(new_flag.clone());
                }
                
                // Read the FFI FilePath as the Win32 HWND integer
                let hwnd_val = cue.filepath.as_ptr() as usize;
                let rx_buffer = if is_deck_a { graphics.webview_rx_a.clone() } else { graphics.webview_rx_b.clone() };
                crate::webview_capture::spawn_receiver(hwnd_val, rx_buffer, new_flag, graphics.device.clone());
            },
            _ => {
                println!("M-Playlist [WARNING]: Unhandled Modality {}", cue.modality);
            }
        }
        
        // 🚨 CRITICAL: Physically advance the A/B deck state machine
        self.is_deck_a_active = is_deck_a;
    }

    pub fn scrub(&self, target_hnsecs: i64) {
        let active_engine = if self.is_deck_a_active {
            self.deck_a.as_ref()
        } else {
            self.deck_b.as_ref()
        };
        if let Some(engine) = active_engine {
            engine.pending_scrub.store(target_hnsecs, std::sync::atomic::Ordering::SeqCst);
        }
    }

    pub fn stop(&mut self) {
        self.deck_a = None;
        self.deck_b = None;
        self.is_transitioning = false;
        self.pending_fire = false;
        println!("M-Playlist [LOGIC]: Playlist Stopped. Decks released.");
    }

    pub fn tick(&mut self, clock: &MasterClock, blend_factor: &std::sync::atomic::AtomicU32, graphics: &Dx11Compositor, geometry: &[[f32; 4]; 4], crop: &[f32; 4], pan_zoom: &[f32; 3], color: &[f32; 3]) {
        
        if self.pending_fire {
            let incoming_ready = if self.incoming_is_static {
                true
            } else if self.is_deck_a_active {
                self.deck_b.as_ref().map_or(false, |d| d.has_started.load(std::sync::atomic::Ordering::Acquire))
            } else {
                self.deck_a.as_ref().map_or(false, |d| d.has_started.load(std::sync::atomic::Ordering::Acquire))
            };

            if incoming_ready {
                self.pending_fire = false;
                self.is_deck_a_active = !self.is_deck_a_active;
                
                if self.transition_duration_hnsecs > 0 {
                    self.is_transitioning = true;
                    self.transition_start_time = clock.get_time_seconds();
                } else {
                    // Hard cut - instantly drop outgoing deck
                    self.is_transitioning = false;
                    if self.is_deck_a_active {
                        blend_factor.store(0.0_f32.to_bits(), std::sync::atomic::Ordering::Release);
                        self.deck_b = None; 
                    } else {
                        blend_factor.store(1.0_f32.to_bits(), std::sync::atomic::Ordering::Release);
                        self.deck_a = None;
                    }
                }
            }
        }

        if self.is_transitioning {
            let elapsed = clock.get_time_seconds() - self.transition_start_time;
            let duration = self.transition_duration_hnsecs as f64 / 10_000_000.0;
            let mut progress = (elapsed / duration) as f32;
            
            if progress > 1.0 { progress = 1.0; }
            if progress < 0.0 { progress = 0.0; }

            // Ping-Pong Logic: is_deck_a_active reflects the deck we transitioned into
            let actual_blend = if self.is_deck_a_active {
                1.0 - progress 
            } else {
                progress
            };
            
            blend_factor.store(actual_blend.to_bits(), std::sync::atomic::Ordering::Release);

            if progress >= 1.0 {
                self.is_transitioning = false;
                
                if self.transition_duration_hnsecs > 0 {
                    println!("M-Playlist [LOGIC]: Transition Complete. Outgoing deck VRAM released.");
                    if self.is_deck_a_active {
                        blend_factor.store(0.0_f32.to_bits(), std::sync::atomic::Ordering::Release);
                        self.deck_b = None;
                        graphics.hardware_flush_b.store(true, std::sync::atomic::Ordering::Release);
                        if let Some(flag) = self.bg_worker_b.take() { flag.store(false, std::sync::atomic::Ordering::Release); }
                    } else {
                        blend_factor.store(1.0_f32.to_bits(), std::sync::atomic::Ordering::Release);
                        self.deck_a = None;
                        graphics.hardware_flush_a.store(true, std::sync::atomic::Ordering::Release);
                        if let Some(flag) = self.bg_worker_a.take() { flag.store(false, std::sync::atomic::Ordering::Release); }
                    }
                    self.transition_duration_hnsecs = 0; // 🚨 STATE SEAL: Prevents infinite loop
                }
            }
        }
        
        // UNCONDITIONALLY RENDER IF ACTIVE
        let blend = f32::from_bits(blend_factor.load(std::sync::atomic::Ordering::Acquire));
        if let Err(e) = graphics.render_composited(blend, geometry, crop, pan_zoom, color) {
            eprintln!("M-Playlist [RENDER ERROR]: Failed to render composited: {:?}", e);
        }
    }
    
    pub fn has_active_decks(&self) -> bool {
        self.deck_a.is_some() || self.deck_b.is_some()
    }
}


