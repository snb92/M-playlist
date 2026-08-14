use crate::graphics::DxgiSharedFrame;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::core::ComInterface;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGIOutput1, IDXGIResource, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT};

pub fn spawn_receiver(
    monitor_index: u8,
    rx_mutex: Arc<Mutex<DxgiSharedFrame>>,
    run_flag: Arc<AtomicBool>,
    device: ID3D11Device,
) {
    std::thread::spawn(move || {
        println!("M-Playlist [DXGI]: Desktop Duplication receiver started for monitor {}", monitor_index);
        unsafe {
            let dxgi_device: windows::core::Result<IDXGIDevice> = device.cast();
            if dxgi_device.is_err() {
                println!("M-Playlist [DXGI]: Failed to get IDXGIDevice.");
                return;
            }
            let dxgi_device = dxgi_device.unwrap();
            
            let dxgi_adapter = match dxgi_device.GetAdapter() {
                Ok(adapter) => adapter,
                Err(e) => {
                    println!("M-Playlist [DXGI]: Failed to get IDXGIAdapter: {:?}", e);
                    return;
                }
            };
            
            let output = match dxgi_adapter.EnumOutputs(monitor_index as u32) {
                Ok(out) => out,
                Err(e) => {
                    println!("M-Playlist [DXGI]: Failed to EnumOutputs (Monitor {}): {:?}", monitor_index, e);
                    return;
                }
            };
            
            let output1: windows::core::Result<IDXGIOutput1> = output.cast();
            if output1.is_err() {
                println!("M-Playlist [DXGI]: Output does not support IDXGIOutput1.");
                return;
            }
            let output1 = output1.unwrap();
            
            let mut duplication = match output1.DuplicateOutput(&device) {
                Ok(dup) => dup,
                Err(e) => {
                    println!("M-Playlist [DXGI]: Failed to DuplicateOutput: {:?}", e);
                    return;
                }
            };

            while run_flag.load(Ordering::Acquire) {
                let mut frame_info = windows::Win32::Graphics::Dxgi::DXGI_OUTDUPL_FRAME_INFO::default();
                let mut resource: Option<IDXGIResource> = None;
                
                let res = duplication.AcquireNextFrame(100, &mut frame_info, &mut resource);
                if let Err(e) = res {
                    if e.code() == DXGI_ERROR_ACCESS_LOST {
                        println!("M-Playlist [DXGI]: Access lost. Rebuilding Duplication interface...");
                        duplication = match output1.DuplicateOutput(&device) {
                            Ok(dup) => dup,
                            Err(_) => {
                                std::thread::sleep(std::time::Duration::from_millis(500));
                                continue;
                            }
                        };
                        continue;
                    } else if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                        continue; // No new frame yet
                    } else {
                        println!("M-Playlist [DXGI]: AcquireNextFrame error: {:?}", e);
                        std::thread::sleep(std::time::Duration::from_millis(16));
                        continue;
                    }
                }
                
                if let Some(res) = resource {
                    if let Ok(tex) = res.cast::<ID3D11Texture2D>() {
                        {
                            let mut lock = rx_mutex.lock().unwrap();
                            lock.texture = Some(tex.clone());
                        }
                        
                        // Spin-Lock Handoff
                        while run_flag.load(Ordering::Acquire) {
                            let has_texture = rx_mutex.lock().unwrap().texture.is_some();
                            if !has_texture {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                    }
                }
                
                let _ = duplication.ReleaseFrame();
            }
        }
        println!("M-Playlist [DXGI]: Desktop Duplication receiver stopped.");
    });
}
