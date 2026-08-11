pub struct NdiVideoFrame { 
    pub data: Vec<u8>, 
    pub width: i32, 
    pub height: i32, 
    pub stride: i32 
}

pub enum NdiPayload {
    Video(NdiVideoFrame),
    Audio(Vec<f32>),
}
