use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Graphics::Capture::{GraphicsCaptureItem, Direct3D11CaptureFramePool};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::core::{ComInterface, IInspectable};

pub struct WebviewSharedFrame {
    pub texture: Option<ID3D11Texture2D>,
}
impl Default for WebviewSharedFrame { fn default() -> Self { Self { texture: None } } }

pub fn spawn_receiver(hwnd_val: usize, rx_mutex: Arc<Mutex<WebviewSharedFrame>>, run_flag: Arc<AtomicBool>, d3d_device: ID3D11Device) {
    std::thread::spawn(move || {
        unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED).ok(); }
        let hwnd = HWND(hwnd_val as isize);
        
        // Interop HWND to GraphicsCaptureItem
        let factory: IGraphicsCaptureItemInterop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().unwrap();
        let item: GraphicsCaptureItem = unsafe { factory.CreateForWindow(hwnd) }.unwrap();
        let size = item.Size().unwrap();
        
        // Wrap D3D11 device into WinRT IDirect3DDevice
        let dxgi_device: windows::Win32::Graphics::Dxgi::IDXGIDevice = d3d_device.cast().unwrap();
        let inspectable: IInspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }.unwrap();
        let winrt_device: IDirect3DDevice = inspectable.cast().unwrap();

        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&winrt_device, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size).unwrap();
        let session = frame_pool.CreateCaptureSession(&item).unwrap();
        session.SetIsCursorCaptureEnabled(false).ok();
        session.SetIsBorderRequired(false).ok();
        session.StartCapture().unwrap();

        while run_flag.load(Ordering::Acquire) {
            if let Ok(frame) = frame_pool.TryGetNextFrame() {
                if let Ok(surface) = frame.Surface() {
                    if let Ok(access) = surface.cast::<IDirect3DDxgiInterfaceAccess>() {
                        if let Ok(d3d_tex) = unsafe { access.GetInterface::<ID3D11Texture2D>() } {
                            if let Ok(mut lock) = rx_mutex.lock() {
                                lock.texture = Some(d3d_tex.clone());
                            }
                        }
                    }
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(8)); // Yield roughly 120Hz
            }
        }
        session.Close().ok();
        frame_pool.Close().ok();
    });
}

