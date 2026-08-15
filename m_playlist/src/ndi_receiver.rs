use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use crate::graphics::NdiPingPong;

pub fn spawn_receiver(uri: String, rx_buffer: Arc<Mutex<NdiPingPong>>, run_flag: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        unsafe {
            // Retrieve global NdiLibrary using whatever singleton method you defined in ndi_ffi.rs
            // (e.g., NdiLibrary::get() or NDI_LIB.as_ref().unwrap())
            let ndi_ptr = crate::ndi_ffi::NdiLibrary::load().unwrap(); 
            
            let source_name = uri.trim_start_matches("ndi://");
            let c_url = std::ffi::CString::new(source_name).unwrap_or_default();
            let source = crate::ndi_ffi::NDIlib_source_t {
                p_ndi_name: c_url.as_ptr(),
                p_url_address: std::ptr::null(),
            };
            
            let recv_desc = crate::ndi_ffi::NDIlib_recv_create_v3_t {
                source_to_connect_to: source, color_format: 0, bandwidth: 100, allow_video_fields: false, p_ndi_recv_name: std::ptr::null(),
            };
            
            let recv_instance = (ndi_ptr.NDIlib_recv_create_v3)(&recv_desc);
            if recv_instance.ptr.is_null() { return; }
            
            let mut video_frame = crate::ndi_ffi::NDIlib_video_frame_v2_t {
                xres: 0, yres: 0, FourCC: 0, frame_rate_N: 0, frame_rate_D: 0,
                picture_aspect_ratio: 0.0, frame_format_type: 0, timecode: 0,
                p_data: std::ptr::null_mut(), line_stride_in_bytes: 0, p_metadata: std::ptr::null(), timestamp: 0,
            };
            let mut audio_frame = std::mem::MaybeUninit::uninit();
            
            
            let mut last_tally = false;
            
            while run_flag.load(Ordering::Acquire) {
                let on_program = rx_buffer.lock().map(|rx| rx.on_program).unwrap_or(false);
                if on_program != last_tally {
                    let tally = crate::ndi_ffi::NDIlib_tally_t {
                        on_program,
                        on_preview: false,
                    };
                    (ndi_ptr.NDIlib_recv_set_tally)(recv_instance, &tally);
                    last_tally = on_program;
                }

                let frame_type = (ndi_ptr.NDIlib_recv_capture_v2)(
                    recv_instance, &mut video_frame, audio_frame.as_mut_ptr(), std::ptr::null_mut(), 100,
                );
                
                if frame_type == 1 && !video_frame.p_data.is_null() {
                    if let Ok(mut rx) = rx_buffer.lock() {
                        let size = (video_frame.line_stride_in_bytes * video_frame.yres) as usize;
                        if rx.pixels.len() != size { rx.pixels.resize(size, 0); }
                        std::ptr::copy_nonoverlapping(video_frame.p_data, rx.pixels.as_mut_ptr(), size);
                        rx.width = video_frame.xres as u32;
                        rx.height = video_frame.yres as u32;
                        rx.stride = video_frame.line_stride_in_bytes as u32;
                        rx.is_dirty = true;
                    }
                    (ndi_ptr.NDIlib_recv_free_video_v2)(recv_instance, &video_frame);
                }
            }
            (ndi_ptr.NDIlib_recv_destroy)(recv_instance);
        }
    });
}
