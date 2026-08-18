use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

use windows::core::Result;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::Media::Audio::{
    eCapture, eConsole, IMMDeviceEnumerator, MMDeviceEnumerator,
    IAudioClient, IAudioCaptureClient, WAVEFORMATEX,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_NOPERSIST, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};

pub static LTC_TIMECODE: AtomicU64 = AtomicU64::new(0);

pub fn start_ltc_capture_thread() {
    thread::spawn(move || {
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if let Err(e) = run_capture_loop() {
            println!("LTC Capture Error: {:?}", e);
        }
    });
}

fn run_capture_loop() -> Result<()> {
    unsafe {
        let enumerator: IMMDeviceEnumerator = windows::Win32::System::Com::CoCreateInstance(
            &MMDeviceEnumerator,
            None,
            windows::Win32::System::Com::CLSCTX_ALL,
        )?;

        let device = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)?;
        let audio_client: IAudioClient = device.Activate(windows::Win32::System::Com::CLSCTX_ALL, None)?;

        let mut format = WAVEFORMATEX::default();
        format.wFormatTag = 3; // WAVE_FORMAT_IEEE_FLOAT
        format.nChannels = 1;
        format.nSamplesPerSec = 48000;
        format.wBitsPerSample = 32;
        format.nBlockAlign = (format.nChannels * format.wBitsPerSample) / 8;
        format.nAvgBytesPerSec = format.nSamplesPerSec * format.nBlockAlign as u32;

        let res = audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_NOPERSIST | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
            0,
            0,
            &format,
            None,
        );

        if res.is_err() {
            return Ok(());
        }

        let event = CreateEventW(None, false, false, None)?;
        audio_client.SetEventHandle(event)?;
        audio_client.Start()?;

        let capture_client: IAudioCaptureClient = audio_client.GetService()?;
        
        let mut bit_buffer: u128 = 0;
        let mut last_sign = false;
        let mut samples_since_cross = 0;

        loop {
            WaitForSingleObject(event, INFINITE);

            let mut packet_length = 0;
            if let Ok(len) = capture_client.GetNextPacketSize() { packet_length = len; } else {
                continue;
            }

            while packet_length > 0 {
                let mut data_ptr = std::ptr::null_mut();
                let mut frames_available = 0;
                let mut flags = 0;

                capture_client.GetBuffer(
                    &mut data_ptr,
                    &mut frames_available,
                    &mut flags,
                    None,
                    None,
                )?;

                if frames_available > 0 {
                    let slice = std::slice::from_raw_parts(data_ptr as *const f32, frames_available as usize);
                    
                    for &sample in slice {
                        let current_sign = sample >= 0.0;
                        if current_sign != last_sign {
                            // Zero crossing
                            if samples_since_cross > 4 {
                                let bit: u128 = if samples_since_cross > 12 { 0 } else { 1 };
                                bit_buffer = (bit_buffer << 1) | bit;
                                
                                // SMPTE Sync word: 0011 1111 1111 1101 = 0x3FFD
                                if (bit_buffer & 0xFFFF) == 0x3FFD {
                                    // Parse timecode
                                    let frame_units = ((bit_buffer >> 16) & 0x0F) as u64;
                                    let frame_tens = ((bit_buffer >> 24) & 0x03) as u64;
                                    
                                    let sec_units = ((bit_buffer >> 32) & 0x0F) as u64;
                                    let sec_tens = ((bit_buffer >> 40) & 0x07) as u64;
                                    
                                    let min_units = ((bit_buffer >> 48) & 0x0F) as u64;
                                    let min_tens = ((bit_buffer >> 56) & 0x07) as u64;
                                    
                                    let hour_units = ((bit_buffer >> 64) & 0x0F) as u64;
                                    let hour_tens = ((bit_buffer >> 72) & 0x03) as u64;

                                    let ff = frame_tens * 10 + frame_units;
                                    let ss = sec_tens * 10 + sec_units;
                                    let mm = min_tens * 10 + min_units;
                                    let hh = hour_tens * 10 + hour_units;

                                    // Store tightly packed inside atomic u64
                                    let tc = (hh << 24) | (mm << 16) | (ss << 8) | ff;
                                    LTC_TIMECODE.store(tc, Ordering::Relaxed);
                                }
                            }
                            samples_since_cross = 0;
                            last_sign = current_sign;
                        } else {
                            samples_since_cross += 1;
                        }
                    }
                }
                capture_client.ReleaseBuffer(frames_available)?;
                if let Ok(len) = capture_client.GetNextPacketSize() { packet_length = len; }
            }
        }
    }
}
