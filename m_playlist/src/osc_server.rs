use std::net::UdpSocket;
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct OscServer {
    is_running: Arc<AtomicBool>,
    _thread: thread::JoinHandle<()>,
}

impl OscServer {
    pub fn start<F>(port: u16, on_fire_next: F) -> Result<Self, std::io::Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = is_running.clone();

        // Bind the UDP socket to all local network interfaces
        let address = format!("0.0.0.0:{}", port);
        let socket = UdpSocket::bind(&address)?;
        
        // Set a small read timeout so the thread can occasionally wake up and check if it should shut down gracefully
        socket.set_read_timeout(Some(std::time::Duration::from_millis(500)))?;

        println!("M-Playlist [OSC]: Listening for Show Control on UDP {}", port);

        let thread = thread::spawn(move || {
            let mut buf = [0u8; 1024]; // Max expected OSC packet size

            while is_running_clone.load(Ordering::Acquire) {
                match socket.recv_from(&mut buf) {
                    Ok((size, _src_addr)) => {
                        let payload = &buf[..size];
                        
                        // OSC strings are null-terminated and padded. 
                        // We do an extremely fast, zero-allocation sub-slice search for our exact command.
                        let command = b"/mplaylist/fire_next";

                        if payload.windows(command.len()).any(|window| window == command) {
                            println!("M-Playlist [OSC]: Received '/mplaylist/fire_next' from network!");
                            
                            // Trigger the Engine Callback!
                            on_fire_next();
                        }
                    }
                    Err(e) => {
                        // Ignore standard timeout errors, log actual socket failures
                        if e.kind() != std::io::ErrorKind::WouldBlock && e.kind() != std::io::ErrorKind::TimedOut {
                            eprintln!("M-Playlist [OSC Error]: {}", e);
                        }
                    }
                }
            }
            println!("M-Playlist [OSC]: Server shut down.");
        });

        Ok(Self {
            is_running,
            _thread: thread,
        })
    }
}

impl Drop for OscServer {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Release);
    }
}
