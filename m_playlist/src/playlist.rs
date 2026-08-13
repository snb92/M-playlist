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
    pub ndi_run_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
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
            ndi_run_flag: None,
        }
    }

    pub fn load_cue(&mut self, _cue: crate::app_logic::EngineCue) {
        // Obsoleted by Muscle Lobotomy. State maintained in C# Brain.
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
        // Map to EngineCue for MediaEngine legacy compatibility
        let engine_cue = crate::app_logic::EngineCue {
            filepath: target_cue.filepath.clone(),
            in_point_hnsecs: target_cue.in_point_hnsecs,
            out_point_hnsecs: target_cue.out_point_hnsecs,
            is_looping: target_cue.is_looping != 0,
            hold_last_frame: target_cue.hold_last_frame != 0,
            transition_duration_hnsecs: target_cue.transition_duration_hnsecs,
            modality: target_cue.modality,
        };
        let cue = &engine_cue;

        // Both WIC (1) and NDI (2) must evaluate as static/timeless to bypass WMF MediaEngine temporal logic.
        let is_static = cue.modality == 1 || cue.modality == 2;
        let transition_ms = target_cue.transition_duration_hnsecs / 10000;

        let has_active_deck = self.deck_a.is_some() || self.deck_b.is_some() || blend_factor.load(std::sync::atomic::Ordering::Acquire) != 0; // fallback just in case
        let transition_hnsecs = (transition_ms as i64) * 10_000;
        
        self.pending_fire = true;
        self.incoming_is_static = cue.modality == 1 || cue.modality == 2;

        if cue.modality == 2 {
            println!("M-Playlist [NDI]: Intercepted Modality 2! Ready to spawn receiver.");
            if let Some(flag) = self.ndi_run_flag.take() {
                flag.store(false, std::sync::atomic::Ordering::Release);
            }
            let new_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            self.ndi_run_flag = Some(new_flag.clone());
            
            let rx_buffer = if !self.is_deck_a_active { graphics.ndi_rx_a.clone() } else { graphics.ndi_rx_b.clone() };
            crate::ndi_receiver::spawn_receiver(cue.filepath.clone(), rx_buffer, new_flag);
        }

        if transition_hnsecs > 0 && has_active_deck {
            self.transition_duration_hnsecs = transition_hnsecs;
            
            if !self.is_deck_a_active {
                println!("M-Playlist [LOGIC]: Preparing Deck A (Crossfade)...");
                ring_a.flush();
                // graphics.clear_deck(0); - we don't clear, might have static image!
                if is_static {
                    self.deck_a = None;
                    if cue.modality == 1 {
                        use std::os::windows::ffi::OsStrExt;
                        let mut wpath: Vec<u16> = std::ffi::OsStr::new(&cue.filepath).encode_wide().collect();
                        wpath.push(0);
                        if let Ok(texture) = crate::wic::load_image_to_texture(&graphics.device, wpath.as_ptr()) {
                            let _ = graphics.update_deck_static_texture(0, &texture);
                        }
                    }
                } else {
                    self.deck_a = Some(MediaEngine::new(0).unwrap());
                    if let Some(deck_a) = self.deck_a.as_mut() {
                        deck_a.load_and_play(cue, ring_a.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                        deck_a.is_paused.store(false, std::sync::atomic::Ordering::Release);
                    }
                }
            } else {
                println!("M-Playlist [LOGIC]: Preparing Deck B (Crossfade)...");
                ring_b.flush();
                if is_static {
                    self.deck_b = None;
                    if cue.modality == 1 {
                        use std::os::windows::ffi::OsStrExt;
                        let mut wpath: Vec<u16> = std::ffi::OsStr::new(&cue.filepath).encode_wide().collect();
                        wpath.push(0);
                        if let Ok(texture) = crate::wic::load_image_to_texture(&graphics.device, wpath.as_ptr()) {
                            let _ = graphics.update_deck_static_texture(1, &texture);
                        }
                    }
                } else {
                    self.deck_b = Some(MediaEngine::new(1).unwrap());
                    if let Some(deck_b) = self.deck_b.as_mut() {
                        deck_b.load_and_play(cue, ring_b.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                        deck_b.is_paused.store(false, std::sync::atomic::Ordering::Release);
                    }
                }
            }
        } else {
            self.transition_duration_hnsecs = 0;
            
            if !self.is_deck_a_active {
                println!("M-Playlist [LOGIC]: Preparing Deck A (Hard Cut)...");
                ring_a.flush();
                
                if is_static {
                    self.deck_a = None;
                    if cue.modality == 1 {
                        use std::os::windows::ffi::OsStrExt;
                        let mut wpath: Vec<u16> = std::ffi::OsStr::new(&cue.filepath).encode_wide().collect();
                        wpath.push(0);
                        if let Ok(texture) = crate::wic::load_image_to_texture(&graphics.device, wpath.as_ptr()) {
                            let _ = graphics.update_deck_static_texture(0, &texture);
                        }
                    }
                } else {
                    self.deck_a = Some(MediaEngine::new(0).unwrap());
                    if let Some(deck_a) = self.deck_a.as_mut() {
                        deck_a.load_and_play(cue, ring_a.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                        deck_a.is_paused.store(false, std::sync::atomic::Ordering::Release);
                    }
                }
            } else {
                println!("M-Playlist [LOGIC]: Preparing Deck B (Hard Cut)...");
                ring_b.flush();
                
                if is_static {
                    self.deck_b = None;
                    if cue.modality == 1 {
                        use std::os::windows::ffi::OsStrExt;
                        let mut wpath: Vec<u16> = std::ffi::OsStr::new(&cue.filepath).encode_wide().collect();
                        wpath.push(0);
                        if let Ok(texture) = crate::wic::load_image_to_texture(&graphics.device, wpath.as_ptr()) {
                            let _ = graphics.update_deck_static_texture(1, &texture);
                        }
                    }
                } else {
                    self.deck_b = Some(MediaEngine::new(1).unwrap());
                    if let Some(deck_b) = self.deck_b.as_mut() {
                        deck_b.load_and_play(cue, ring_b.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                        deck_b.is_paused.store(false, std::sync::atomic::Ordering::Release);
                    }
                }
            }
        }
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

    pub fn tick(&mut self, clock: &MasterClock, blend_factor: &std::sync::atomic::AtomicU32, graphics: &Dx11Compositor, geometry: &[[f32; 4]; 4]) {
        
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
                
                if self.is_deck_a_active {
                    blend_factor.store(0.0_f32.to_bits(), std::sync::atomic::Ordering::Release);
                    self.deck_b = None; // Drop outgoing deck B
                } else {
                    blend_factor.store(1.0_f32.to_bits(), std::sync::atomic::Ordering::Release);
                    self.deck_a = None; // Drop outgoing deck A
                }
                println!("M-Playlist [LOGIC]: Transition Complete. Outgoing deck VRAM released.");
            }
        }
        
        // UNCONDITIONALLY RENDER IF ACTIVE
        let blend = f32::from_bits(blend_factor.load(std::sync::atomic::Ordering::Acquire));
        if let Err(e) = graphics.render_composited(blend, geometry) {
            eprintln!("M-Playlist [RENDER ERROR]: Failed to render composited: {:?}", e);
        }
    }
    
    pub fn has_active_decks(&self) -> bool {
        self.deck_a.is_some() || self.deck_b.is_some()
    }
}
