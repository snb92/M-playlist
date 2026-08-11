use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, mpsc::SyncSender};
use crate::ndi_transmitter::{NdiPayload, NdiVideoFrame};
use windows::core::{ComInterface, Result};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_1, ID3DBlob, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
    D3D_SRV_DIMENSION_TEXTURE2D, D3D_SRV_DIMENSION_TEXTURE2DARRAY,
};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, D3D11_BIND_CONSTANT_BUFFER,
    D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_WRITE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_FILTER_MIN_MAG_MIP_LINEAR,
    D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_WRITE_DISCARD, D3D11_SAMPLER_DESC,
    D3D11_SDK_VERSION, D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_TEXTURE2D_DESC,
    D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DYNAMIC,
    ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
    ID3D11PixelShader, ID3D11Resource, ID3D11SamplerState, ID3D11ShaderResourceView,
    ID3D11Texture2D, ID3D11VertexShader,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter, IDXGIDevice, IDXGIFactory2,
    IDXGISwapChain1,
};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

pub const VS_CODE: &[u8] = b"
struct VS_OUT { float4 pos : SV_POSITION; float2 uv : TEXCOORD; };
VS_OUT VS_Main(uint id : SV_VertexID) {
    VS_OUT output;
    output.uv = float2((id << 1) & 2, id & 2);
    output.pos = float4(output.uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return output;
}
\0";

pub const PS_CODE: &[u8] = b"
struct VS_OUT { float4 pos : SV_POSITION; float2 uv : TEXCOORD; };
Texture2D texA : register(t0);
Texture2D texB : register(t1);
SamplerState smp : register(s0);
cbuffer BlendBuffer : register(b0) { 
    float blendFactor; float aspectA; float aspectB; float aspectOut; 
    float4x4 invHomography;
};

float2 FitAspect(float2 uv, float texAspect, float outAspect) {
    if (texAspect <= 0.01 || outAspect <= 0.01) return uv;
    float scaleX = 1.0;
    float scaleY = 1.0;
    if (texAspect > outAspect) {
        scaleY = outAspect / texAspect;
    } else {
        scaleX = texAspect / outAspect;
    }
    return float2((uv.x - 0.5) / scaleX + 0.5, (uv.y - 0.5) / scaleY + 0.5);
}

float4 PS_Main(VS_OUT input) : SV_TARGET {
    // 1. Multiply the screen UV by the inverse 3D matrix
    float3 uvw = mul(float3(input.uv, 1.0), (float3x3)invHomography);
    
    // 2. Perform the true Perspective Divide
    float2 final_uv = uvw.xy / uvw.z;
    
    // 3. Render black outside the bounds of the warped quad
    if (final_uv.x < 0.0 || final_uv.x > 1.0 || final_uv.y < 0.0 || final_uv.y > 1.0) {
        return float4(0, 0, 0, 1);
    }
    
    float2 uvA = FitAspect(final_uv, aspectA, aspectOut);
    float4 colorA = float4(0, 0, 0, 1);
    if (uvA.x >= 0.0 && uvA.x <= 1.0 && uvA.y >= 0.0 && uvA.y <= 1.0) {
        colorA = texA.Sample(smp, uvA);
    }
    
    float2 uvB = FitAspect(final_uv, aspectB, aspectOut);
    float4 colorB = float4(0, 0, 0, 1);
    if (uvB.x >= 0.0 && uvB.x <= 1.0 && uvB.y >= 0.0 && uvB.y <= 1.0) {
        colorB = texB.Sample(smp, uvB);
    }
    
    return lerp(colorA, colorB, blendFactor);
}
\0";

#[repr(C, align(16))]
pub struct BlendData {
    pub blend_factor: f32,
    pub aspect_a: f32,
    pub aspect_b: f32,
    pub aspect_out: f32,
    pub inv_homography: [f32; 16],
}

fn calculate_inverse_homography(geometry: &[[f32; 4]; 4]) -> [f32; 16] {
    // Map NDC (-1 to 1) to UV Space (0 to 1)
    let map_x = |x: f32| (x + 1.0) / 2.0;
    let map_y = |y: f32| (1.0 - y) / 2.0;

    let x0 = map_x(geometry[0][0]); let y0 = map_y(geometry[0][1]); // TL
    let x1 = map_x(geometry[1][0]); let y1 = map_y(geometry[1][1]); // TR
    let x2 = map_x(geometry[3][0]); let y2 = map_y(geometry[3][1]); // BR (Cyclic order)
    let x3 = map_x(geometry[2][0]); let y3 = map_y(geometry[2][1]); // BL (Cyclic order)

    let dx1 = x1 - x2; 
    let dx2 = x3 - x2; 
    let sx = x0 - x1 + x2 - x3;
    
    let dy1 = y1 - y2; 
    let dy2 = y3 - y2; 
    let sy = y0 - y1 + y2 - y3;

    let det = dx1 * dy2 - dy1 * dx2;
    let (g, h) = if det.abs() < 0.0001 {
        (0.0, 0.0)
    } else {
        ((sx * dy2 - sy * dx2) / det, (sy * dx1 - sx * dy1) / det)
    };

    let a = x1 - x0 + g * x1;
    let b = x3 - x0 + h * x3;
    let c = x0;
    let d = y1 - y0 + g * y1;
    let e = y3 - y0 + h * y3;
    let f = y0;

    // Adjugate matrix (Inverse)
    let inv_a = e - f * h;
    let inv_b = c * h - b;
    let inv_c = b * f - c * e;
    let inv_d = f * g - d;
    let inv_e = a - c * g;
    let inv_f = c * d - a * f;
    let inv_g = d * h - e * g;
    let inv_h = b * g - a * h;
    let inv_i = a * e - b * d;

    [
        inv_a, inv_d, inv_g, 0.0,
        inv_b, inv_e, inv_h, 0.0,
        inv_c, inv_f, inv_i, 0.0,
        0.0,   0.0,   0.0,   1.0,
    ]
}

pub struct SendableSample(pub windows::Win32::Media::MediaFoundation::IMFSample);
unsafe impl Send for SendableSample {}
unsafe impl Sync for SendableSample {}

pub struct Dx11Compositor {
    pub device: ID3D11Device,
    context: ID3D11DeviceContext,
    swapchain: IDXGISwapChain1,
    pub width: AtomicU32,
    pub height: AtomicU32,
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    sampler: ID3D11SamplerState,
    constant_buffer: ID3D11Buffer,
    pub staging_a: Mutex<Option<(ID3D11Texture2D, u32, SendableSample)>>,
    pub staging_b: Mutex<Option<(ID3D11Texture2D, u32, SendableSample)>>,
    pub ndi_staging_textures: Mutex<[Option<ID3D11Texture2D>; 3]>,
    pub ndi_staging_index: Mutex<usize>,
    pub frame_count: AtomicU64,
    pub ndi_tx: Mutex<Option<SyncSender<NdiPayload>>>,
    pub video_grave_rx: Mutex<Option<std::sync::mpsc::Receiver<Vec<u8>>>>,
    pub master_rtv: windows::Win32::Graphics::Direct3D11::ID3D11RenderTargetView,
    pub d2d_factory: windows::Win32::Graphics::Direct2D::ID2D1Factory,
    pub d2d_render_target: windows::Win32::Graphics::Direct2D::ID2D1RenderTarget,
    pub dwrite_factory: windows::Win32::Graphics::DirectWrite::IDWriteFactory,
    pub text_format: windows::Win32::Graphics::DirectWrite::IDWriteTextFormat,
    pub text_brush: windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
}

impl Dx11Compositor {
    pub fn new(hwnd_ptr: *mut std::ffi::c_void) -> Result<Self> {
        unsafe {
            let hwnd = HWND(hwnd_ptr as _);
            
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect).expect("Failed to get window client rect");
            let width = (rect.right - rect.left) as u32;
            let height = (rect.bottom - rect.top) as u32;

            let mut d3d11_device_opt: Option<ID3D11Device> = None;
            let mut d3d11_context_opt: Option<ID3D11DeviceContext> = None;
            
            D3D11CreateDevice(
                None, D3D_DRIVER_TYPE_HARDWARE, None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_1]), D3D11_SDK_VERSION,
                Some(&mut d3d11_device_opt), None, Some(&mut d3d11_context_opt),
            )?;
            
            let device = d3d11_device_opt.unwrap();
            let context = d3d11_context_opt.unwrap();

            if let Ok(d3d11_multithread) = device.cast::<ID3D11Multithread>() {
                d3d11_multithread.SetMultithreadProtected(true);
            } else if let Ok(d3d10_multithread) = device.cast::<windows::Win32::Graphics::Direct3D10::ID3D10Multithread>() {
                d3d10_multithread.SetMultithreadProtected(true);
            }

            let dxgi_device: IDXGIDevice = device.cast()?;
            let dxgi_adapter: IDXGIAdapter = dxgi_device.GetAdapter()?;
            let dxgi_factory: IDXGIFactory2 = dxgi_adapter.GetParent()?;

            let swapchain_desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: 1920, 
                Height: 1080,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE, 
                Flags: 0,
            };

            let swapchain = dxgi_factory.CreateSwapChainForHwnd(&device, hwnd, &swapchain_desc, None, None)?;

            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: 1920,
                Height: 1080,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: windows::Win32::Graphics::Direct3D11::D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut tex1_opt: Option<ID3D11Texture2D> = None;
            device.CreateTexture2D(&staging_desc, None, Some(&mut tex1_opt))?;
            let mut tex2_opt: Option<ID3D11Texture2D> = None;
            device.CreateTexture2D(&staging_desc, None, Some(&mut tex2_opt))?;
            let mut tex3_opt: Option<ID3D11Texture2D> = None;
            device.CreateTexture2D(&staging_desc, None, Some(&mut tex3_opt))?;
            let ndi_staging_textures = Mutex::new([tex1_opt, tex2_opt, tex3_opt]);

            // Shaders
            let mut vs_blob_opt: Option<ID3DBlob> = None;
            let mut error_blob_opt: Option<ID3DBlob> = None;
            let res = D3DCompile(
                VS_CODE.as_ptr() as *const _, VS_CODE.len() - 1,
                windows::core::s!("VS_Main"), None, None,
                windows::core::s!("VS_Main"), windows::core::s!("vs_5_0"),
                0, 0, &mut vs_blob_opt, Some(&mut error_blob_opt),
            );
            if res.is_err() {
                if let Some(err_blob) = error_blob_opt {
                    let err_str = std::ffi::CStr::from_ptr(err_blob.GetBufferPointer() as *const i8).to_string_lossy();
                    panic!("VS Compile Error: {}", err_str);
                } else {
                    res?;
                }
            }
            let vs_blob = vs_blob_opt.unwrap();
            
            let mut ps_blob_opt: Option<ID3DBlob> = None;
            let mut error_blob_opt2: Option<ID3DBlob> = None;
            let res2 = D3DCompile(
                PS_CODE.as_ptr() as *const _, PS_CODE.len() - 1,
                windows::core::s!("PS_Main"), None, None,
                windows::core::s!("PS_Main"), windows::core::s!("ps_5_0"),
                0, 0, &mut ps_blob_opt, Some(&mut error_blob_opt2),
            );
            if res2.is_err() {
                if let Some(err_blob) = error_blob_opt2 {
                    let err_str = std::ffi::CStr::from_ptr(err_blob.GetBufferPointer() as *const i8).to_string_lossy();
                    panic!("PS Compile Error: {}", err_str);
                } else {
                    res2?;
                }
            }
            let ps_blob = ps_blob_opt.unwrap();

            let mut vertex_shader_opt: Option<ID3D11VertexShader> = None;
            device.CreateVertexShader(
                std::slice::from_raw_parts(vs_blob.GetBufferPointer() as *const u8, vs_blob.GetBufferSize()),
                None, Some(&mut vertex_shader_opt),
            )?;
            let vertex_shader = vertex_shader_opt.unwrap();

            let mut pixel_shader_opt: Option<ID3D11PixelShader> = None;
            device.CreatePixelShader(
                std::slice::from_raw_parts(ps_blob.GetBufferPointer() as *const u8, ps_blob.GetBufferSize()),
                None, Some(&mut pixel_shader_opt),
            )?;
            let pixel_shader = pixel_shader_opt.unwrap();

            let sampler_desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                ComparisonFunc: windows::Win32::Graphics::Direct3D11::D3D11_COMPARISON_NEVER,
                MaxLOD: std::f32::MAX,
                ..Default::default()
            };
            let mut sampler_opt: Option<ID3D11SamplerState> = None;
            device.CreateSamplerState(&sampler_desc, Some(&mut sampler_opt))?;
            let sampler = sampler_opt.unwrap();

            let cb_desc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of::<BlendData>() as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                ..Default::default()
            };
            let mut constant_buffer_opt: Option<ID3D11Buffer> = None;
            device.CreateBuffer(&cb_desc, None, Some(&mut constant_buffer_opt))?;
            let constant_buffer = constant_buffer_opt.unwrap();

            // 2. Extract 1080p Backbuffer and Create RTV ONCE
            let backbuffer: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D = swapchain.GetBuffer(0).unwrap();
            let dest_res: windows::Win32::Graphics::Direct3D11::ID3D11Resource = backbuffer.cast().unwrap();
            let mut rtv_opt = None;
            device.CreateRenderTargetView(&dest_res, None, Some(&mut rtv_opt)).unwrap();
            let master_rtv = rtv_opt.unwrap();

            // 3. Initialize Direct2D & DirectWrite Factories
            let d2d_factory: windows::Win32::Graphics::Direct2D::ID2D1Factory = windows::Win32::Graphics::Direct2D::D2D1CreateFactory(
                windows::Win32::Graphics::Direct2D::D2D1_FACTORY_TYPE_MULTI_THREADED,
                None,
            ).unwrap();

            let dwrite_factory: windows::Win32::Graphics::DirectWrite::IDWriteFactory = windows::Win32::Graphics::DirectWrite::DWriteCreateFactory(
                windows::Win32::Graphics::DirectWrite::DWRITE_FACTORY_TYPE_SHARED,
            ).unwrap();

            // 4. Create Direct2D Render Target (Wrapped natively around DX11 Backbuffer)
            let dxgi_surface: windows::Win32::Graphics::Dxgi::IDXGISurface = backbuffer.cast().unwrap();
            let render_props = windows::Win32::Graphics::Direct2D::D2D1_RENDER_TARGET_PROPERTIES {
                r#type: windows::Win32::Graphics::Direct2D::D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: windows::Win32::Graphics::Direct2D::Common::D2D1_PIXEL_FORMAT {
                    format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM, // Matches BGRA_SUPPORT
                    alphaMode: windows::Win32::Graphics::Direct2D::Common::D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };

            let d2d_render_target = d2d_factory.CreateDxgiSurfaceRenderTarget(&dxgi_surface, &render_props).unwrap();

            // 5. Create Typography Resources
            let text_brush = {
                let color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
                d2d_render_target.CreateSolidColorBrush(&color as *const _, None).unwrap()
            };

            let text_format = dwrite_factory.CreateTextFormat(
                windows::core::w!("Consolas"),
                None,
                windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT_BOLD,
                windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STYLE_NORMAL,
                windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STRETCH_NORMAL,
                64.0, // Broadcast-sized typography
                windows::core::w!("en-US"),
            ).unwrap();

            Ok(Self { 
                device, 
                context, 
                swapchain, 
                width: AtomicU32::new(width), 
                height: AtomicU32::new(height),
                vertex_shader,
                pixel_shader,
                sampler,
                constant_buffer,
                staging_a: Mutex::new(None),
                staging_b: Mutex::new(None),
                ndi_staging_textures,
                ndi_staging_index: Mutex::new(0),
                frame_count: AtomicU64::new(0),
                ndi_tx: Mutex::new(None),
                video_grave_rx: Mutex::new(None),
                master_rtv,
                d2d_factory,
                d2d_render_target,
                dwrite_factory,
                text_format,
                text_brush,
            })
        }
    }

    pub fn update_deck_texture(&self, deck_id: u8, src_texture: &ID3D11Texture2D, subresource_index: u32, sample: &windows::Win32::Media::MediaFoundation::IMFSample) -> Result<()> {
        let staging_mutex = if deck_id == 0 { &self.staging_a } else { &self.staging_b };
        if let Ok(mut staging_lock) = staging_mutex.lock() {
            // TRUE ZERO-COPY: Hold the COM reference and the specific slice index!
            // We MUST also hold the IMFSample reference so MF doesn't overwrite this slice in its pool!
            *staging_lock = Some((src_texture.clone(), subresource_index, SendableSample(sample.clone())));
        }
        Ok(())
    }

    pub fn clear_deck(&self, deck_id: u8) {
        let staging_mutex = if deck_id == 0 { &self.staging_a } else { &self.staging_b };
        if let Ok(mut staging_lock) = staging_mutex.lock() {
            *staging_lock = None;
        }
    }

    pub fn render_composited(&self, blend_factor: f32, geometry: &[[f32; 4]; 4]) -> Result<()> {
        unsafe {
            let backbuffer: ID3D11Texture2D = self.swapchain.GetBuffer(0)?;
            let current_w = 1920;
            let current_h = 1080;

            // 1. Static 1080p Viewport & RTV Binding
            let viewport = windows::Win32::Graphics::Direct3D11::D3D11_VIEWPORT { 
                TopLeftX: 0.0, TopLeftY: 0.0, Width: 1920.0, Height: 1080.0, MinDepth: 0.0, MaxDepth: 1.0 
            };
            self.context.RSSetViewports(Some(&[viewport]));

            let clear_color: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
            self.context.ClearRenderTargetView(&self.master_rtv, &clear_color);
            self.context.OMSetRenderTargets(Some(&[Some(self.master_rtv.clone())]), None);


            self.context.IASetPrimitiveTopology(windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.context.VSSetShader(&self.vertex_shader, None);
            self.context.PSSetShader(&self.pixel_shader, None);
            self.context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));

            let lock_a = self.staging_a.lock().unwrap();
            let lock_b = self.staging_b.lock().unwrap();

            let get_aspect = |tex_opt: &Option<(ID3D11Texture2D, u32, SendableSample)>| -> f32 {
                if let Some((tex, _, _)) = tex_opt {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    tex.GetDesc(&mut desc);
                    if desc.Height > 0 {
                        return desc.Width as f32 / desc.Height as f32;
                    }
                }
                1.0
            };

            let aspect_a = get_aspect(&*lock_a);
            let aspect_b = get_aspect(&*lock_b);
            let aspect_out = if current_h > 0 { current_w as f32 / current_h as f32 } else { 1.0 };

            let blend_data = BlendData {
                blend_factor, aspect_a, aspect_b, aspect_out,
                inv_homography: calculate_inverse_homography(geometry),
            };
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(&self.constant_buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
            std::ptr::copy_nonoverlapping(&blend_data as *const _ as *const u8, mapped.pData as *mut u8, std::mem::size_of::<BlendData>());
            self.context.Unmap(&self.constant_buffer, 0);
            
            self.context.VSSetConstantBuffers(0, Some(&[Some(self.constant_buffer.clone())]));
            self.context.PSSetConstantBuffers(0, Some(&[Some(self.constant_buffer.clone())]));

            let create_srv = |tex_opt: &Option<(ID3D11Texture2D, u32, SendableSample)>| -> Result<Option<ID3D11ShaderResourceView>> {
                if let Some((tex, subresource, _)) = tex_opt {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    tex.GetDesc(&mut desc);

                    let mut srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                        Format: desc.Format,
                        ..Default::default()
                    };

                    if desc.ArraySize > 1 {
                        srv_desc.ViewDimension = D3D_SRV_DIMENSION_TEXTURE2DARRAY;
                        srv_desc.Anonymous.Texture2DArray = windows::Win32::Graphics::Direct3D11::D3D11_TEX2D_ARRAY_SRV {
                            MostDetailedMip: 0,
                            MipLevels: 1,
                            FirstArraySlice: *subresource,
                            ArraySize: 1,
                        };
                    } else {
                        srv_desc.ViewDimension = D3D_SRV_DIMENSION_TEXTURE2D;
                        srv_desc.Anonymous.Texture2D = windows::Win32::Graphics::Direct3D11::D3D11_TEX2D_SRV {
                            MostDetailedMip: 0,
                            MipLevels: 1,
                        };
                    }
                    let mut srv_opt: Option<ID3D11ShaderResourceView> = None;
                    let res: ID3D11Resource = tex.cast()?;
                    self.device.CreateShaderResourceView(&res, Some(&srv_desc), Some(&mut srv_opt))?;
                    Ok(srv_opt)
                } else {
                    Ok(None)
                }
            };


            // Better way:
            let srv_a_opt = create_srv(&*lock_a)?;
            let srv_b_opt = create_srv(&*lock_b)?;

            let srvs = [srv_a_opt.clone(), srv_b_opt.clone()];
            self.context.PSSetShaderResources(0, Some(&srvs));

            self.context.Draw(3, 0);

            // 3. DIRECT2D TYPOGRAPHY PASS (Hardware Accelerated over VRAM)
            self.d2d_render_target.BeginDraw();
            
            let text: Vec<u16> = "M-PLAYLIST BROADCAST CORE: 1080p/60 DIRECT2D ACTIVE".encode_utf16().collect();
            let text_rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F { 
                left: 50.0, top: 50.0, right: 1870.0, bottom: 200.0 
            };
            
            self.d2d_render_target.DrawText(
                &text,
                &self.text_format,
                &text_rect,
                &self.text_brush,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
            
            self.d2d_render_target.EndDraw(None, None).unwrap();

            // STRIKE 2: The CopyResource & Delayed Map Pipeline
            let frame = self.frame_count.load(Ordering::Relaxed);
            let mut ndi_staging_lock = self.ndi_staging_textures.lock().unwrap();
            let mut index_lock = self.ndi_staging_index.lock().unwrap();
            let current_index = *index_lock;
            let tx_opt = self.ndi_tx.lock().unwrap();

            // Hardware Copy
            let target_staging_opt = &ndi_staging_lock[current_index];
            if let Some(target_staging) = target_staging_opt {
                let dest_staging_res: ID3D11Resource = target_staging.cast()?;
                let src_backbuffer_res: ID3D11Resource = backbuffer.cast()?;
                self.context.CopyResource(&dest_staging_res, &src_backbuffer_res);
            }

            // Map Oldest
            let read_index = (current_index + 1) % 3;
            if frame >= 2 {
                let mapped_tex_opt = &ndi_staging_lock[read_index];
                if let Some(mapped_tex) = mapped_tex_opt {
                    let mapped_res: ID3D11Resource = mapped_tex.cast()?;
                    let mut mapped_subresource = D3D11_MAPPED_SUBRESOURCE::default();
                    
                    let map_result = self.context.Map(
                        &mapped_res, 
                        0, 
                        windows::Win32::Graphics::Direct3D11::D3D11_MAP_READ, 
                        0, 
                        Some(&mut mapped_subresource)
                    );

                    if map_result.is_ok() {
                        let total_bytes = (mapped_subresource.RowPitch * current_h) as usize;
                        
                        let mut buffer = {
                            let mut rx_lock = self.video_grave_rx.lock().unwrap();
                            if let Some(rx) = rx_lock.as_ref() {
                                rx.try_recv().unwrap_or_else(|_| Vec::with_capacity(total_bytes))
                            } else {
                                Vec::with_capacity(total_bytes)
                            }
                        };
                        
                        if buffer.capacity() < total_bytes {
                            buffer.reserve(total_bytes - buffer.capacity());
                        }

                        unsafe {
                            buffer.set_len(total_bytes);
                            std::ptr::copy_nonoverlapping(mapped_subresource.pData as *const u8, buffer.as_mut_ptr(), total_bytes);
                        }
                        
                        self.context.Unmap(&mapped_res, 0);
                        
                        if crate::ffi::NDI_ENABLED.load(Ordering::Relaxed) {
                            if let Some(tx) = tx_opt.as_ref() {
                                let ndi_frame = NdiVideoFrame { 
                                    data: buffer, 
                                    width: current_w as i32, 
                                    height: current_h as i32, 
                                    stride: mapped_subresource.RowPitch as i32 
                                };
                                let _ = tx.try_send(NdiPayload::Video(ndi_frame));
                            }
                        }
                    }
                }
            }

            // Advance
            *index_lock = (current_index + 1) % 3;
            self.frame_count.fetch_add(1, Ordering::Relaxed);

            // Unbind to release references safely
            self.context.PSSetShaderResources(0, Some(&[None, None]));
            self.context.OMSetRenderTargets(None, None);

            let _ = self.swapchain.Present(1, 0); 
        }
        Ok(())
    }

    pub fn resize(&self, _new_width: u32, _new_height: u32) -> Result<()> {
        // No-op: Lock the swapchain to 1080p
        Ok(())
    }
}
