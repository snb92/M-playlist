use std::sync::Arc;

use crate::audio_ring::AudioRingBuffer;
use crate::clock::MasterClock;
use crate::graphics::Dx11Compositor;
use crate::media_engine::MediaEngine;

pub struct Playlist {
    cues: Vec<crate::app_logic::EngineCue>,
    current_index: usize,
    deck_a: Option<MediaEngine>,
    deck_b: Option<MediaEngine>,
    pub is_deck_a_active: bool,
    pub is_transitioning: bool,
    pub transition_start_time: f64,
    pub transition_duration_hnsecs: i64,
    pub ndi_enabled: bool,
    pub pending_fire: bool,
}

impl Playlist {
    pub fn new() -> Self {
        Self {
            cues: Vec::new(),
            current_index: 0,
            deck_a: None,
            deck_b: None,
            is_deck_a_active: false,
            is_transitioning: false,
            transition_start_time: 0.0,
            transition_duration_hnsecs: 0,
            ndi_enabled: false,
            pending_fire: false,
        }
    }

    pub fn load_cue(&mut self, cue: crate::app_logic::EngineCue) {
        self.cues.push(cue);
        println!("M-Playlist [LOGIC]: Added Cue #{} - {}", self.cues.len(), self.cues.last().unwrap().filepath);
    }

    pub fn fire_cue(
        &mut self,
        cue_index: u32,
        transition_ms: u32,
        in_point_hnsecs: i64,
        out_point_hnsecs: i64,
        ring_a: Arc<AudioRingBuffer>,
        ring_b: Arc<AudioRingBuffer>,
        blend_factor: Arc<std::sync::atomic::AtomicU32>,
        clock: Arc<MasterClock>,
        graphics: Arc<Dx11Compositor>,
    ) {
        if self.cues.is_empty() { return; }

        self.current_index = (cue_index as usize) % self.cues.len();

        // UPDATE the stored cue dynamically to reflect UI adjustments!
        self.cues[self.current_index].in_point_hnsecs = in_point_hnsecs;
        self.cues[self.current_index].out_point_hnsecs = out_point_hnsecs;

        let cue = &self.cues[self.current_index];
        let has_active_deck = self.deck_a.is_some() || self.deck_b.is_some();
        let transition_hnsecs = (transition_ms as i64) * 10_000;
        
        if transition_hnsecs > 0 && has_active_deck {
            self.transition_duration_hnsecs = transition_hnsecs;
            self.pending_fire = true;
            
            if !self.is_deck_a_active {
                println!("M-Playlist [LOGIC]: Preparing Deck A (Crossfade)...");
                ring_a.flush();
                graphics.clear_deck(0);
                self.deck_a = Some(MediaEngine::new(0).unwrap());
                if let Some(deck_a) = self.deck_a.as_mut() {
                    deck_a.load_and_play(cue, ring_a.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                    deck_a.is_paused.store(false, std::sync::atomic::Ordering::Release);
                }
            } else {
                println!("M-Playlist [LOGIC]: Preparing Deck B (Crossfade)...");
                ring_b.flush();
                graphics.clear_deck(1);
                self.deck_b = Some(MediaEngine::new(1).unwrap());
                if let Some(deck_b) = self.deck_b.as_mut() {
                    deck_b.load_and_play(cue, ring_b.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                    deck_b.is_paused.store(false, std::sync::atomic::Ordering::Release);
                }
            }
        } else {
            self.transition_duration_hnsecs = 0;
            self.pending_fire = true;
            
            if !self.is_deck_a_active {
                println!("M-Playlist [LOGIC]: Preparing Deck A (Hard Cut)...");
                ring_a.flush();
                graphics.clear_deck(0);
                
                self.deck_a = Some(MediaEngine::new(0).unwrap());
                if let Some(deck_a) = self.deck_a.as_mut() {
                    deck_a.load_and_play(cue, ring_a.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                    deck_a.is_paused.store(false, std::sync::atomic::Ordering::Release);
                }
            } else {
                println!("M-Playlist [LOGIC]: Preparing Deck B (Hard Cut)...");
                ring_b.flush();
                graphics.clear_deck(1);
                
                self.deck_b = Some(MediaEngine::new(1).unwrap());
                if let Some(deck_b) = self.deck_b.as_mut() {
                    deck_b.load_and_play(cue, ring_b.clone(), clock.clone(), graphics.clone(), blend_factor.clone()).unwrap();
                    deck_b.is_paused.store(false, std::sync::atomic::Ordering::Release);
                }
            }
        }

        self.current_index = (self.current_index + 1) % self.cues.len();
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
            let incoming_ready = if self.is_deck_a_active {
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
        if self.deck_a.is_some() || self.deck_b.is_some() {
            let blend = f32::from_bits(blend_factor.load(std::sync::atomic::Ordering::Acquire));
            if let Err(e) = graphics.render_composited(blend, geometry) {
                eprintln!("M-Playlist [RENDER ERROR]: Failed to render composited: {:?}", e);
            }
        }
    }
    
    pub fn has_active_decks(&self) -> bool {
        self.deck_a.is_some() || self.deck_b.is_some()
    }
}
