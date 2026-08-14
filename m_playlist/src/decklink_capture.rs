use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct DecklinkSharedFrame {
    pub frame_ptr: *mut std::ffi::c_void, 
    pub data_ptr: *const u8, 
    pub width: u32, 
    pub height: u32, 
    pub row_bytes: u32,
}
unsafe impl Send for DecklinkSharedFrame {} 
unsafe impl Sync for DecklinkSharedFrame {}

extern "C" {
    fn decklink_start(
        hardware_index: u8, 
        cb: extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *const u8, u32, u32, u32), 
        user_data: *mut std::ffi::c_void
    ) -> *mut std::ffi::c_void;
    pub fn decklink_release_frame(frame: *mut std::ffi::c_void);
    fn decklink_stop(ctx: *mut std::ffi::c_void);
}

extern "C" fn rust_decklink_callback(
    user_data: *mut std::ffi::c_void, 
    frame: *mut std::ffi::c_void, 
    data: *const u8, 
    width: u32, 
    height: u32, 
    row_bytes: u32
) {
    let mutex_ptr = user_data as *const Mutex<Option<DecklinkSharedFrame>>;
    if let Some(mutex) = unsafe { mutex_ptr.as_ref() } {
        if let Ok(mut lock) = mutex.lock() {
            if let Some(old) = lock.take() {
                unsafe { decklink_release_frame(old.frame_ptr) };
            }
            *lock = Some(DecklinkSharedFrame {
                frame_ptr: frame,
                data_ptr: data,
                width,
                height,
                row_bytes,
            });
        }
    }
}

pub fn spawn_receiver(
    hardware_index: u8,
    rx_mutex: Arc<Mutex<Option<DecklinkSharedFrame>>>,
    run_flag: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        println!("M-Playlist [SDI]: DeckLink receiver started for device {}", hardware_index);
        
        let mutex_ptr = Arc::into_raw(rx_mutex.clone()) as *mut std::ffi::c_void;
        
        let ctx = unsafe { decklink_start(hardware_index, rust_decklink_callback, mutex_ptr) };
        if ctx.is_null() {
            println!("M-Playlist [SDI]: Failed to start DeckLink stream. SDK missing or hardware unavailable.");
            let _reclaimed = unsafe { Arc::from_raw(mutex_ptr as *const Mutex<Option<DecklinkSharedFrame>>) };
            return;
        }

        while run_flag.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }

        unsafe { decklink_stop(ctx) };
        let _reclaimed_arc = unsafe { Arc::from_raw(mutex_ptr as *const Mutex<Option<DecklinkSharedFrame>>) };
        
        println!("M-Playlist [SDI]: DeckLink receiver stopped.");
    });
}
