use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, mpsc::SyncSender};
use crate::ndi_transmitter::NdiFrame;
use windows::core::{ComInterface, Result};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_1, ID3DBlob, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
    D3D_SRV_DIMENSION_TEXTURE2D,
};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_SHADER_RESOURCE,
    D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_WRITE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_FILTER_MIN_MAG_MIP_LINEAR,
    D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_WRITE_DISCARD, D3D11_SAMPLER_DESC,
    D3D11_SDK_VERSION, D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_TEXTURE2D_DESC,
    D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DYNAMIC,
    ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
    ID3D11PixelShader, ID3D11Resource, ID3D11SamplerState, ID3D11ShaderResourceView,
    ID3D11Texture2D, ID3D11VertexShader,
    D3D11_BIND_RENDER_TARGET,
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
    output.pos = float4(output.uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return output;
}
\0";

pub const PS_CODE: &[u8] = b"
struct VS_OUT { float4 pos : SV_POSITION; float2 uv : TEXCOORD; };
Texture2D texA : register(t0);
Texture2D texB : register(t1);
SamplerState smp : register(s0);
cbuffer BlendBuffer : register(b0) { float blendFactor; float aspectA; float aspectB; float aspectOut; };

float2 adjust_uv(float2 uv, float texAspect) {
    if (texAspect <= 0.0) return uv;
    float2 new_uv = uv;
    if (texAspect > aspectOut) { // Letterbox
        float ratio = aspectOut / texAspect;
        new_uv.y = (uv.y - 0.5) / ratio + 0.5;
        if (new_uv.y < 0.0 || new_uv.y > 1.0) return float2(-1.0, -1.0);
    } else { // Pillarbox
        float ratio = texAspect / aspectOut;
        new_uv.x = (uv.x - 0.5) / ratio + 0.5;
        if (new_uv.x < 0.0 || new_uv.x > 1.0) return float2(-1.0, -1.0);
    }
    return new_uv;
}

float4 PS_Main(VS_OUT input) : SV_TARGET {
    float2 uvA = adjust_uv(input.uv, aspectA);
    float2 uvB = adjust_uv(input.uv, aspectB);
    
    float4 colorA = (uvA.x < 0.0) ? float4(0,0,0,1) : texA.Sample(smp, uvA);
    float4 colorB = (uvB.x < 0.0) ? float4(0,0,0,1) : texB.Sample(smp, uvB);
    
    return lerp(colorA, colorB, blendFactor);
}
\0";

#[repr(C, align(16))]
pub struct BlendData {
    pub blend_factor: f32,
    pub aspect_a: f32,
    pub aspect_b: f32,
    pub aspect_out: f32,
}

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
    pub staging_a: Mutex<Option<ID3D11Texture2D>>,
    pub staging_b: Mutex<Option<ID3D11Texture2D>>,
    pub readback_textures: Mutex<[ID3D11Texture2D; 2]>,
    pub frame_counter: AtomicU64,
    pub ndi_tx: Mutex<Option<SyncSender<NdiFrame>>>,
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
                Width: width, 
                Height: height,
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
                Width: width,
                Height: height,
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
            let readback_textures = Mutex::new([tex1_opt.unwrap(), tex2_opt.unwrap()]);

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
                readback_textures,
                frame_counter: AtomicU64::new(0),
                ndi_tx: Mutex::new(None),
            })
        }
    }

    pub fn update_deck_texture(&self, deck_id: u8, src_texture: &ID3D11Texture2D) -> Result<()> {
        unsafe {
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            src_texture.GetDesc(&mut desc);

            let staging_mutex = if deck_id == 0 { &self.staging_a } else { &self.staging_b };
            let mut staging_lock = staging_mutex.lock().unwrap();

            let needs_new_texture = match staging_lock.as_ref() {
                Some(t) => {
                    let mut curr_desc = D3D11_TEXTURE2D_DESC::default();
                    t.GetDesc(&mut curr_desc);
                    curr_desc.Width != desc.Width || curr_desc.Height != desc.Height
                }
                None => true,
            };

            if needs_new_texture {
                let new_desc = D3D11_TEXTURE2D_DESC {
                    Width: desc.Width,
                    Height: desc.Height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: desc.Format,
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32 | D3D11_BIND_RENDER_TARGET.0 as u32,
                    ..Default::default()
                };
                let mut new_tex_opt: Option<ID3D11Texture2D> = None;
                self.device.CreateTexture2D(&new_desc, None, Some(&mut new_tex_opt))?;
                *staging_lock = Some(new_tex_opt.unwrap());
            }

            let staging_tex = staging_lock.as_ref().unwrap();
            let dest_res: ID3D11Resource = staging_tex.cast()?;
            let src_res: ID3D11Resource = src_texture.cast()?;
            self.context.CopyResource(&dest_res, &src_res);
        }
        Ok(())
    }

    pub fn clear_deck(&self, deck_id: u8) {
        let staging_mutex = if deck_id == 0 { &self.staging_a } else { &self.staging_b };
        if let Ok(mut staging_lock) = staging_mutex.lock() {
            *staging_lock = None;
        }
    }

    pub fn render_composited(&self, blend_factor: f32) -> Result<()> {
        unsafe {
            let backbuffer: ID3D11Texture2D = self.swapchain.GetBuffer(0)?;
            let mut rtv_opt: Option<windows::Win32::Graphics::Direct3D11::ID3D11RenderTargetView> = None;
            let dest_res: ID3D11Resource = backbuffer.cast()?;
            self.device.CreateRenderTargetView(&dest_res, None, Some(&mut rtv_opt))?;
            let rtv = rtv_opt.unwrap();
            
            let current_w = self.width.load(Ordering::Acquire);
            let current_h = self.height.load(Ordering::Acquire);

            let viewport = windows::Win32::Graphics::Direct3D11::D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: current_w as f32,
                Height: current_h as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            };
            self.context.RSSetViewports(Some(&[viewport]));
            
            self.context.OMSetRenderTargets(Some(&[Some(rtv)]), None);

            self.context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.context.VSSetShader(&self.vertex_shader, None);
            self.context.PSSetShader(&self.pixel_shader, None);
            self.context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));

            let lock_a = self.staging_a.lock().unwrap();
            let lock_b = self.staging_b.lock().unwrap();

            let get_aspect = |tex_opt: &Option<ID3D11Texture2D>| -> f32 {
                if let Some(tex) = tex_opt {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    tex.GetDesc(&mut desc);
                    if desc.Height > 0 {
                        return desc.Width as f32 / desc.Height as f32;
                    }
                }
                0.0
            };

            let aspect_a = get_aspect(&*lock_a);
            let aspect_b = get_aspect(&*lock_b);
            let aspect_out = if current_h > 0 { current_w as f32 / current_h as f32 } else { 1.0 };

            let blend_data = BlendData { blend_factor, aspect_a, aspect_b, aspect_out };
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(&self.constant_buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
            std::ptr::copy_nonoverlapping(&blend_data as *const _ as *const u8, mapped.pData as *mut u8, std::mem::size_of::<BlendData>());
            self.context.Unmap(&self.constant_buffer, 0);
            
            self.context.PSSetConstantBuffers(0, Some(&[Some(self.constant_buffer.clone())]));

            let create_srv = |tex_opt: &Option<ID3D11Texture2D>| -> Result<Option<ID3D11ShaderResourceView>> {
                if let Some(tex) = tex_opt {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    tex.GetDesc(&mut desc);
                    let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                        Format: desc.Format,
                        ViewDimension: D3D_SRV_DIMENSION_TEXTURE2D,
                        Anonymous: windows::Win32::Graphics::Direct3D11::D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                            Texture2D: windows::Win32::Graphics::Direct3D11::D3D11_TEX2D_SRV {
                                MostDetailedMip: 0,
                                MipLevels: 1,
                            }
                        },
                    };
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

            // Phase 5A: Pipelined Readback - GATED by NDI!
            let tx_opt = self.ndi_tx.lock().unwrap();
            if tx_opt.is_some() {
                let frame = self.frame_counter.load(Ordering::Relaxed);
                
                let readback_lock = self.readback_textures.lock().unwrap();
                let target_readback = &readback_lock[(frame % 2) as usize];
                let dest_readback_res: ID3D11Resource = target_readback.cast()?;
                let src_backbuffer_res: ID3D11Resource = backbuffer.cast()?;
                self.context.CopyResource(&dest_readback_res, &src_backbuffer_res);
                
                if frame > 0 {
                    let mapped_tex = &readback_lock[((frame - 1) % 2) as usize];
                    let mapped_res: ID3D11Resource = mapped_tex.cast()?;
                    let mut mapped_subresource = D3D11_MAPPED_SUBRESOURCE::default();
                    self.context.Map(
                        &mapped_res, 
                        0, 
                        windows::Win32::Graphics::Direct3D11::D3D11_MAP_READ, 
                        0, 
                        Some(&mut mapped_subresource)
                    )?;
                    
                    if let Some(tx) = tx_opt.as_ref() {
                        let total_bytes = (mapped_subresource.RowPitch * current_h) as usize;
                        let slice = std::slice::from_raw_parts(mapped_subresource.pData as *const u8, total_bytes);
                        let ndi_frame = NdiFrame { 
                            data: slice.to_vec(), 
                            width: current_w as i32, 
                            height: current_h as i32, 
                            stride: mapped_subresource.RowPitch as i32 
                        };
                        let _ = tx.try_send(ndi_frame);
                    }
                    
                    self.context.Unmap(&mapped_res, 0);
                }
                
                self.frame_counter.fetch_add(1, Ordering::Relaxed);
            }

            // Unbind to release references safely
            self.context.PSSetShaderResources(0, Some(&[None, None]));
            self.context.OMSetRenderTargets(None, None);

            let _ = self.swapchain.Present(1, 0); 
        }
        Ok(())
    }

    pub fn resize(&self, new_width: u32, new_height: u32) -> Result<()> {
        if new_width == 0 || new_height == 0 { return Ok(()); }
        unsafe {
            let current_w = self.width.load(Ordering::Acquire);
            let current_h = self.height.load(Ordering::Acquire);
            if current_w == new_width && current_h == new_height { return Ok(()); }

            // 1. Release readback textures to free references to DXGI
            let mut readback_lock = self.readback_textures.lock().unwrap();
            
            // 2. Resize swapchain buffers
            self.swapchain.ResizeBuffers(2, new_width, new_height, DXGI_FORMAT_B8G8R8A8_UNORM, 0)?;

            // 3. Update atomics
            self.width.store(new_width, Ordering::Release);
            self.height.store(new_height, Ordering::Release);

            // 4. Recreate readback textures
            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: new_width,
                Height: new_height,
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
            self.device.CreateTexture2D(&staging_desc, None, Some(&mut tex1_opt))?;
            let mut tex2_opt: Option<ID3D11Texture2D> = None;
            self.device.CreateTexture2D(&staging_desc, None, Some(&mut tex2_opt))?;
            *readback_lock = [tex1_opt.unwrap(), tex2_opt.unwrap()];
        }
        Ok(())
    }
}
