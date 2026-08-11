use std::sync::atomic::{AtomicU64, Ordering};

/// The absolute source of truth for time in the engine.
/// Driven exclusively by the physical quartz crystal of the audio DAC.
pub struct MasterClock {
    audio_frames_played: AtomicU64,
    sample_rate: u32,
    pub is_paused: std::sync::atomic::AtomicBool,
}

impl MasterClock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            audio_frames_played: AtomicU64::new(0),
            sample_rate,
            is_paused: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn add_frames(&self, frames: u64) {
        if !self.is_paused.load(Ordering::Acquire) {
            self.audio_frames_played.fetch_add(frames, Ordering::Release);
        }
    }

    pub fn get_time_seconds(&self) -> f64 {
        let frames = self.audio_frames_played.load(Ordering::Acquire);
        frames as f64 / self.sample_rate as f64
    }

    pub fn overwrite_time(&self, hnsecs: i64) {
        let new_audio_frames = (hnsecs * 48_000) / 10_000_000;
        self.audio_frames_played.store(new_audio_frames as u64, Ordering::SeqCst);
    }
}
