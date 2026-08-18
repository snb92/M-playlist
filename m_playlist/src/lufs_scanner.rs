use windows::core::{Result, PCWSTR};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::Media::MediaFoundation::{
    MFCreateSourceReaderFromURL, IMFSourceReader,
    MFCreateAttributes, IMFAttributes,
    MFCreateMediaType, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MFMediaType_Audio, MF_SOURCE_READER_FIRST_AUDIO_STREAM,
    MF_SOURCE_READERF_ENDOFSTREAM, MFAudioFormat_Float
};

pub fn calculate_lufs(filepath: &str) -> Result<f32> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        
        let mut path_u16: Vec<u16> = filepath.encode_utf16().collect();
        path_u16.push(0);
        
        let attrs: IMFAttributes = {
            let mut a: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut a, 1)?;
            a.unwrap()
        };
        
        let reader: IMFSourceReader = MFCreateSourceReaderFromURL(
            PCWSTR(path_u16.as_ptr()),
            &attrs
        )?;
        
        let media_type = MFCreateMediaType()?;
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        media_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)?;
        
        reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, None, &media_type)?;
        
        reader.SetStreamSelection(windows::Win32::Media::MediaFoundation::MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        reader.SetStreamSelection(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, true)?;

        let mut total_sq: f64 = 0.0;
        let mut total_samples: u64 = 0;

        loop {
            let mut stream_index = 0;
            let mut flags = 0;
            let mut timestamp = 0;
            let mut sample_opt = None;
            
            reader.ReadSample(
                MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32,
                0,
                Some(&mut stream_index),
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample_opt)
            )?;

            if (flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
                break;
            }

            if let Some(sample) = sample_opt {
                let buffer = sample.ConvertToContiguousBuffer()?;
                let mut ptr = std::ptr::null_mut();
                let mut current_length = 0;
                let mut max_length = 0;
                
                buffer.Lock(&mut ptr, Some(&mut max_length), Some(&mut current_length))?;
                
                let num_floats = (current_length / 4) as usize;
                let float_slice = std::slice::from_raw_parts(ptr as *const f32, num_floats);
                
                for &f in float_slice {
                    total_sq += (f as f64) * (f as f64);
                }
                total_samples += num_floats as u64;
                
                buffer.Unlock()?;
            }
        }
        
        if total_samples == 0 {
            return Ok(0.0);
        }
        
        let rms = (total_sq / total_samples as f64).sqrt();
        let mut db = 20.0 * rms.log10();
        if db < -100.0 { db = -100.0; }
        
        let target_lufs = -14.0;
        let offset = target_lufs - db;
        
        Ok(offset as f32)
    }
}
