use std::sync::mpsc::{sync_channel, SyncSender, Receiver};
use std::thread;
use std::ffi::CString;

use crate::ndi_sys::*;

pub struct NdiFrame { 
    pub data: Vec<u8>, 
    pub width: i32, 
    pub height: i32, 
    pub stride: i32 
}

pub struct NdiTransmitter {
    pub tx: SyncSender<NdiFrame>,
    // we leak the library pointer to avoid lifetime/Send issues across threads
}

impl NdiTransmitter {
    pub fn new() -> Option<Self> {
        let lib = match unsafe { libloading::Library::new("Processing.NDI.Lib.x64.dll") } {
            Ok(l) => l,
            Err(e) => {
                eprintln!("M-Playlist [NDI]: Failed to load Processing.NDI.Lib.x64.dll: {:?}", e);
                return None;
            }
        };

        unsafe {
            let initialize: libloading::Symbol<NDIlib_initialize_fn> = 
                match lib.get(b"NDIlib_initialize\0") {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("M-Playlist [NDI]: Failed to get NDIlib_initialize: {:?}", e);
                        return None;
                    }
                };
            let send_create: libloading::Symbol<NDIlib_send_create_fn> = 
                match lib.get(b"NDIlib_send_create\0") {
                    Ok(s) => s,
                    Err(_) => return None,
                };
            let send_video: libloading::Symbol<NDIlib_send_send_video_v2_fn> = 
                match lib.get(b"NDIlib_send_send_video_v2\0") {
                    Ok(s) => s,
                    Err(_) => return None,
                };

            if initialize() == 0 {
                eprintln!("M-Playlist [NDI]: NDIlib_initialize failed!");
                return None;
            }

            let ndi_name = CString::new("M-Playlist Output").unwrap();
            let create_desc = NDIlib_send_create_t {
                p_ndi_name: ndi_name.as_ptr(),
                p_groups: std::ptr::null(),
                clock_video: 0,
                clock_audio: 0,
            };

            let instance = send_create(&create_desc);
            if instance.is_null() {
                eprintln!("M-Playlist [NDI]: Failed to create NDI send instance!");
                return None;
            }
            let instance_usize = instance as usize;
            let send_video_usize = (*send_video) as usize;
            
            // Leak the library so it stays alive for the duration of the program.
            // This is perfectly safe as NDI is a singleton DLL anyway.
            Box::leak(Box::new(lib));

            let (tx, rx): (SyncSender<NdiFrame>, Receiver<NdiFrame>) = sync_channel(2);

            thread::spawn(move || {
                let instance_ptr = instance_usize as *mut std::ffi::c_void;
                let send_video_ptr: NDIlib_send_send_video_v2_fn = std::mem::transmute(send_video_usize);
                
                println!("M-Playlist [NDI]: Transmitter thread started.");

                while let Ok(frame) = rx.recv() {
                    let video_data = NDIlib_video_frame_v2_t {
                        xres: frame.width,
                        yres: frame.height,
                        FourCC: 0x41524742, // BGRA
                        frame_rate_N: 60000,
                        frame_rate_D: 1000,
                        picture_aspect_ratio: frame.width as f32 / frame.height as f32,
                        frame_format_type: 1, // progressive
                        timecode: 0,
                        p_data: frame.data.as_ptr(),
                        line_stride_in_bytes: frame.stride,
                        p_metadata: std::ptr::null(),
                        timestamp: 0,
                    };
                    send_video_ptr(instance_ptr, &video_data);
                }
                
                println!("M-Playlist [NDI]: Transmitter thread shutting down.");
            });

            Some(NdiTransmitter { tx })
        }
    }
}
