import re

with open(r'Z:\M-Playlist\m_playlist\src\playlist.rs', 'r') as f:
    content = f.read()

# We want to replace the body of fire_cue.
# Let's find pub fn fire_cue
start = content.find('pub fn fire_cue')
end = content.find('pub fn scrub', start)

if start == -1 or end == -1:
    print('Could not find fire_cue')
    exit(1)

new_fire_cue = '''pub fn fire_cue(
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
            ring_a.flush();
        } else {
            ring_b.flush();
        }

        // Clear webview on the target deck so we don't hold over old HTML
        if cue.modality != 6 {
            if is_deck_a {
                if let Ok(mut lock) = graphics.webview_srv_a.lock() { *lock = None; }
            } else {
                if let Ok(mut lock) = graphics.webview_srv_b.lock() { *lock = None; }
            }
        }

        match cue.modality {
            0 => {
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
                if let Some(flag) = self.ndi_run_flag.take() {
                    flag.store(false, std::sync::atomic::Ordering::Release);
                }
                let new_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                self.ndi_run_flag = Some(new_flag.clone());
                
                let rx_buffer = if is_deck_a { graphics.ndi_rx_a.clone() } else { graphics.ndi_rx_b.clone() };
                crate::ndi_receiver::spawn_receiver(cue.filepath.clone(), rx_buffer, new_flag);
            },
            4 => {
                println!("M-Playlist [DXGI]: Intercepted Modality 4! Ready to spawn receiver.");
                if let Some(flag) = self.dxgi_run_flag.take() {
                    flag.store(false, std::sync::atomic::Ordering::Release);
                }
                let new_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                self.dxgi_run_flag = Some(new_flag.clone());
                
                let rx_buffer = if is_deck_a { graphics.dxgi_rx_a.clone() } else { graphics.dxgi_rx_b.clone() };
                crate::desktop_capture::spawn_receiver(cue.hardware_index, rx_buffer, new_flag, graphics.device.clone());
            },
            5 => {
                println!("M-Playlist [SDI]: Intercepted Modality 5! Ready to spawn receiver.");
                if let Some(flag) = self.dxgi_run_flag.take() {
                    flag.store(false, std::sync::atomic::Ordering::Release);
                }
                let new_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                self.dxgi_run_flag = Some(new_flag.clone());
                
                let rx_buffer = if is_deck_a { graphics.sdi_rx_a.clone() } else { graphics.sdi_rx_b.clone() };
                crate::decklink_capture::spawn_receiver(cue.hardware_index, rx_buffer, new_flag);
            },
            6 => {
                println!("M-Playlist [WEBVIEW]: Intercepted Modality 6 Shared Handle!");
                let handle_val = cue.filepath.as_ptr() as usize;
                if let Err(e) = graphics.load_shared_surface(handle_val, is_deck_a) {
                    println!("M-Playlist [WEBVIEW]: Failed to open shared surface: {:?}", e);
                }
            },
            _ => {
                println!("M-Playlist [WARNING]: Unhandled Modality {}", cue.modality);
            }
        }
    }

    '''

new_content = content[:start] + new_fire_cue + content[end:]
with open(r'Z:\M-Playlist\m_playlist\src\playlist.rs', 'w') as f:
    f.write(new_content)

print('Success')
