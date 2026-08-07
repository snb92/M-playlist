use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// A lock-free, zero-allocation ring buffer for passing real-time audio.
pub struct AudioRingBuffer {
    buffer: Vec<AtomicU32>,
    head: AtomicUsize,
    tail: AtomicUsize,
    capacity: usize,
}

impl AudioRingBuffer {
    pub fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(AtomicU32::new(0)); // Pre-allocate on the heap
        }
        
        Self {
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            capacity,
        }
    }
    /// Instantly flushes the buffer (Used during hot-swaps to prevent audio overlap)
    pub fn clear(&self) {
        let current_head = self.head.load(std::sync::atomic::Ordering::Acquire);
        self.tail.store(current_head, std::sync::atomic::Ordering::Release);
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
