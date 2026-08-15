use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// A lock-free, zero-allocation ring buffer for passing real-time audio.
pub struct AudioRingBuffer {
    buffer: Vec<AtomicU32>,
    head: AtomicUsize,
    tail: AtomicUsize,
    capacity: usize,
    pub routing_matrix: [AtomicU32; 256],
    pub routing_offset: std::sync::atomic::AtomicU8,
}

impl AudioRingBuffer {
    pub fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(AtomicU32::new(0)); // Pre-allocate on the heap
        }
        
        let matrix = std::array::from_fn(|i| {
            let in_ch = i / 16;
            let out_bus = i % 16;
            // Default Identity Diagonal: In 0 -> Out 0, In 1 -> Out 1
            let default_gain = if in_ch == out_bus && in_ch < 2 { 1.0f32 } else { 0.0f32 };
            AtomicU32::new(default_gain.to_bits())
        });

        Self {
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            capacity,
            routing_matrix: matrix,
            routing_offset: std::sync::atomic::AtomicU8::new(0),
        }
    }

    pub fn set_route(&self, in_ch: usize, out_bus: usize, gain_db: f32) {
        if in_ch < 16 && out_bus < 16 {
            // Linear conversion: -100dB evaluates to absolute silence (0.0)
            let gain_linear = if gain_db <= -100.0 { 0.0 } else { 10.0f32.powf(gain_db / 20.0) };
            self.routing_matrix[in_ch * 16 + out_bus].store(gain_linear.to_bits(), std::sync::atomic::Ordering::Relaxed);
        }
    }
    /// Instantly flushes the buffer (Used during hot-swaps to prevent audio overlap)
    pub fn clear(&self) {
        let current_head = self.head.load(std::sync::atomic::Ordering::Acquire);
        self.tail.store(current_head, std::sync::atomic::Ordering::Release);
    }

    pub fn flush(&self) {
        let current_write = self.head.load(std::sync::atomic::Ordering::Relaxed);
        self.tail.store(current_write, std::sync::atomic::Ordering::Release);
    }

    pub fn get_occupancy(&self) -> usize {
        let w = self.head.load(std::sync::atomic::Ordering::Relaxed);
        let r = self.tail.load(std::sync::atomic::Ordering::Relaxed);
        
        if w >= r {
            w - r
        } else {
            self.capacity - r + w
        }
    }

    pub fn get_capacity(&self) -> usize {
        self.capacity
    }

    pub fn push(&self, sample: f32) -> Result<(), &'static str> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        
        let next_head = (head + 1) % self.capacity;
        if next_head == tail {
            return Err("Audio Buffer Overflow");
        }

        self.buffer[head].store(sample.to_bits(), Ordering::Relaxed);
        self.head.store(next_head, Ordering::Release);
        Ok(())
    }

    pub fn pop(&self) -> Option<f32> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if tail == head {
            return None; // Buffer underflow
        }

        let bits = self.buffer[tail].load(Ordering::Relaxed);
        self.tail.store((tail + 1) % self.capacity, Ordering::Release);
        
        Some(f32::from_bits(bits))
    }
}
