use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

// Global offset stored in microseconds. Default: 80ms (80,000 us)
pub static SYNC_OFFSET_US: AtomicI64 = AtomicI64::new(80_000);
// Global tracker for the last decoded video frame
pub static CURRENT_VIDEO_TIME_US: AtomicU64 = AtomicU64::new(0);
// Global toggle for NDI broadcast
pub static NDI_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

use crate::audio_ring::AudioRingBuffer;
use crate::audio_wasapi::WasapiEngine;
use crate::clock::MasterClock;
use crate::graphics::Dx11Compositor;
use crate::app_logic::{AppLogic, EngineCommand, EngineCue};

#[repr(C)]
pub struct FfiCue {
    pub filepath: *const std::ffi::c_char,
    pub in_point_hnsecs: i64,      // 100-nanosecond units. 0 = beginning
    pub out_point_hnsecs: i64,     // 0 = play to end
    pub is_looping: u8,            // u8 used to prevent C# bool ABI alignment issues
    pub hold_last_frame: u8,
    pub transition_duration_hnsecs: i64,
    pub modality: u8,
}

struct EngineState {
    ring_a: Arc<AudioRingBuffer>,
    ring_b: Arc<AudioRingBuffer>,
    blend_factor: Arc<std::sync::atomic::AtomicU32>,
    clock: Arc<MasterClock>,
    wasapi: Option<Arc<WasapiEngine>>,
    graphics: Option<Arc<Dx11Compositor>>,
    app_logic: Option<AppLogic>, // <-- Replaces Playlist
}

static ENGINE_STATE: OnceLock<Mutex<EngineState>> = OnceLock::new();

fn get_state() -> &'static Mutex<EngineState> {
    ENGINE_STATE.get_or_init(|| {
        Mutex::new(EngineState {
            ring_a: Arc::new(AudioRingBuffer::new(153600)), // 200ms strict capacity (16 channels)
            ring_b: Arc::new(AudioRingBuffer::new(153600)), // 200ms strict capacity (16 channels)
            blend_factor: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            clock: Arc::new(MasterClock::new(48000)),
            wasapi: None,
            graphics: None,
            app_logic: None,
        })
    })
}

#[no_mangle]
pub extern "C" fn mplaylist_init() -> bool {
    // Elevate the Windows system timer resolution to 1 millisecond.
    // Without this, std::thread::sleep(1ms) will actually sleep for 15.6ms!
    unsafe { windows::Win32::Media::timeBeginPeriod(1); }
    
    unsafe { windows::Win32::Media::MediaFoundation::MFStartup(windows::Win32::Media::MediaFoundation::MF_VERSION, 0).ok(); }
    
    let mut state = get_state().lock().unwrap();
    if state.wasapi.is_some() { return true; }
    
    // Note: We no longer initialize the Playlist here, as it waits for the Graphics Window.
    
    match crate::audio_wasapi::WasapiEngine::start(
        state.ring_a.clone(), 
        state.ring_b.clone(), 
        state.clock.clone(),
        state.blend_factor.clone()
    ) {
        Ok(engine) => {
            state.wasapi = Some(engine);
            true
        }
        Err(_) => false
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_shutdown() {
    let mut state = get_state().lock().unwrap();
    state.app_logic = None; 
    state.wasapi = None; 
    unsafe { windows::Win32::Media::MediaFoundation::MFShutdown().ok(); }
    unsafe { windows::Win32::Media::timeEndPeriod(1); }
}

#[no_mangle]
pub extern "C" fn mplaylist_set_window(hwnd_ptr: *mut std::ffi::c_void) -> bool {
    if hwnd_ptr.is_null() { return false; }
    let mut state = get_state().lock().unwrap();
    
    if state.graphics.is_some() { return true; }

    match Dx11Compositor::new(hwnd_ptr) {
        Ok(gfx) => {
            let gfx_arc = Arc::new(gfx);
            println!("M-Playlist: DX11 Swapchain bound to HWND ({}x{}).", 
                gfx_arc.width.load(std::sync::atomic::Ordering::Relaxed), 
                gfx_arc.height.load(std::sync::atomic::Ordering::Relaxed)
            );
            state.graphics = Some(gfx_arc.clone());
            
            // We can only start the AppLogic once the Graphics Compositor is bound!
            if state.app_logic.is_none() {
                if let Some(wasapi_arc) = state.wasapi.clone() {
                    state.app_logic = Some(AppLogic::start(
                        state.ring_a.clone(), 
                        state.ring_b.clone(),
                        state.blend_factor.clone(),
                        state.clock.clone(), 
                        gfx_arc,
                        wasapi_arc
                    ));
                }
            }
            true
        }
        Err(e) => {
            eprintln!("Failed to init Graphics Compositor: {:?}", e);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_load_cue(cue: FfiCue) -> bool {
    if cue.filepath.is_null() { return false; }
    
    let path_str = unsafe { std::ffi::CStr::from_ptr(cue.filepath) }
        .to_string_lossy()
        .into_owned();

    let engine_cue = EngineCue {
        filepath: path_str,
        in_point_hnsecs: cue.in_point_hnsecs,
        out_point_hnsecs: cue.out_point_hnsecs,
        is_looping: cue.is_looping != 0,
        hold_last_frame: cue.hold_last_frame != 0,
        transition_duration_hnsecs: cue.transition_duration_hnsecs,
        modality: cue.modality,
    };

    let state = get_state().lock().unwrap();
    if let Some(logic) = state.app_logic.as_ref() {
        let _ = logic.tx.send(EngineCommand::LoadCue(engine_cue));
        return true;
    }
    false
}

#[no_mangle]
pub extern "C" fn mplaylist_fire_cue(cue: FfiCue) -> bool {
    let filepath = if cue.filepath.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(cue.filepath).to_string_lossy().into_owned() } };
    
    let owned_cue = crate::app_logic::OwnedCue {
        filepath,
        in_point_hnsecs: cue.in_point_hnsecs,
        out_point_hnsecs: cue.out_point_hnsecs,
        is_looping: cue.is_looping,
        hold_last_frame: cue.hold_last_frame,
        transition_duration_hnsecs: cue.transition_duration_hnsecs,
        modality: cue.modality,
    };

    let state = get_state().lock().unwrap();
    if let Some(logic) = state.app_logic.as_ref() {
        let _ = logic.tx.send(crate::app_logic::EngineCommand::FireCue(owned_cue));
        return true;
    }
    false
}

#[no_mangle]
pub extern "C" fn mplaylist_stop() -> bool {
    let state = get_state().lock().unwrap();
    if let Some(logic) = state.app_logic.as_ref() {
        let _ = logic.tx.send(EngineCommand::Stop);
        return true;
    }
    false
}

#[no_mangle]
pub extern "C" fn mplaylist_pause() {
    let state = get_state().lock().unwrap();
    if let Some(logic) = state.app_logic.as_ref() {
        let _ = logic.tx.send(EngineCommand::Pause);
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_resume() {
    let state = get_state().lock().unwrap();
    if let Some(logic) = state.app_logic.as_ref() {
        let _ = logic.tx.send(EngineCommand::Resume);
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_set_volume_db(db: f32) {
    let mut amplitude = 10_f32.powf(db / 20.0);
    if db <= -60.0 {
        amplitude = 0.0;
    }
    
    let state = get_state().lock().unwrap();
    if let Some(logic) = state.app_logic.as_ref() {
        let _ = logic.tx.send(EngineCommand::SetVolume(amplitude));
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_set_audio_device(index: u32) {
    let state = get_state().lock().unwrap();
    if let Some(logic) = state.app_logic.as_ref() {
        let _ = logic.tx.send(EngineCommand::SetAudioDevice(index));
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_scrub_to(target_hnsecs: i64) {
    let state = get_state().lock().unwrap();
    if let Some(logic) = state.app_logic.as_ref() {
        let _ = logic.tx.send(EngineCommand::Scrub(target_hnsecs));
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_get_dimensions(out_width: *mut u32, out_height: *mut u32) -> bool {
    let state = get_state().lock().unwrap();
    if let Some(graphics) = state.graphics.as_ref() {
        unsafe {
            if !out_width.is_null() {
                *out_width = graphics.width.load(std::sync::atomic::Ordering::Relaxed);
            }
            if !out_height.is_null() {
                *out_height = graphics.height.load(std::sync::atomic::Ordering::Relaxed);
            }
        }
        return true;
    }
    false
}

#[no_mangle]
pub extern "C" fn mplaylist_set_sync_offset(offset_seconds: f64) {
    let microseconds = (offset_seconds * 1_000_000.0) as i64;
    SYNC_OFFSET_US.store(microseconds, Ordering::Relaxed);
}

#[no_mangle]
pub extern "C" fn mplaylist_get_diagnostics(out_audio_time: *mut f64, out_video_time: *mut f64) -> bool {
    let state = get_state().lock().unwrap();
    let clock = &state.clock;
    unsafe {
        if !out_audio_time.is_null() {
            *out_audio_time = clock.get_time_seconds();
        }
        if !out_video_time.is_null() {
            let video_us = CURRENT_VIDEO_TIME_US.load(Ordering::Relaxed);
            *out_video_time = video_us as f64 / 1_000_000.0;
        }
    }
    true
}

#[no_mangle]
pub extern "C" fn mplaylist_get_audio_device_count() -> u32 {
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
        if let Ok(enum_obj) = windows::Win32::System::Com::CoCreateInstance::<_, windows::Win32::Media::Audio::IMMDeviceEnumerator>(
            &windows::Win32::Media::Audio::MMDeviceEnumerator, None, windows::Win32::System::Com::CLSCTX_ALL) {
            if let Ok(collection) = enum_obj.EnumAudioEndpoints(windows::Win32::Media::Audio::eRender, windows::Win32::Media::Audio::DEVICE_STATE_ACTIVE) {
                if let Ok(count) = collection.GetCount() {
                    return count;
                }
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn mplaylist_get_audio_device_name(index: u32, buffer: *mut u8, max_len: u32) -> u32 {
    if buffer.is_null() || max_len == 0 { return 0; }
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
        if let Ok(enum_obj) = windows::Win32::System::Com::CoCreateInstance::<_, windows::Win32::Media::Audio::IMMDeviceEnumerator>(
            &windows::Win32::Media::Audio::MMDeviceEnumerator, None, windows::Win32::System::Com::CLSCTX_ALL) {
            if let Ok(collection) = enum_obj.EnumAudioEndpoints(windows::Win32::Media::Audio::eRender, windows::Win32::Media::Audio::DEVICE_STATE_ACTIVE) {
                if let Ok(device) = collection.Item(index) {
                    if let Ok(store) = device.OpenPropertyStore(windows::Win32::System::Com::STGM_READ) {
                        let pkey = windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY {
                            fmtid: windows::core::GUID::from_values(0xa45c254e, 0xdf1c, 0x4efd, [0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0]),
                            pid: 14,
                        };
                        if let Ok(prop_var) = store.GetValue(&pkey) {
                            if prop_var.Anonymous.Anonymous.vt == windows::Win32::System::Variant::VT_LPWSTR {
                                let pwstr = prop_var.Anonymous.Anonymous.Anonymous.pwszVal;
                                if !pwstr.is_null() {
                                    let name_str = pwstr.to_string().unwrap_or_default();
                                    let bytes = name_str.as_bytes();
                                    let copy_len = std::cmp::min(bytes.len(), (max_len - 1) as usize);
                                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, copy_len);
                                    *buffer.add(copy_len) = 0; // null terminator
                                    
                                    windows::Win32::System::Com::CoTaskMemFree(Some(pwstr.as_ptr() as *const std::ffi::c_void));
                                    return copy_len as u32;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn mplaylist_set_ndi_enabled(enabled: bool) {
    NDI_ENABLED.store(enabled, Ordering::Relaxed);
}

#[no_mangle]
pub extern "C" fn mplaylist_resize_swapchain(_width: u32, _height: u32) {
    // LOBOTOMIZED: The engine is now locked to a 1920x1080 internal broadcast standard.
    // DXGI hardware scaling (DXGI_SCALING_STRETCH) will automatically fit it to the C# UI window.
}

#[no_mangle]
pub extern "C" fn mplaylist_set_geometry(
    tl_x: f32, tl_y: f32,
    tr_x: f32, tr_y: f32,
    bl_x: f32, bl_y: f32,
    br_x: f32, br_y: f32,
) {
    let state = get_state().lock().unwrap();
    if let Some(logic) = state.app_logic.as_ref() {
        let _ = logic.tx.send(EngineCommand::SetGeometry([
            tl_x, tl_y, tr_x, tr_y, bl_x, bl_y, br_x, br_y,
        ]));
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_get_audio_telemetry(deck_id: i32, out_occupancy: *mut i32, out_capacity: *mut i32) {
    if out_occupancy.is_null() || out_capacity.is_null() { return; }
    if let Ok(state) = get_state().try_lock() {
        let ring = if deck_id == 0 { &state.ring_a } else { &state.ring_b };
        unsafe {
            *out_occupancy = ring.get_occupancy() as i32;
            *out_capacity = ring.get_capacity() as i32;
        }
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_set_audio_route(deck_id: i32, in_ch: i32, out_bus: i32, gain_db: f32) {
    let state = get_state().lock().unwrap();
    if let Some(logic) = state.app_logic.as_ref() {
        let _ = logic.tx.send(EngineCommand::SetAudioRoute { deck_id, in_ch, out_bus, gain_db });
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_set_spatial_color(
    crop_left: f32, crop_top: f32, crop_right: f32, crop_bottom: f32,
    pan_x: f32, pan_y: f32, zoom: f32,
    brightness: f32, contrast: f32, saturation: f32
) {
    if let Ok(mut state) = crate::graphics::SPATIAL_COLOR_STATE.write() {
        state.crop_left = crop_left;
        state.crop_top = crop_top;
        state.crop_right = crop_right;
        state.crop_bottom = crop_bottom;
        state.pan_x = pan_x;
        state.pan_y = pan_y;
        state.zoom = zoom;
        state.brightness = brightness;
        state.contrast = contrast;
        state.saturation = saturation;
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_bind_output_matrix(hwnd: *mut std::ffi::c_void) -> bool {
    let state = get_state().lock().unwrap();
    if let Some(graphics) = state.graphics.as_ref() {
        unsafe {
            use windows::Win32::Graphics::Dxgi::*;
            use windows::Win32::Foundation::HWND;
            use windows::core::ComInterface;
            
            let device = &graphics.device;
            let dxgi_device: windows::core::Result<IDXGIDevice> = device.cast();
            if dxgi_device.is_err() { return false; }
            let dxgi_device = dxgi_device.unwrap();
            
            let dxgi_adapter: windows::core::Result<IDXGIAdapter> = dxgi_device.GetAdapter();
            if dxgi_adapter.is_err() { return false; }
            let dxgi_adapter = dxgi_adapter.unwrap();
            
            let dxgi_factory: windows::core::Result<IDXGIFactory2> = dxgi_adapter.GetParent();
            if dxgi_factory.is_err() { return false; }
            let dxgi_factory = dxgi_factory.unwrap();

            let swapchain_desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: 1920, 
                Height: 1080,
                Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: false.into(),
                SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: windows::Win32::Graphics::Dxgi::Common::DXGI_ALPHA_MODE_IGNORE, 
                Flags: 0,
            };

            let swapchain = dxgi_factory.CreateSwapChainForHwnd(
                device, 
                HWND(hwnd as _), 
                &swapchain_desc, 
                None, 
                None
            );
            
            if let Ok(swapchain) = swapchain {
                if let Ok(mut lock) = graphics.output_swapchain.lock() {
                    *lock = Some(swapchain);
                    return true;
                }
            }
        }
    }
    false
}
