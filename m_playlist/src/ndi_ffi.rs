#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::ffi::{c_void, CStr, CString};
use windows::core::PCSTR;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows::Win32::Foundation::HMODULE;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NDIlib_send_instance_t {
    pub ptr: *mut c_void,
}
unsafe impl Send for NDIlib_send_instance_t {}
unsafe impl Sync for NDIlib_send_instance_t {}

#[repr(C)]
pub struct NDIlib_send_create_t {
    pub p_ndi_name: *const i8,
    pub p_groups: *const i8,
    pub clock_video: bool,
    pub clock_audio: bool,
}

#[repr(C)]
pub struct NDIlib_video_frame_v2_t {
    pub xres: i32,
    pub yres: i32,
    pub FourCC: i32,
    pub frame_rate_N: i32,
    pub frame_rate_D: i32,
    pub picture_aspect_ratio: f32,
    pub frame_format_type: i32,
    pub timecode: i64,
    pub p_data: *mut u8,
    pub line_stride_in_bytes: i32,
    pub p_metadata: *const i8,
    pub timestamp: i64,
}

#[repr(C)]
pub struct NDIlib_audio_frame_v2_t {
    pub sample_rate: i32,
    pub no_channels: i32,
    pub no_samples: i32,
    pub timecode: i64,
    pub p_data: *mut f32,
    pub channel_stride_in_bytes: i32,
    pub p_metadata: *const i8,
    pub timestamp: i64,
}

pub const NDIlib_FourCC_video_type_BGRA: i32 = i32::from_le_bytes([b'B', b'G', b'R', b'A']);

pub type fn_NDIlib_initialize = unsafe extern "C" fn() -> bool;
pub type fn_NDIlib_send_create = unsafe extern "C" fn(p_create_settings: *const NDIlib_send_create_t) -> NDIlib_send_instance_t;
pub type fn_NDIlib_send_send_video_v2 = unsafe extern "C" fn(p_instance: NDIlib_send_instance_t, p_video_data: *const NDIlib_video_frame_v2_t);
pub type fn_NDIlib_send_send_audio_v2 = unsafe extern "C" fn(p_instance: NDIlib_send_instance_t, p_audio_data: *const NDIlib_audio_frame_v2_t);
pub type fn_NDIlib_send_destroy = unsafe extern "C" fn(p_instance: NDIlib_send_instance_t);
pub type fn_NDIlib_destroy = unsafe extern "C" fn();

pub struct NdiLibrary {
    handle: HMODULE,
    pub NDIlib_initialize: fn_NDIlib_initialize,
    pub NDIlib_send_create: fn_NDIlib_send_create,
    pub NDIlib_send_send_video_v2: fn_NDIlib_send_send_video_v2,
    pub NDIlib_send_send_audio_v2: fn_NDIlib_send_send_audio_v2,
    pub NDIlib_send_destroy: fn_NDIlib_send_destroy,
    pub NDIlib_destroy: fn_NDIlib_destroy,
}

impl NdiLibrary {
    pub fn load() -> Result<Self, String> {
        unsafe {
            let dll_name = CString::new("Processing.NDI.Lib.x64.dll").unwrap();
            let handle = match LoadLibraryA(PCSTR(dll_name.as_ptr() as *const u8)) {
                Ok(h) => h,
                Err(_) => return Err("Failed to load Processing.NDI.Lib.x64.dll".to_string()),
            };

            if handle.is_invalid() {
                return Err("Failed to load Processing.NDI.Lib.x64.dll".to_string());
            }

            macro_rules! get_proc {
                ($name:expr, $type:ty) => {{
                    let func_name = CString::new($name).unwrap();
                    let addr = GetProcAddress(handle, PCSTR(func_name.as_ptr() as *const u8));
                    if addr.is_none() {
                        return Err(format!("Failed to locate function {}", $name));
                    }
                    std::mem::transmute::<_, $type>(addr.unwrap())
                }};
            }

            let init = get_proc!("NDIlib_initialize", fn_NDIlib_initialize);
            let send_create = get_proc!("NDIlib_send_create", fn_NDIlib_send_create);
            let send_video = get_proc!("NDIlib_send_send_video_v2", fn_NDIlib_send_send_video_v2);
            let send_audio = get_proc!("NDIlib_send_send_audio_v2", fn_NDIlib_send_send_audio_v2);
            let send_destroy = get_proc!("NDIlib_send_destroy", fn_NDIlib_send_destroy);
            let destroy = get_proc!("NDIlib_destroy", fn_NDIlib_destroy);

            Ok(Self {
                handle,
                NDIlib_initialize: init,
                NDIlib_send_create: send_create,
                NDIlib_send_send_video_v2: send_video,
                NDIlib_send_send_audio_v2: send_audio,
                NDIlib_send_destroy: send_destroy,
                NDIlib_destroy: destroy,
            })
        }
    }
}
