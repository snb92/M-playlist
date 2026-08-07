#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

pub const NDILIB_FOURCC_VIDEO_TYPE_BGRA: u32 = 0x41524742;

#[repr(C)]
pub struct NDIlib_video_frame_v2_t {
    pub xres: i32,
    pub yres: i32,
    pub FourCC: u32,
    pub frame_rate_N: i32,
    pub frame_rate_D: i32,
    pub picture_aspect_ratio: f32,
    pub frame_format_type: u32, // NDIlib_frame_format_type_progressive = 1
    pub timecode: i64, 
    pub p_data: *const u8,
    pub line_stride_in_bytes: i32,
    pub p_metadata: *const std::ffi::c_char,
    pub timestamp: i64,
}

#[repr(C)]
pub struct NDIlib_send_create_t {
    pub p_ndi_name: *const std::ffi::c_char,
    pub p_groups: *const std::ffi::c_char,
    pub clock_video: u8,
    pub clock_audio: u8,
}

pub type NDIlib_initialize_fn = unsafe extern "C" fn() -> u8;
pub type NDIlib_send_create_fn = unsafe extern "C" fn(p_create_settings: *const NDIlib_send_create_t) -> *mut std::ffi::c_void;
pub type NDIlib_send_send_video_v2_fn = unsafe extern "C" fn(p_instance: *mut std::ffi::c_void, p_video_data: *const NDIlib_video_frame_v2_t);
pub type NDIlib_send_destroy_fn = unsafe extern "C" fn(p_instance: *mut std::ffi::c_void);
pub type NDIlib_destroy_fn = unsafe extern "C" fn();
