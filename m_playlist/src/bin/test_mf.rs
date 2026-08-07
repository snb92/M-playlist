use std::os::windows::ffi::OsStrExt;
use windows::core::{ComInterface, PCWSTR};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED, CoUninitialize};
use windows::Win32::Media::MediaFoundation::{
    MFStartup, MFShutdown, MFCreateSourceReaderFromURL, IMFDXGIDeviceManager, MFCreateDXGIDeviceManager,
    MFCreateAttributes, IMFAttributes, MF_SOURCE_READER_D3D_MANAGER, MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING,
    MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, MF_VERSION
};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_1
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_CREATE_DEVICE_VIDEO_SUPPORT, ID3D11Device, ID3D11DeviceContext, D3D11_SDK_VERSION, ID3D11Multithread
};

fn main() {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok();
        MFStartup(MF_VERSION, 0).ok();

        let mut device_opt: Option<ID3D11Device> = None;
        let mut context_opt: Option<ID3D11DeviceContext> = None;
        
        D3D11CreateDevice(
            None, D3D_DRIVER_TYPE_HARDWARE, None,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_1]), D3D11_SDK_VERSION,
            Some(&mut device_opt), None, Some(&mut context_opt),
        ).unwrap();
        
        let device = device_opt.unwrap();
        let multithread: ID3D11Multithread = device.cast().unwrap();
        multithread.SetMultithreadProtected(true);

        let mut dxgi_manager_opt: Option<IMFDXGIDeviceManager> = None;
        let mut reset_token: u32 = 0;
        MFCreateDXGIDeviceManager(&mut reset_token, &mut dxgi_manager_opt).unwrap();
        let dxgi_manager = dxgi_manager_opt.unwrap();
        
        let iunknown_device: windows::core::IUnknown = device.cast().unwrap();
        dxgi_manager.ResetDevice(&iunknown_device, reset_token).unwrap();

        // TEST 1: Just the DXGI Manager
        {
            println!("Test 1: Only D3D Manager");
            let mut attributes_opt: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes_opt, 3).unwrap();
            let attributes = attributes_opt.unwrap();
            
            let iunknown_dxgi: windows::core::IUnknown = dxgi_manager.clone().cast().unwrap();
            attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &iunknown_dxgi).unwrap();
            
            let path_str = r"C:\Windows\Media\tada.wav"; // just some valid path
            let path_os = std::ffi::OsStr::new(path_str);
            let mut path_vec: Vec<u16> = path_os.encode_wide().collect();
            path_vec.push(0);
            let pcwstr_path = PCWSTR::from_raw(path_vec.as_ptr());
            
            match MFCreateSourceReaderFromURL(pcwstr_path, &attributes) {
                Ok(_) => println!("Test 1 OK"),
                Err(e) => println!("Test 1 Failed: {:?}", e),
            }
        }

        // TEST 2: DXGI Manager + ENABLE_VIDEO_PROCESSING
        {
            println!("Test 2: D3D Manager + ENABLE_VIDEO_PROCESSING");
            let mut attributes_opt: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes_opt, 3).unwrap();
            let attributes = attributes_opt.unwrap();
            
            let iunknown_dxgi: windows::core::IUnknown = dxgi_manager.clone().cast().unwrap();
            attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &iunknown_dxgi).unwrap();
            attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1).unwrap();
            
            let path_str = r"C:\Windows\Media\tada.wav"; // just some valid path
            let path_os = std::ffi::OsStr::new(path_str);
            let mut path_vec: Vec<u16> = path_os.encode_wide().collect();
            path_vec.push(0);
            let pcwstr_path = PCWSTR::from_raw(path_vec.as_ptr());
            
            match MFCreateSourceReaderFromURL(pcwstr_path, &attributes) {
                Ok(_) => println!("Test 2 OK"),
                Err(e) => println!("Test 2 Failed: {:?}", e),
            }
        }
        
        // TEST 3: DXGI Manager + ENABLE_ADVANCED_VIDEO_PROCESSING
        {
            println!("Test 3: D3D Manager + ENABLE_ADVANCED_VIDEO_PROCESSING");
            let mut attributes_opt: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes_opt, 3).unwrap();
            let attributes = attributes_opt.unwrap();
            
            let iunknown_dxgi: windows::core::IUnknown = dxgi_manager.clone().cast().unwrap();
            attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &iunknown_dxgi).unwrap();
            attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1).unwrap();
            
            let path_str = r"C:\Windows\Media\tada.wav"; // just some valid path
            let path_os = std::ffi::OsStr::new(path_str);
            let mut path_vec: Vec<u16> = path_os.encode_wide().collect();
            path_vec.push(0);
            let pcwstr_path = PCWSTR::from_raw(path_vec.as_ptr());
            
            match MFCreateSourceReaderFromURL(pcwstr_path, &attributes) {
                Ok(_) => println!("Test 3 OK"),
                Err(e) => println!("Test 3 Failed: {:?}", e),
            }
        }

        MFShutdown().ok();
        CoUninitialize();
    }
}
