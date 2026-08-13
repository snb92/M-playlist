use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Imaging::*;
use windows::core::{Result, PCWSTR};

pub fn load_image_to_texture(
    device: &ID3D11Device,
    file_path: *const u16,
) -> Result<ID3D11Texture2D> {
    unsafe {
        // 1. Initialize WIC Factory
        let factory: IWICImagingFactory = windows::Win32::System::Com::CoCreateInstance(
            &CLSID_WICImagingFactory,
            None,
            windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
        )?;

        // 2. Decode the specific file
        let decoder = factory.CreateDecoderFromFilename(
            PCWSTR(file_path),
            None,
            windows::Win32::Foundation::GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = decoder.GetFrame(0)?;

        // 3. Force mathematical alignment to the swapchain (PBGRA)
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppPBGRA, 
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;

        let mut width = 0;
        let mut height = 0;
        converter.GetSize(&mut width, &mut height)?;
        
        let stride = width * 4;
        let buffer_size = stride * height;
        let mut pixels = vec![0u8; buffer_size as usize];
        converter.CopyPixels(std::ptr::null(), stride, &mut pixels)?;

        // 4. Blast into Immutable VRAM
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM, // Matches PBGRA exactly
            SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_IMMUTABLE,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            ..Default::default()
        };

        let init_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: pixels.as_ptr() as *const _,
            SysMemPitch: stride,
            SysMemSlicePitch: buffer_size,
        };

        let mut texture: Option<ID3D11Texture2D> = None;
        device.CreateTexture2D(&desc, Some(&init_data), Some(&mut texture))?;
        
        Ok(texture.unwrap()) // The Vec<u8> immediately drops here, preventing system RAM bloat.
    }
}
