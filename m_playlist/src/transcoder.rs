use windows::core::{Result, PCWSTR, GUID, ComInterface};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::Media::MediaFoundation::{
    MFCreateSourceReaderFromURL, IMFSourceReader,
    MFCreateSinkWriterFromURL, IMFSinkWriter,
    MFCreateMediaType, IMFMediaType,
    MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MFMediaType_Video, MFMediaType_Audio,
    MFVideoFormat_H264, MFAudioFormat_AAC,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READER_FIRST_AUDIO_STREAM,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_PIXEL_ASPECT_RATIO,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE,
    MFVideoInterlace_Progressive, MF_SOURCE_READER_ALL_STREAMS
};

pub fn transcode_file(in_path: &str, out_path: &str) -> Result<()> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        
        let mut in_u16: Vec<u16> = in_path.encode_utf16().collect();
        in_u16.push(0);
        let mut out_u16: Vec<u16> = out_path.encode_utf16().collect();
        out_u16.push(0);
        
        let reader: IMFSourceReader = MFCreateSourceReaderFromURL(PCWSTR(in_u16.as_ptr()), None)?;
        let writer: IMFSinkWriter = MFCreateSinkWriterFromURL(PCWSTR(out_u16.as_ptr()), None, None)?;
        
        // Setup Video
        let mut video_out_idx = 0;
        let mut audio_out_idx = 0;
        let mut has_video = false;
        let mut has_audio = false;
        
        if let Ok(native_video_mt) = reader.GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0) {
            let out_video_mt: IMFMediaType = MFCreateMediaType()?;
            out_video_mt.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            out_video_mt.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            
            // Extract from native
            
            let size = native_video_mt.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(((1920_u64) << 32) | 1080_u64);
            out_video_mt.SetUINT64(&MF_MT_FRAME_SIZE, size)?;
            
            let ratio = native_video_mt.GetUINT64(&MF_MT_FRAME_RATE).unwrap_or(((60000_u64) << 32) | 1000_u64);
            out_video_mt.SetUINT64(&MF_MT_FRAME_RATE, ratio)?;
            
            out_video_mt.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            out_video_mt.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, ((1_u64) << 32) | 1_u64)?;

            
            video_out_idx = writer.AddStream(&out_video_mt)?;
            writer.SetInputMediaType(video_out_idx, &native_video_mt, None)?;
            has_video = true;
        }

        if let Ok(native_audio_mt) = reader.GetNativeMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, 0) {
            let out_audio_mt: IMFMediaType = MFCreateMediaType()?;
            out_audio_mt.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            out_audio_mt.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
            out_audio_mt.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
            out_audio_mt.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
            out_audio_mt.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            
            audio_out_idx = writer.AddStream(&out_audio_mt)?;
            writer.SetInputMediaType(audio_out_idx, &native_audio_mt, None)?;
            has_audio = true;
        }

        writer.BeginWriting()?;
        
        loop {
            let mut stream_index = 0;
            let mut flags = 0;
            let mut timestamp = 0;
            let mut sample_opt = None;
            
            reader.ReadSample(
                MF_SOURCE_READER_ALL_STREAMS.0 as u32,
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
                if has_video && stream_index == MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32 {
                    writer.WriteSample(video_out_idx, &sample)?;
                } else if has_audio && stream_index == MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32 {
                    writer.WriteSample(audio_out_idx, &sample)?;
                }
            }
        }
        
        writer.Finalize()?;
        Ok(())
    }
}
