pub mod audio_ring;
pub mod audio_wasapi;
pub mod clock;
pub mod ffi;
pub mod graphics;
pub mod playlist;
pub mod media_engine;

pub mod app_logic;
pub mod ndi_sys;
pub mod ndi_transmitter;
pub mod ndi_ffi;
pub mod ndi_receiver;
pub mod desktop_capture;
pub mod wic;
pub mod webview_capture;

// --- PHASE 8 & 9 FFI BOUNDARIES ---
#[no_mangle]
pub extern "C" fn mplaylist_get_audio_levels(left: *mut f32, right: *mut f32) {
    if !left.is_null() && !right.is_null() {
        unsafe {
            *left = f32::from_bits(crate::audio_wasapi::PEAK_L.swap(0, std::sync::atomic::Ordering::Relaxed));
            *right = f32::from_bits(crate::audio_wasapi::PEAK_R.swap(0, std::sync::atomic::Ordering::Relaxed));
        }
    }
}

#[no_mangle]
pub extern "C" fn mplaylist_set_overlay_text(show: bool, text: *const u16) {
    crate::graphics::SHOW_OVERLAY.store(show, std::sync::atomic::Ordering::Relaxed);
    
    if text.is_null() {
        if let Ok(mut lock) = crate::graphics::OVERLAY_TEXT.write() { lock.clear(); }
        return;
    }

    unsafe {
        let mut len = 0;
        while *text.add(len) != 0 { len += 1; } // Find UTF-16 null terminator
        let slice = std::slice::from_raw_parts(text, len);
        
        if let Ok(mut lock) = crate::graphics::OVERLAY_TEXT.write() {
            lock.clear();
            lock.extend_from_slice(slice);
        }
    }
}
pub mod decklink_capture;


pub mod lufs_scanner;

pub mod transcoder;

pub mod audio_capture;
