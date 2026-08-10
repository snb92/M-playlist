use std::sync::{Arc, atomic::{AtomicBool, AtomicI64, Ordering}};
use std::thread;
use std::time::Duration;
use windows::core::{ComInterface, Result, PCWSTR};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED, CoUninitialize};
use windows::Win32::Media::MediaFoundation::{
    MFCreateSourceReaderFromURL, IMFSourceReader, IMFDXGIDeviceManager, MFCreateDXGIDeviceManager,
    MFCreateAttributes, IMFAttributes, MF_SOURCE_READER_D3D_MANAGER, MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
    MFCreateMediaType, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MFMediaType_Audio, MFAudioFormat_Float, MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF_SOURCE_READERF_ENDOFSTREAM, IMFDXGIBuffer, MFVideoFormat_ARGB32, MFMediaType_Video,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND
};


use crate::audio_ring::AudioRingBuffer;
use crate::clock::MasterClock;
use crate::graphics::Dx11Compositor;

pub struct MediaEngine {
    is_playing: Arc<AtomicBool>,
    pub is_paused: Arc<AtomicBool>,
    pub pending_scrub: Arc<AtomicI64>,
    pub has_started: Arc<AtomicBool>,
    deck_id: u8,
    _decoder_thread: Option<thread::JoinHandle<()>>,
}

impl MediaEngine {
    pub fn new(deck_id: u8) -> Result<Self> {
        Ok(Self {
            is_playing: Arc::new(AtomicBool::new(false)),
            is_paused: Arc::new(AtomicBool::new(true)),
            pending_scrub: Arc::new(AtomicI64::new(-1)),
            has_started: Arc::new(AtomicBool::new(false)),
            deck_id,
            _decoder_thread: None,
        })
    }

    /// Configures the DXGI Zero-Copy pipeline and spawns the decoder thread.
    pub fn load_and_play(
        &mut self, 
        cue: &crate::app_logic::EngineCue, 
        audio_ring: Arc<AudioRingBuffer>, 
        _clock: Arc<MasterClock>,
        graphics: Arc<Dx11Compositor>,
        _blend_factor: Arc<std::sync::atomic::AtomicU32>,
    ) -> Result<()> {
        
        self.is_playing.store(false, Ordering::Release);
        if let Some(thread) = self._decoder_thread.take() {
            let _ = thread.join(); // Block until previous clip shuts down cleanly
        }
        self.is_playing.store(true, Ordering::Release);
        let is_playing_clone = self.is_playing.clone();
        let is_paused_clone = self.is_paused.clone();
        let pending_scrub_clone = self.pending_scrub.clone();
        let has_started_clone = self.has_started.clone();
        let deck_id = self.deck_id;

        // 1. Convert string to UTF16 for MFCreateSourceReaderFromURL
        use std::os::windows::ffi::OsStrExt;
        let utf16_path: Vec<u16> = std::ffi::OsStr::new(&cue.filepath)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let path_vec = utf16_path.clone();

        let in_point = cue.in_point_hnsecs;
        let out_point = cue.out_point_hnsecs;
        let is_looping = cue.is_looping;
        let hold_last_frame = cue.hold_last_frame;

        self._decoder_thread = Some(thread::spawn(move || {
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED).ok();

                // USE THE GLOBAL GRAPHICS DEVICE FROM THE UI
                // 1. Create the DXGI Device Manager correctly
                let mut reset_token = 0;
                let mut dxgi_manager_opt: Option<IMFDXGIDeviceManager> = None;
                MFCreateDXGIDeviceManager(&mut reset_token, &mut dxgi_manager_opt).unwrap();
                let dxgi_manager = dxgi_manager_opt.unwrap();
                
                // 2. Bind the global DX11 device to the manager
                let iunknown_d3d11: windows::core::IUnknown = graphics.device.cast().unwrap();
                dxgi_manager.ResetDevice(&iunknown_d3d11, reset_token).unwrap();

                // 3. Configure Attributes
                let mut attributes_opt: Option<IMFAttributes> = None;
                MFCreateAttributes(&mut attributes_opt, 3).unwrap();
                let attributes = attributes_opt.unwrap();
                
                // PASS THE DXGI MANAGER (NOT THE RAW DEVICE)
                let iunknown_dxgi: windows::core::IUnknown = dxgi_manager.cast().unwrap();
                attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &iunknown_dxgi).unwrap();
                
                // HARDWARE GPU VIDEO PROCESSOR ONLY (Must use ADVANCED for D3D11/DXGI)
                attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1).unwrap();

                // ENABLE DXVA HARDWARE DECODING (Decompress H.264/HEVC on GPU ASIC instead of CPU)
                attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1).unwrap();

                // 5. Initialize the Source Reader with the File Path and GPU Attributes
                let pcwstr_path = PCWSTR::from_raw(path_vec.as_ptr());
                let source_reader: IMFSourceReader = match MFCreateSourceReaderFromURL(pcwstr_path, &attributes) {
                    Ok(reader) => reader,
                    Err(e) => { eprintln!("FATAL: Failed to load file into MF: {:?}", e); return; }
                };

                // 6. Force Audio Output format to 32-bit Float PCM
                let audio_type = MFCreateMediaType().unwrap();
                audio_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).unwrap();
                audio_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float).unwrap();
                if let Err(e) = source_reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, None, &audio_type) {
                    eprintln!("FATAL: Audio SetCurrentMediaType failed! {:?}", e);
                    return;
                }

                // Native Output (Matches B8G8R8A8 Swapchain exactly, allows native 4K scaling)
                let video_type = MFCreateMediaType().unwrap();
                video_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).unwrap();
                video_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_ARGB32).unwrap(); 
                
                if let Err(e) = source_reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &video_type) {
                    eprintln!("FATAL: Video SetCurrentMediaType failed! {:?}", e);
                    return; 
                }

                // Enable both streams
                source_reader.SetStreamSelection(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, true).unwrap();
                source_reader.SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true).unwrap();
                
                // 6. Discover the actual stream indices for decoding
                let mut actual_video_stream = 0;
                let mut actual_audio_stream = 1;
                
                for i in 0..10 {
                    if let Ok(media_type) = source_reader.GetCurrentMediaType(i) {
                        if let Ok(major_type) = media_type.GetMajorType() {
                            if major_type == MFMediaType_Video {
                                actual_video_stream = i;
                            } else if major_type == MFMediaType_Audio {
                                actual_audio_stream = i;
                            }
                        }
                    }
                }

                // 6b. Force Audio Output to 32-bit Float PCM, STEREO, 48kHz
                let audio_type = MFCreateMediaType().unwrap();
                audio_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).unwrap();
                audio_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float).unwrap();
                
                // CRITICAL FIX: Force OS to downmix 5.1 to Stereo and resample to our 48kHz Master Clock!
                audio_type.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2).unwrap();
                audio_type.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000).unwrap();
                
                if let Err(e) = source_reader.SetCurrentMediaType(actual_audio_stream, None, &audio_type) {
                    eprintln!("M-Playlist [WARNING]: Failed to set audio type: {:?}", e);
                }

                // 6. In-Point Trim Physics
                if in_point > 0 {
                    let mut prop = windows::Win32::System::Com::StructuredStorage::PROPVARIANT::default();
                    (*prop.Anonymous.Anonymous).vt = windows::Win32::System::Variant::VT_I8;
                    (*prop.Anonymous.Anonymous).Anonymous.hVal = in_point;
                    source_reader.SetCurrentPosition(&windows::core::GUID::zeroed(), &prop).unwrap();
                }

                // 7. The Decoding Pump
                Self::decode_loop(
                    source_reader, 
                    audio_ring, 
                    _clock.clone(), 
                    is_playing_clone, 
                    is_paused_clone,
                    pending_scrub_clone,
                    has_started_clone,
                    actual_video_stream,
                    actual_audio_stream,
                    graphics,
                    in_point,
                    out_point,
                    is_looping,
                    hold_last_frame,
                    deck_id
                );

                CoUninitialize();
            }
        }));

        Ok(())
    }

    /// Background loop that pulls samples from the MF Source Reader
    unsafe fn decode_loop(
        reader: windows::Win32::Media::MediaFoundation::IMFSourceReader, 
        audio_ring: Arc<crate::audio_ring::AudioRingBuffer>, 
        clock: Arc<crate::clock::MasterClock>,
        is_playing: Arc<std::sync::atomic::AtomicBool>,
        is_paused: Arc<std::sync::atomic::AtomicBool>,
        pending_scrub: Arc<std::sync::atomic::AtomicI64>,
        has_started: Arc<std::sync::atomic::AtomicBool>,
        video_stream_index: u32,
        audio_stream_index: u32,
        graphics: Arc<crate::graphics::Dx11Compositor>,
        in_point_hnsecs: i64,
        out_point_hnsecs: i64,
        is_looping: bool,
        hold_last_frame: bool,
        deck_id: u8
    ) {
        let mut start_time_offset = 0.0;
        let mut is_currently_paused = false;
        let mut pause_start_time = 0.0;

        let mut first_video_frame_seen = false;
        let mut is_normalized_decoder = false;
        let mut current_base_hnsecs = in_point_hnsecs;
        let mut force_read_once = false;

        while is_playing.load(Ordering::Acquire) {
            
            // 1. Check for manual scrubs!
            let target_hnsecs = pending_scrub.swap(-1, Ordering::SeqCst);
            
            if target_hnsecs >= 0 {
                let _ = reader.Flush(windows::Win32::Media::MediaFoundation::MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32);
                let _ = reader.Flush(windows::Win32::Media::MediaFoundation::MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32);
                
                let mut prop = windows::Win32::System::Com::StructuredStorage::PROPVARIANT::default();
                (*prop.Anonymous.Anonymous).vt = windows::Win32::System::Variant::VT_I8;
                (*prop.Anonymous.Anonymous).Anonymous.hVal = target_hnsecs;
                let _ = reader.SetCurrentPosition(&windows::core::GUID::zeroed(), &prop);
                
                audio_ring.clear();
                
                // CRITICAL FIX: Instantly sync the playback offset to the target scrub time!
                let target_sec = target_hnsecs as f64 / 10_000_000.0;
                start_time_offset = clock.get_time_seconds() - target_sec;
                
                current_base_hnsecs = target_hnsecs;
                first_video_frame_seen = false;
                
                force_read_once = true; // Bypass pause gate to execute the visual frame update
            }

            // --- THE PAUSE GATE ---
            let currently_paused = is_paused.load(Ordering::Acquire);
            if currently_paused && !force_read_once {
                if !is_currently_paused {
                    pause_start_time = clock.get_time_seconds();
                    is_currently_paused = true;
                }
                thread::sleep(Duration::from_millis(5));
                continue;
            } else if is_currently_paused {
                // We just unpaused! Shift the start_time_offset forward by the duration of the pause!
                let pause_duration = clock.get_time_seconds() - pause_start_time;
                start_time_offset += pause_duration;
                is_currently_paused = false;
            }


            // ----------------------
            let mut stream_index = 0;
            let mut flags = 0;
            let mut timestamp = 0;
            let mut sample_opt = None;

            // Note: adapt pointer types based on windows-rs 0.52.0 strict typing
            let hr = reader.ReadSample(
                windows::Win32::Media::MediaFoundation::MF_SOURCE_READER_ANY_STREAM.0 as u32,
                0,
                Some(&mut stream_index),
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample_opt),
            );

            if hr.is_err() || (flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
                println!("M-Playlist: End of stream reached.");
                break;
            }

            if let Some(sample) = sample_opt {
                if stream_index == audio_stream_index {
                    
                    // AUDIO BYPASSES THE SYNC GATE! 
                    // We extract audio instantly to feed the Master Clock.
                    if let Ok(buffer) = sample.ConvertToContiguousBuffer() {
                        let mut raw_ptr = std::ptr::null_mut();
                        let mut current_len = 0;
                        
                        buffer.Lock(&mut raw_ptr, None, Some(&mut current_len)).unwrap();
                        let num_floats = (current_len / 4) as usize; 
                        let float_slice = std::slice::from_raw_parts(raw_ptr as *const f32, num_floats);
                        
                        // Ring buffer backpressure naturally syncs audio decoding speed
                        for &f in float_slice {
                            while audio_ring.push(f).is_err() {
                                if !is_playing.load(std::sync::atomic::Ordering::Acquire) { break; }
                                std::thread::sleep(std::time::Duration::from_millis(2));
                            }
                        }
                        buffer.Unlock().unwrap();
                    }
                    
                } else if stream_index == video_stream_index {
                    let mut frame_time_100ns = timestamp;
                    
                    if !first_video_frame_seen {
                        if current_base_hnsecs > 0 && frame_time_100ns < (current_base_hnsecs / 2) {
                            println!("M-Playlist [WARNING]: Hardware Decoder normalized timestamps. Engaging absolute timeline correction.");
                            is_normalized_decoder = true;
                        } else {
                            is_normalized_decoder = false;
                        }
                        first_video_frame_seen = true;
                    }

                    if is_normalized_decoder {
                        frame_time_100ns += current_base_hnsecs;
                    }
                    
                    // PRE-ROLL PURGE: Discard hardware keyframes emitted before the target trim boundary
                    if frame_time_100ns < current_base_hnsecs {
                        continue;
                    }
                    
                    if out_point_hnsecs > 0 && frame_time_100ns >= out_point_hnsecs {
                        if is_looping {
                            // Atomic Swap to trigger a scrub back to in_point!
                            pending_scrub.store(in_point_hnsecs, Ordering::SeqCst);
                            continue;
                        } else if hold_last_frame {
                            // Freeze without clearing DX11
                            is_paused.store(true, Ordering::Release);
                            continue;
                        } else {
                            break;
                        }
                    }

                    if !has_started.load(Ordering::Acquire) {
                        let in_point_sec = in_point_hnsecs as f64 / 10_000_000.0;
                        // Anchor the local timeline exactly when the first frame is ready, neutralizing seek latency
                        start_time_offset = clock.get_time_seconds() - in_point_sec; 
                        has_started.store(true, Ordering::Release);
                    }

                    let normalized_time_100ns = frame_time_100ns;
                    let frame_time_sec = normalized_time_100ns as f64 / 10_000_000.0;
                    
                    // Report the exact video frame time to the global diagnostic tracker!
                    crate::ffi::CURRENT_VIDEO_TIME_US.store((frame_time_sec * 1_000_000.0) as u64, std::sync::atomic::Ordering::Relaxed);
                    
                    let current_playback_time = clock.get_time_seconds() - start_time_offset;
                    let offset_us = crate::ffi::SYNC_OFFSET_US.load(std::sync::atomic::Ordering::Relaxed);
                    let hardware_sync_offset = offset_us as f64 / 1_000_000.0;
                    let calibrated_time = current_playback_time - hardware_sync_offset;
                    
                    // --- FRAME DROP LOGIC (Slow Decoder Protection) ---
                    // If the hardware decoder is choking on a 120FPS 4K file and falls >50ms behind real-time,
                    // we must DROP the frame to prevent the video from playing in slow motion!
                    if frame_time_sec < calibrated_time - 0.05 && !force_read_once {
                        continue;
                    }
                    
                    // --- VIDEO A/V SYNC GATE ---
                    loop {
                        if !is_playing.load(std::sync::atomic::Ordering::Acquire) || is_paused.load(std::sync::atomic::Ordering::Acquire) {
                            break;
                        }
                        
                        let current_playback_time = clock.get_time_seconds() - start_time_offset;
                        
                        // Read the live global offset from the UI (convert microseconds back to seconds)
                        let offset_us = crate::ffi::SYNC_OFFSET_US.load(std::sync::atomic::Ordering::Relaxed);
                        let hardware_sync_offset = offset_us as f64 / 1_000_000.0;
                        
                        let calibrated_time = current_playback_time - hardware_sync_offset;

                        // Tightened to 0.01 for sub-frame accuracy!
                        if frame_time_sec <= calibrated_time + 0.01 { 
                            break; 
                        }
                        
                        // Tightened sleep to 1ms for higher precision polling
                        std::thread::sleep(std::time::Duration::from_millis(1)); 
                    }
                    
                    force_read_once = false;
                    
                    // EXTRACT & RENDER
                    if let Ok(media_buffer) = sample.GetBufferByIndex(0) {
                        
                        // 1. Cast the generic media buffer into a DXGI Hardware Buffer
                        let dxgi_buffer_result: windows::core::Result<IMFDXGIBuffer> = media_buffer.cast();
                        
                        if let Ok(dxgi_buffer) = dxgi_buffer_result {
                            
                            let subresource_index = unsafe { dxgi_buffer.GetSubresourceIndex().unwrap_or(0) };
                            
                            // 2. Extract the native D3D11 Texture from the DXGI Buffer!
                            let mut texture_opt: Option<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D> = None;
                            let hr = unsafe { dxgi_buffer.GetResource(
                                &<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D as windows::core::ComInterface>::IID, 
                                &mut texture_opt as *mut _ as *mut *mut std::ffi::c_void
                            ) };
                            
                            hr.expect(
                                "FATAL ARCHITECTURE VIOLATION: Failed to get ID3D11Texture2D from the DXGI Buffer."
                            );
                            
                            let video_texture = texture_opt.unwrap();

                            // 3. Blast it to the screen via Compositor
                            if let Err(e) = graphics.update_deck_texture(deck_id, &video_texture, subresource_index, &sample) {
                                eprintln!("M-Playlist [RENDER ERROR]: Failed to update deck texture: {:?}", e);
                            }
                            
                        } else {
                            panic!("FATAL ARCHITECTURE VIOLATION: Media Buffer is NOT a DXGI Buffer! The DXGI Device Manager was bypassed. DO NOT FALL BACK TO CPU.");
                        }
                    }
                }
            }
        }
    }
}

impl Drop for MediaEngine {
    fn drop(&mut self) {
        self.is_playing.store(false, Ordering::Release);
        if let Some(thread) = self._decoder_thread.take() {
            let _ = thread.join();
        }
    }
}
