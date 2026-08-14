use std::sync::atomic::{AtomicU32, AtomicU64, Ordering, AtomicBool};
use std::sync::{Mutex, mpsc::SyncSender, RwLock};
use crate::ffi::DEVICE_LOST_FLAG;

pub static SHOW_OVERLAY: AtomicBool = AtomicBool::new(false);
pub static OVERLAY_TEXT: RwLock<Vec<u16>> = RwLock::new(Vec::new());

use crate::ndi_transmitter::{NdiPayload, NdiVideoFrame};
use windows::core::{ComInterface, Result};

#[derive(Clone, Copy, Debug)]
pub struct SpatialColorState {
    pub crop_left: f32,
    pub crop_top: f32,
    pub crop_right: f32,
    pub crop_bottom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
}

pub static SPATIAL_COLOR_STATE: RwLock<SpatialColorState> = RwLock::new(SpatialColorState {
    crop_left: 0.0,
    crop_top: 0.0,
    crop_right: 0.0,
    crop_bottom: 0.0,
    pan_x: 0.0,
    pan_y: 0.0,
    zoom: 1.0,
    brightness: 0.0,
    contrast: 1.0,
    saturation: 1.0,
});
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_1, ID3DBlob,
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
    ID3D11Texture2D, ID3D11VertexShader, ID3D11ComputeShader, ID3D11UnorderedAccessView
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

const PS_CODE: &str = r#"
struct PS_INPUT { float4 pos : SV_POSITION; float2 uv : TEXCOORD; };
Texture2D<float4> texA : register(t0);
Texture2D<float4> texB : register(t1);
SamplerState smp : register(s0);

cbuffer BlendBuffer : register(b0) { 
    float blendFactor; float aspectA; float aspectB; float aspectOut; 
    float4x4 invHomography;
    float cropLeft; float cropTop; float cropRight; float cropBottom;
    float panX; float panY; float zoom; float _pad0;
    float brightness; float contrast; float saturation; float _pad1;
    float is_overlay_a; float is_overlay_b; float pad1; float pad2;
};

float4 PS_Main(PS_INPUT input) : SV_TARGET {
    float2 uv = input.uv;
    uv -= float2(0.5, 0.5);
    uv /= (zoom == 0.0 ? 1.0 : zoom);
    uv += float2(0.5, 0.5);
    uv -= float2(panX, -panY);

    float3 warped = mul(invHomography, float3(uv, 1.0));
    float2 final_uv = warped.xy / warped.z;

    float2 uvA = final_uv;
    float2 uvB = final_uv;
    if (aspectA > aspectOut) { uvA.y = (uvA.y - 0.5) * (aspectOut / aspectA) + 0.5; } 
    else { uvA.x = (uvA.x - 0.5) * (aspectA / aspectOut) + 0.5; }
    
    if (aspectB > aspectOut) { uvB.y = (uvB.y - 0.5) * (aspectOut / aspectB) + 0.5; } 
    else { uvB.x = (uvB.x - 0.5) * (aspectB / aspectOut) + 0.5; }

    float4 colorA = texA.Sample(smp, uvA);
    float4 colorB = texB.Sample(smp, uvB);

    if (uvA.x < cropLeft || uvA.x > (1.0 - cropRight) || uvA.y < cropTop || uvA.y > (1.0 - cropBottom)) colorA = float4(0,0,0,0);
    if (uvB.x < cropLeft || uvB.x > (1.0 - cropRight) || uvB.y < cropTop || uvB.y > (1.0 - cropBottom)) colorB = float4(0,0,0,0);

    float4 finalColor;

    if (is_overlay_b > 0.5) {
        // OVER Operator: Deck B is Premultiplied HTML (Foreground)
        float4 fg = colorB * blendFactor;
        finalColor.rgb = fg.rgb + (colorA.rgb * (1.0 - fg.a));
        finalColor.a = fg.a + (colorA.a * (1.0 - fg.a));
    } 
    else if (is_overlay_a > 0.5) {
        // OVER Operator: Deck A is Premultiplied HTML (Foreground)
        float alphaA = 1.0 - blendFactor;
        float4 fg = colorA * alphaA;
        finalColor.rgb = fg.rgb + (colorB.rgb * (1.0 - fg.a));
        finalColor.a = fg.a + (colorB.a * (1.0 - fg.a));
    }
    else {
        // Standard Video Temporal Crossfade
        finalColor = lerp(colorA, colorB, blendFactor);
    }

    // Neutral Color Grading (0.0 = no change)
    finalColor.rgb *= (brightness + 1.0);
    finalColor.rgb = (finalColor.rgb - 0.5) * (contrast + 1.0) + 0.5;
    
    float luminance = dot(finalColor.rgb, float3(0.2126, 0.7152, 0.0722));
    finalColor.rgb = lerp(float3(luminance, luminance, luminance), finalColor.rgb, saturation + 1.0);

    // Output HDR buffer clamped to SDR monitor
    return float4(saturate(finalColor.rgb), finalColor.a);
}
"#;

const COMPUTE_SHADER_P010: &str = r#"
Texture2D<float> LumaTex : register(t2);
Texture2D<float2> ChromaTex : register(t3);
RWTexture2D<float4> OutTex : register(u0);

[numthreads(8, 8, 1)]
void CSMain(uint3 tid : SV_DispatchThreadID) {
    uint width, height;
    OutTex.GetDimensions(width, height);
    if (tid.x >= width || tid.y >= height) return;

    float y = LumaTex[tid.xy].r;
    float2 uv = ChromaTex[tid.xy / 2].rg;
    
    // BT.709 Mathematical Reconstruction (Video Range)
    y = 1.164383 * (y - 0.062745);
    float u = uv.r - 0.5;
    float v = uv.g - 0.5;
    
    float r = y + 1.596027 * v;
    float g = y - 0.391762 * u - 0.812968 * v;
    float b = y + 2.017232 * u;
    
    OutTex[tid.xy] = float4(saturate(r), saturate(g), saturate(b), 1.0);
}
"#;

const COMPUTE_SHADER_UYVY: &str = r#"
Texture2D<float4> MacropixelTex : register(t4);
RWTexture2D<float4> OutTex : register(u0);

[numthreads(8, 8, 1)]
void CSMain(uint3 tid : SV_DispatchThreadID) {
    uint width, height; OutTex.GetDimensions(width, height);
    if (tid.x * 2 >= width || tid.y >= height) return;

    float4 macro = MacropixelTex[tid.xy]; // R=U0, G=Y0, B=V0, A=Y1
    float u = macro.r - 0.5; float v = macro.b - 0.5;
    
    // Pixel 0 (Y0)
    float y0 = 1.164383 * (macro.g - 0.062745);
    float r0 = y0 + 1.596027 * v; float g0 = y0 - 0.391762 * u - 0.812968 * v; float b0 = y0 + 2.017232 * u;
    OutTex[uint2(tid.x * 2, tid.y)] = float4(saturate(r0), saturate(g0), saturate(b0), 1.0);

    // Pixel 1 (Y1)
    float y1 = 1.164383 * (macro.a - 0.062745);
    float r1 = y1 + 1.596027 * v; float g1 = y1 - 0.391762 * u - 0.812968 * v; float b1 = y1 + 2.017232 * u;
    OutTex[uint2(tid.x * 2 + 1, tid.y)] = float4(saturate(r1), saturate(g1), saturate(b1), 1.0);
}
"#;

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct BlendData {
    pub blend_factor: f32,
    pub aspect_a: f32,
    pub aspect_b: f32,
    pub aspect_out: f32,
    
    pub inv_homography: [f32; 16],
    
    pub crop_left: f32,
    pub crop_top: f32,
    pub crop_right: f32,
    pub crop_bottom: f32,
    
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
    pub _pad0: f32,
    
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub _pad1: f32,

    pub is_overlay_a: f32,
    pub is_overlay_b: f32,
    pub pad1: f32,
    pub pad2: f32,
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

pub struct ComputeStaging {
    pub planar_tex: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    pub uav_tex: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    pub uav_view: windows::Win32::Graphics::Direct3D11::ID3D11UnorderedAccessView,
    pub srv_view: windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView,
    pub width: u32,
    pub height: u32,
    pub format: u32,
}

pub struct SendableSample(pub windows::Win32::Media::MediaFoundation::IMFSample);
unsafe impl Send for SendableSample {}
unsafe impl Sync for SendableSample {}

pub struct NdiPingPong {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixels: Vec<u8>,
    pub is_dirty: bool,
}

pub struct DxgiSharedFrame {
    pub texture: Option<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D>,
}
impl Default for DxgiSharedFrame {
    fn default() -> Self { Self { texture: None } }
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
    compute_shader: ID3D11ComputeShader,
    compute_shader_uyvy: ID3D11ComputeShader,
    pub compute_staging_a: std::sync::Mutex<Option<ComputeStaging>>,
    pub compute_staging_b: std::sync::Mutex<Option<ComputeStaging>>,
    pub staging_a: Mutex<Option<(ID3D11Texture2D, u32, Option<SendableSample>)>>,
    pub staging_b: Mutex<Option<(ID3D11Texture2D, u32, Option<SendableSample>)>>,
    pub ndi_rx_a: std::sync::Arc<std::sync::Mutex<NdiPingPong>>,
    pub ndi_rx_b: std::sync::Arc<std::sync::Mutex<NdiPingPong>>,
    pub dxgi_rx_a: std::sync::Arc<std::sync::Mutex<DxgiSharedFrame>>,
    pub dxgi_rx_b: std::sync::Arc<std::sync::Mutex<DxgiSharedFrame>>,
    pub dxgi_staging_a: std::sync::Mutex<Option<(windows::Win32::Graphics::Direct3D11::ID3D11Texture2D, windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView)>>,
    pub dxgi_staging_b: std::sync::Mutex<Option<(windows::Win32::Graphics::Direct3D11::ID3D11Texture2D, windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView)>>,
    pub sdi_rx_a: std::sync::Arc<std::sync::Mutex<Option<crate::decklink_capture::DecklinkSharedFrame>>>,
    pub sdi_rx_b: std::sync::Arc<std::sync::Mutex<Option<crate::decklink_capture::DecklinkSharedFrame>>>,
    pub sdi_staging_a: std::sync::Mutex<Option<(windows::Win32::Graphics::Direct3D11::ID3D11Texture2D, windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView)>>,
    pub sdi_staging_b: std::sync::Mutex<Option<(windows::Win32::Graphics::Direct3D11::ID3D11Texture2D, windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView)>>,
    pub webview_srv_a: std::sync::Mutex<Option<windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView>>,
    pub webview_srv_b: std::sync::Mutex<Option<windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView>>,
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
    pub output_swapchain: std::sync::Mutex<Option<windows::Win32::Graphics::Dxgi::IDXGISwapChain1>>,
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

            let mut cs_blob_opt: Option<ID3DBlob> = None;
            let mut cs_error_blob_opt: Option<ID3DBlob> = None;
            let res = D3DCompile(
                COMPUTE_SHADER_P010.as_ptr() as *const _, COMPUTE_SHADER_P010.len() - 1,
                windows::core::s!("CSMain"), None, None,
                windows::core::s!("CSMain"), windows::core::s!("cs_5_0"),
                0, 0, &mut cs_blob_opt, Some(&mut cs_error_blob_opt),
            );
            if res.is_err() {
                if let Some(err_blob) = cs_error_blob_opt {
                    let err_str = unsafe { std::ffi::CStr::from_ptr(err_blob.GetBufferPointer() as *const i8).to_string_lossy() };
                    panic!("CS Compile Error: {}", err_str);
                } else { res?; }
            }
            let cs_blob = cs_blob_opt.unwrap();
            let mut compute_shader_opt: Option<ID3D11ComputeShader> = None;
            unsafe {
                device.CreateComputeShader(
                    std::slice::from_raw_parts(cs_blob.GetBufferPointer() as *const u8, cs_blob.GetBufferSize()),
                    None, Some(&mut compute_shader_opt)
                )?;
            }
            let compute_shader = compute_shader_opt.unwrap();
            
            let mut cs_uyvy_blob_opt: Option<ID3DBlob> = None;
            let mut cs_uyvy_error_blob_opt: Option<ID3DBlob> = None;
            let res_uyvy = D3DCompile(
                COMPUTE_SHADER_UYVY.as_ptr() as *const _, COMPUTE_SHADER_UYVY.len() - 1,
                windows::core::s!("CSMain"), None, None,
                windows::core::s!("CSMain"), windows::core::s!("cs_5_0"),
                0, 0, &mut cs_uyvy_blob_opt, Some(&mut cs_uyvy_error_blob_opt),
            );
            if res_uyvy.is_err() {
                if let Some(err_blob) = cs_uyvy_error_blob_opt {
                    let err_str = unsafe { std::ffi::CStr::from_ptr(err_blob.GetBufferPointer() as *const i8).to_string_lossy() };
                    panic!("CS UYVY Compile Error: {}", err_str);
                } else { res_uyvy?; }
            }
            let mut compute_shader_uyvy_opt: Option<ID3D11ComputeShader> = None;
            let cs_uyvy_blob = cs_uyvy_blob_opt.unwrap();
            unsafe {
                device.CreateComputeShader(
                    std::slice::from_raw_parts(cs_uyvy_blob.GetBufferPointer() as *const u8, cs_uyvy_blob.GetBufferSize()),
                    None, Some(&mut compute_shader_uyvy_opt)
                )?;
            }
            let compute_shader_uyvy = compute_shader_uyvy_opt.unwrap();

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
                compute_shader,
                compute_shader_uyvy,
                compute_staging_a: std::sync::Mutex::new(None),
                compute_staging_b: std::sync::Mutex::new(None),
                staging_a: Mutex::new(None),
                staging_b: Mutex::new(None),
                ndi_rx_a: std::sync::Arc::new(std::sync::Mutex::new(NdiPingPong { width: 0, height: 0, stride: 0, pixels: Vec::new(), is_dirty: false })),
                ndi_rx_b: std::sync::Arc::new(std::sync::Mutex::new(NdiPingPong { width: 0, height: 0, stride: 0, pixels: Vec::new(), is_dirty: false })),
                dxgi_rx_a: std::sync::Arc::new(std::sync::Mutex::new(DxgiSharedFrame::default())),
                dxgi_rx_b: std::sync::Arc::new(std::sync::Mutex::new(DxgiSharedFrame::default())),
                dxgi_staging_a: std::sync::Mutex::new(None),
                dxgi_staging_b: std::sync::Mutex::new(None),
                sdi_rx_a: std::sync::Arc::new(std::sync::Mutex::new(None)),
                sdi_rx_b: std::sync::Arc::new(std::sync::Mutex::new(None)),
                sdi_staging_a: std::sync::Mutex::new(None),
                sdi_staging_b: std::sync::Mutex::new(None),
                webview_srv_a: std::sync::Mutex::new(None),
                webview_srv_b: std::sync::Mutex::new(None),
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
                output_swapchain: std::sync::Mutex::new(None),
            })
        }
    }

    pub fn load_shared_surface(&self, handle_val: usize, is_deck_a: bool) -> Result<()> {
        if handle_val == 0 { return Ok(()); }
        let shared_handle = windows::Win32::Foundation::HANDLE(handle_val as isize);
        let mut resource_ptr: Option<windows::Win32::Graphics::Direct3D11::ID3D11Resource> = None;
        unsafe {
            self.device.OpenSharedResource(
                shared_handle,
                &mut resource_ptr as *mut _
            )?;
        }
        let texture: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D = resource_ptr.unwrap().cast()?;
        let mut srv_opt = None;
        unsafe {
            self.device.CreateShaderResourceView(&texture, None, Some(&mut srv_opt))?;
        }
        if is_deck_a {
            if let Ok(mut lock) = self.webview_srv_a.lock() {
                *lock = srv_opt;
            }
        } else {
            if let Ok(mut lock) = self.webview_srv_b.lock() {
                *lock = srv_opt;
            }
        }
        Ok(())
    }

    pub fn update_deck_texture(&self, deck_id: u8, src_texture: &ID3D11Texture2D, subresource_index: u32, sample: &windows::Win32::Media::MediaFoundation::IMFSample) -> Result<()> {
        let staging_mutex = if deck_id == 0 { &self.staging_a } else { &self.staging_b };
        if let Ok(mut staging_lock) = staging_mutex.lock() {
            // TRUE ZERO-COPY: Hold the COM reference and the specific slice index!
            // We MUST also hold the IMFSample reference so MF doesn't overwrite this slice in its pool!
            *staging_lock = Some((src_texture.clone(), subresource_index, Some(SendableSample(sample.clone()))));
        }
        Ok(())
    }

    pub fn update_deck_static_texture(&self, deck_id: u8, src_texture: &ID3D11Texture2D) -> Result<()> {
        let staging_mutex = if deck_id == 0 { &self.staging_a } else { &self.staging_b };
        if let Ok(mut staging_lock) = staging_mutex.lock() {
            *staging_lock = Some((src_texture.clone(), 0, None));
        }
        Ok(())
    }

    pub fn clear_deck(&self, deck_id: u8) {
        let staging_mutex = if deck_id == 0 { &self.staging_a } else { &self.staging_b };
        if let Ok(mut staging_lock) = staging_mutex.lock() {
            *staging_lock = None;
        }
    }

    pub fn render_composited(&self, blend_factor: f32, geometry: &[[f32; 4]; 4], crop: &[f32; 4], pan_zoom: &[f32; 3], color: &[f32; 3]) -> Result<()> {
        // 1. FRAME ORIGIN: Dynamically lock the active DXGI Flip backbuffer
        let current_rtv = unsafe {
            let backbuffer: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D = self.swapchain.GetBuffer(0)?;
            let backbuffer_res: windows::Win32::Graphics::Direct3D11::ID3D11Resource = backbuffer.cast()?;
            let mut rtv_opt = None;
            self.device.CreateRenderTargetView(&backbuffer_res, None, Some(&mut rtv_opt))?;
            rtv_opt.unwrap()
        };

        unsafe {
            let current_w = 1920;
            let current_h = 1080;

            let viewport = windows::Win32::Graphics::Direct3D11::D3D11_VIEWPORT { 
                TopLeftX: 0.0, TopLeftY: 0.0, Width: 1920.0, Height: 1080.0, MinDepth: 0.0, MaxDepth: 1.0 
            };
            self.context.RSSetViewports(Some(&[viewport]));

            let clear_color: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
            self.context.ClearRenderTargetView(&current_rtv, &clear_color);
            self.context.OMSetRenderTargets(Some(&[Some(current_rtv.clone())]), None);

            self.context.IASetPrimitiveTopology(windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.context.VSSetShader(&self.vertex_shader, None);
            self.context.PSSetShader(&self.pixel_shader, None);
            self.context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));

            let process_ndi_rx = |rx_mutex: &std::sync::Mutex<NdiPingPong>, staging_mutex: &std::sync::Mutex<Option<(windows::Win32::Graphics::Direct3D11::ID3D11Texture2D, u32, Option<crate::graphics::SendableSample>)>>| {
                if let Ok(mut rx) = rx_mutex.lock() {
                    if rx.is_dirty && rx.width > 0 && rx.height > 0 {
                        // 1. Attempt zero-copy Map/Unmap if DYNAMIC texture already matches dimensions
                        let mut can_map = false;
                        if let Ok(staging_lock) = staging_mutex.lock() {
                            if let Some((tex, _, _)) = staging_lock.as_ref() {
                                let mut desc = windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC::default();
                                tex.GetDesc(&mut desc);
                                if desc.Width == rx.width && desc.Height == rx.height && desc.Usage == windows::Win32::Graphics::Direct3D11::D3D11_USAGE_DYNAMIC {
                                    can_map = true;
                                }
                            }
                        }

                        if can_map {
                            if let Ok(staging_lock) = staging_mutex.lock() {
                                let tex = &staging_lock.as_ref().unwrap().0;
                                let mut mapped = windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
                                if self.context.Map(tex, 0, windows::Win32::Graphics::Direct3D11::D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped)).is_ok() {
                                    let src_pitch = (rx.width * 4) as usize;
                                    let src_ptr = rx.pixels.as_ptr();
                                    let dst_ptr = mapped.pData as *mut u8;
                                    
                                    for y in 0..rx.height {
                                        std::ptr::copy_nonoverlapping(
                                            src_ptr.offset((y as usize * src_pitch) as isize),
                                            dst_ptr.offset((y * mapped.RowPitch) as isize),
                                            src_pitch
                                        );
                                    }
                                    self.context.Unmap(tex, 0);
                                    rx.is_dirty = false;
                                    return;
                                }
                            }
                        }

                        // 2. Fallback: Create new texture if mapping failed or dimensions changed
                        let desc = windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC {
                            Width: rx.width,
                            Height: rx.height,
                            MipLevels: 1,
                            ArraySize: 1,
                            Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                            SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                            Usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE_DEFAULT,
                            BindFlags: windows::Win32::Graphics::Direct3D11::D3D11_BIND_SHADER_RESOURCE.0 as u32,
                            ..Default::default()
                        };
                        
                        let init_data = windows::Win32::Graphics::Direct3D11::D3D11_SUBRESOURCE_DATA {
                            pSysMem: rx.pixels.as_ptr() as *const _,
                            SysMemPitch: rx.width * 4,
                            SysMemSlicePitch: 0,
                        };

                        let mut texture: Option<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D> = None;
                        if self.device.CreateTexture2D(&desc, Some(&init_data), Some(&mut texture)).is_ok() {
                            if let Ok(mut staging_lock) = staging_mutex.lock() {
                                *staging_lock = Some((texture.unwrap(), 0, None));
                                rx.is_dirty = false;
                            }
                        }
                    }
                }
            };

            process_ndi_rx(&self.ndi_rx_a, &self.staging_a);
            process_ndi_rx(&self.ndi_rx_b, &self.staging_b);

            let lock_a = self.staging_a.lock().unwrap();
            let lock_b = self.staging_b.lock().unwrap();

            let get_aspect = |tex_opt: &Option<(ID3D11Texture2D, u32, Option<SendableSample>)>| -> f32 {
                if let Some((tex, _, _)) = tex_opt {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    tex.GetDesc(&mut desc);
                    if desc.Height > 0 {
                        return desc.Width as f32 / desc.Height as f32;
                    }
                }
                1.0
            };

            let mut is_overlay_a = 0.0;
            let mut is_overlay_b = 0.0;

            let mut webview_srv_a_opt = None;
            if let Ok(lock) = self.webview_srv_a.lock() {
                if let Some(srv) = lock.as_ref() {
                    webview_srv_a_opt = Some(srv.clone());
                    is_overlay_a = 1.0;
                }
            }

            let mut webview_srv_b_opt = None;
            if let Ok(lock) = self.webview_srv_b.lock() {
                if let Some(srv) = lock.as_ref() {
                    webview_srv_b_opt = Some(srv.clone());
                    is_overlay_b = 1.0;
                }
            }

            let aspect_a = get_aspect(&*lock_a);
            let aspect_b = get_aspect(&*lock_b);
            let aspect_out = if current_h > 0 { current_w as f32 / current_h as f32 } else { 1.0 };

            let blend_data = BlendData {
                blend_factor, aspect_a, aspect_b, aspect_out,
                inv_homography: calculate_inverse_homography(geometry),
                crop_left: crop[0],
                crop_top: crop[1],
                crop_right: crop[2],
                crop_bottom: crop[3],
                pan_x: pan_zoom[0],
                pan_y: pan_zoom[1],
                zoom: pan_zoom[2],
                _pad0: 0.0,
                brightness: color[0],
                contrast: color[1],
                saturation: color[2],
                _pad1: 0.0,
                is_overlay_a,
                is_overlay_b,
                pad1: 0.0,
                pad2: 0.0,
            };
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(&self.constant_buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
            std::ptr::copy_nonoverlapping(&blend_data as *const _ as *const u8, mapped.pData as *mut u8, std::mem::size_of::<BlendData>());
            self.context.Unmap(&self.constant_buffer, 0);
            
            self.context.VSSetConstantBuffers(0, Some(&[Some(self.constant_buffer.clone())]));
            self.context.PSSetConstantBuffers(0, Some(&[Some(self.constant_buffer.clone())]));

            let create_srv = |tex_opt: &Option<(ID3D11Texture2D, u32, Option<SendableSample>)>, deck_index: u8| -> Result<Option<ID3D11ShaderResourceView>> {
                if let Some((tex, subresource, _)) = tex_opt {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    unsafe { tex.GetDesc(&mut desc) };

                    let is_p010 = desc.Format == windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_P010;
                    let is_nv12 = desc.Format == windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_NV12;

                    if is_p010 || is_nv12 {
                        let mut staging_lock = if deck_index == 0 { self.compute_staging_a.lock().unwrap() } else { self.compute_staging_b.lock().unwrap() };
                        
                        let need_realloc = match &*staging_lock {
                            Some(stage) => stage.width != desc.Width || stage.height != desc.Height || stage.format != desc.Format.0 as u32,
                            None => true,
                        };

                        if need_realloc {
                            let planar_desc = D3D11_TEXTURE2D_DESC {
                                Width: desc.Width, Height: desc.Height, MipLevels: 1, ArraySize: 1, Format: desc.Format,
                                SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                                Usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE_DEFAULT,
                                BindFlags: windows::Win32::Graphics::Direct3D11::D3D11_BIND_SHADER_RESOURCE.0 as u32,
                                ..Default::default()
                            };
                            let mut planar_tex_opt: Option<ID3D11Texture2D> = None;
                            unsafe { self.device.CreateTexture2D(&planar_desc, None, Some(&mut planar_tex_opt))? };
                            let planar_tex = planar_tex_opt.unwrap();

                            let uav_desc = D3D11_TEXTURE2D_DESC {
                                Width: desc.Width, Height: desc.Height, MipLevels: 1, ArraySize: 1,
                                Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16G16B16A16_FLOAT, // 16-Bit Guaranteed Hardware UAV Support
                                SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                                Usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE_DEFAULT,
                                BindFlags: windows::Win32::Graphics::Direct3D11::D3D11_BIND_UNORDERED_ACCESS.0 as u32 | windows::Win32::Graphics::Direct3D11::D3D11_BIND_SHADER_RESOURCE.0 as u32,
                                ..Default::default()
                            };
                            let mut uav_tex_opt: Option<ID3D11Texture2D> = None;
                            unsafe { self.device.CreateTexture2D(&uav_desc, None, Some(&mut uav_tex_opt))? };
                            let uav_tex = uav_tex_opt.unwrap();
                            let res_uav: ID3D11Resource = uav_tex.cast()?;
                            let mut uav_view_opt: Option<ID3D11UnorderedAccessView> = None;
                            unsafe { self.device.CreateUnorderedAccessView(&res_uav, None, Some(&mut uav_view_opt))? };
                            let mut srv_view_opt: Option<ID3D11ShaderResourceView> = None;
                            unsafe { self.device.CreateShaderResourceView(&res_uav, None, Some(&mut srv_view_opt))? };

                            *staging_lock = Some(ComputeStaging {
                                planar_tex, uav_tex, uav_view: uav_view_opt.unwrap(), srv_view: srv_view_opt.unwrap(),
                                width: desc.Width, height: desc.Height, format: desc.Format.0 as u32,
                            });
                        }

                        let stage = staging_lock.as_ref().unwrap();

                        // 2. DUAL ZERO-COPY DMA TRANSFER
                        let src_res: ID3D11Resource = tex.cast()?;
                        let dst_res: ID3D11Resource = stage.planar_tex.cast()?;
                        let mut src_desc = D3D11_TEXTURE2D_DESC::default();
                        unsafe { tex.GetDesc(&mut src_desc) };

                        unsafe { 
                            // Luma
                            self.context.CopySubresourceRegion(&dst_res, 0, 0, 0, 0, &src_res, *subresource, None);
                            // Chroma (Mathematically requires + ArraySize for DXGI Planar formats)
                            self.context.CopySubresourceRegion(&dst_res, 1, 0, 0, 0, &src_res, *subresource + src_desc.ArraySize, None);
                        }

                        let (luma_fmt, chroma_fmt) = if is_p010 { 
                            (windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16_UNORM, windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16G16_UNORM)
                        } else { 
                            (windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8_UNORM, windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8_UNORM)
                        };

                        let mut luma_desc = D3D11_SHADER_RESOURCE_VIEW_DESC { Format: luma_fmt, ViewDimension: D3D_SRV_DIMENSION_TEXTURE2D, ..Default::default() };
                        luma_desc.Anonymous.Texture2D = windows::Win32::Graphics::Direct3D11::D3D11_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1 };
                        
                        let mut chroma_desc = D3D11_SHADER_RESOURCE_VIEW_DESC { Format: chroma_fmt, ViewDimension: D3D_SRV_DIMENSION_TEXTURE2D, ..Default::default() };
                        chroma_desc.Anonymous.Texture2D = windows::Win32::Graphics::Direct3D11::D3D11_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1 };

                        let mut luma_srv_opt: Option<ID3D11ShaderResourceView> = None;
                        let mut chroma_srv_opt: Option<ID3D11ShaderResourceView> = None;
                        
                        unsafe {
                            self.device.CreateShaderResourceView(&dst_res, Some(&luma_desc), Some(&mut luma_srv_opt))?;
                            self.device.CreateShaderResourceView(&dst_res, Some(&chroma_desc), Some(&mut chroma_srv_opt))?;
                            
                            self.context.CSSetShader(&self.compute_shader, None);
                            let srvs = [luma_srv_opt.clone(), chroma_srv_opt.clone()];
                            self.context.CSSetShaderResources(2, Some(&srvs));
                            
                            let uavs = [Some(stage.uav_view.clone())];
                            self.context.CSSetUnorderedAccessViews(0, 1, Some(uavs.as_ptr() as *const _), None);
                            
                            self.context.Dispatch((desc.Width + 7) / 8, (desc.Height + 7) / 8, 1); 
                            
                            // CRITICAL FLUSH: Unbind UAV and CS SRVs before returning
                            let null_uavs: [Option<ID3D11UnorderedAccessView>; 1] = [None];
                            self.context.CSSetUnorderedAccessViews(0, 1, Some(null_uavs.as_ptr() as *const _), None);
                            let null_cs_srvs: [Option<ID3D11ShaderResourceView>; 2] = [None, None];
                            self.context.CSSetShaderResources(2, Some(&null_cs_srvs));
                        }
                        
                        return Ok(Some(stage.srv_view.clone()));
                    }

                    // STANDARD 8-BIT BYPASS (NDI / UI)
                    let mut srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC { Format: desc.Format, ..Default::default() };
                    if desc.ArraySize > 1 {
                        srv_desc.ViewDimension = D3D_SRV_DIMENSION_TEXTURE2DARRAY;
                        srv_desc.Anonymous.Texture2DArray = windows::Win32::Graphics::Direct3D11::D3D11_TEX2D_ARRAY_SRV { MostDetailedMip: 0, MipLevels: 1, FirstArraySlice: *subresource, ArraySize: 1 };
                    } else {
                        srv_desc.ViewDimension = D3D_SRV_DIMENSION_TEXTURE2D;
                        srv_desc.Anonymous.Texture2D = windows::Win32::Graphics::Direct3D11::D3D11_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1 };
                    }
                    let mut srv_opt: Option<ID3D11ShaderResourceView> = None;
                    let res: ID3D11Resource = tex.cast()?;
                    unsafe { self.device.CreateShaderResourceView(&res, Some(&srv_desc), Some(&mut srv_opt))? };
                    Ok(srv_opt)
                } else {
                    Ok(None)
                }
            };

            let process_dxgi_rx = |rx_mutex: &std::sync::Mutex<DxgiSharedFrame>, staging_mutex: &std::sync::Mutex<Option<(windows::Win32::Graphics::Direct3D11::ID3D11Texture2D, windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView)>>| -> Result<Option<ID3D11ShaderResourceView>> {
                let mut extracted_tex_opt = None;
                if let Ok(mut lock) = rx_mutex.lock() {
                    if lock.texture.is_some() {
                        extracted_tex_opt = lock.texture.take();
                    }
                }
                
                if let Some(extracted) = extracted_tex_opt {
                    let mut src_desc = D3D11_TEXTURE2D_DESC::default();
                    unsafe { extracted.GetDesc(&mut src_desc) };
                    
                    let mut needs_alloc = true;
                    if let Ok(staging_lock) = staging_mutex.lock() {
                        if let Some((stg_tex, _)) = staging_lock.as_ref() {
                            let mut stg_desc = D3D11_TEXTURE2D_DESC::default();
                            unsafe { stg_tex.GetDesc(&mut stg_desc) };
                            if stg_desc.Width == src_desc.Width && stg_desc.Height == src_desc.Height {
                                needs_alloc = false;
                            }
                        }
                    }
                    
                    if needs_alloc {
                        let desc = D3D11_TEXTURE2D_DESC {
                            Width: src_desc.Width,
                            Height: src_desc.Height,
                            MipLevels: 1,
                            ArraySize: 1,
                            Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                            SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                            Usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE_DEFAULT,
                            BindFlags: windows::Win32::Graphics::Direct3D11::D3D11_BIND_SHADER_RESOURCE.0 as u32,
                            ..Default::default()
                        };
                        let mut texture: Option<ID3D11Texture2D> = None;
                        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut texture))? };
                        let tex = texture.unwrap();
                        let res: ID3D11Resource = tex.cast()?;
                        let mut srv: Option<ID3D11ShaderResourceView> = None;
                        unsafe { self.device.CreateShaderResourceView(&res, None, Some(&mut srv))? };
                        
                        if let Ok(mut staging_lock) = staging_mutex.lock() {
                            *staging_lock = Some((tex, srv.unwrap()));
                        }
                    }
                    
                    if let Ok(staging_lock) = staging_mutex.lock() {
                        if let Some((stg_tex, _)) = staging_lock.as_ref() {
                            let dst_res: ID3D11Resource = stg_tex.cast()?;
                            let src_res: ID3D11Resource = extracted.cast()?;
                            unsafe { self.context.CopyResource(&dst_res, &src_res) };
                        }
                    }
                }
                
                if let Ok(staging_lock) = staging_mutex.lock() {
                    if let Some((_, srv)) = staging_lock.as_ref() {
                        return Ok(Some(srv.clone()));
                    }
                }
                Ok(None)
            };

            let process_sdi_rx = |rx_mutex: &std::sync::Arc<std::sync::Mutex<Option<crate::decklink_capture::DecklinkSharedFrame>>>, 
                                  staging_mutex: &std::sync::Mutex<Option<(windows::Win32::Graphics::Direct3D11::ID3D11Texture2D, windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView)>>, 
                                  compute_staging: &std::sync::Mutex<Option<ComputeStaging>>| -> Result<Option<ID3D11ShaderResourceView>> {
                let mut extracted_opt = None;
                if let Ok(mut lock) = rx_mutex.lock() {
                    extracted_opt = lock.take();
                }

                if let Some(extracted) = extracted_opt {
                    let target_width = extracted.width / 2;
                    let target_height = extracted.height;
                    let mut needs_alloc = true;

                    if let Ok(staging_lock) = staging_mutex.lock() {
                        if let Some((stg_tex, _)) = staging_lock.as_ref() {
                            let mut stg_desc = D3D11_TEXTURE2D_DESC::default();
                            unsafe { stg_tex.GetDesc(&mut stg_desc) };
                            if stg_desc.Width == target_width && stg_desc.Height == target_height {
                                needs_alloc = false;
                            }
                        }
                    }

                    if needs_alloc {
                        let desc = D3D11_TEXTURE2D_DESC {
                            Width: target_width,
                            Height: target_height,
                            MipLevels: 1,
                            ArraySize: 1,
                            Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM,
                            SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                            Usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE_DYNAMIC,
                            BindFlags: windows::Win32::Graphics::Direct3D11::D3D11_BIND_SHADER_RESOURCE.0 as u32,
                            CPUAccessFlags: windows::Win32::Graphics::Direct3D11::D3D11_CPU_ACCESS_WRITE.0 as u32,
                            ..Default::default()
                        };
                        let mut texture: Option<ID3D11Texture2D> = None;
                        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut texture))? };
                        let tex = texture.unwrap();
                        let res: ID3D11Resource = tex.cast()?;
                        let mut srv: Option<ID3D11ShaderResourceView> = None;
                        unsafe { self.device.CreateShaderResourceView(&res, None, Some(&mut srv))? };
                        
                        if let Ok(mut staging_lock) = staging_mutex.lock() {
                            *staging_lock = Some((tex, srv.unwrap()));
                        }
                    }

                    if let Ok(staging_lock) = staging_mutex.lock() {
                        if let Some((stg_tex, srv)) = staging_lock.as_ref() {
                            let mut mapped = windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
                            let stg_res: ID3D11Resource = stg_tex.cast()?;
                            unsafe {
                                if self.context.Map(&stg_res, 0, windows::Win32::Graphics::Direct3D11::D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped)).is_ok() {
                                    let src_ptr = extracted.data_ptr;
                                    let dst_ptr = mapped.pData as *mut u8;
                                    for y in 0..target_height {
                                        std::ptr::copy_nonoverlapping(
                                            src_ptr.offset((y * extracted.row_bytes) as isize),
                                            dst_ptr.offset((y * mapped.RowPitch) as isize),
                                            (extracted.width * 2) as usize
                                        );
                                    }
                                    self.context.Unmap(&stg_res, 0);
                                }
                            }
                            
                            let mut comp_lock = compute_staging.lock().unwrap();
                            let need_comp_realloc = match &*comp_lock {
                                Some(stage) => stage.width != extracted.width || stage.height != extracted.height,
                                None => true,
                            };
                            
                            if need_comp_realloc {
                                let uav_desc = D3D11_TEXTURE2D_DESC {
                                    Width: extracted.width, Height: extracted.height, MipLevels: 1, ArraySize: 1,
                                    Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16G16B16A16_FLOAT,
                                    SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                                    Usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE_DEFAULT,
                                    BindFlags: windows::Win32::Graphics::Direct3D11::D3D11_BIND_UNORDERED_ACCESS.0 as u32 | windows::Win32::Graphics::Direct3D11::D3D11_BIND_SHADER_RESOURCE.0 as u32,
                                    ..Default::default()
                                };
                                let mut uav_tex_opt: Option<ID3D11Texture2D> = None;
                                unsafe { self.device.CreateTexture2D(&uav_desc, None, Some(&mut uav_tex_opt))? };
                                let uav_tex = uav_tex_opt.unwrap();
                                let res_uav: ID3D11Resource = uav_tex.cast()?;
                                let mut uav_view_opt: Option<ID3D11UnorderedAccessView> = None;
                                unsafe { self.device.CreateUnorderedAccessView(&res_uav, None, Some(&mut uav_view_opt))? };
                                let mut srv_view_opt: Option<ID3D11ShaderResourceView> = None;
                                unsafe { self.device.CreateShaderResourceView(&res_uav, None, Some(&mut srv_view_opt))? };

                                *comp_lock = Some(ComputeStaging {
                                    planar_tex: stg_tex.clone(), uav_tex, uav_view: uav_view_opt.unwrap(), srv_view: srv_view_opt.unwrap(),
                                    width: extracted.width, height: extracted.height, format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16G16B16A16_FLOAT.0 as u32,
                                });
                            }
                            
                            let comp_stage = comp_lock.as_ref().unwrap();
                            unsafe {
                                self.context.CSSetShader(&self.compute_shader_uyvy, None);
                                let srvs = [Some(srv.clone())];
                                self.context.CSSetShaderResources(4, Some(&srvs));
                                
                                let uavs = [Some(comp_stage.uav_view.clone())];
                                self.context.CSSetUnorderedAccessViews(0, 1, Some(uavs.as_ptr() as *const _), None);
                                
                                self.context.Dispatch((target_width + 7) / 8, (target_height + 7) / 8, 1);
                                
                                let null_uavs: [Option<ID3D11UnorderedAccessView>; 1] = [None];
                                self.context.CSSetUnorderedAccessViews(0, 1, Some(null_uavs.as_ptr() as *const _), None);
                                let null_cs_srvs: [Option<ID3D11ShaderResourceView>; 1] = [None];
                                self.context.CSSetShaderResources(4, Some(&null_cs_srvs));
                            }
                        }
                    }

                    crate::decklink_capture::decklink_release_frame(extracted.frame_ptr);
                }

                if let Ok(comp_lock) = compute_staging.lock() {
                    if let Some(comp_stage) = comp_lock.as_ref() {
                        return Ok(Some(comp_stage.srv_view.clone()));
                    }
                }
                
                Ok(None)
            };

            let mut srv_a_opt = webview_srv_a_opt.clone();
            if srv_a_opt.is_none() {
                srv_a_opt = process_dxgi_rx(&self.dxgi_rx_a, &self.dxgi_staging_a)?;
            }
            if srv_a_opt.is_none() {
                srv_a_opt = process_sdi_rx(&self.sdi_rx_a, &self.sdi_staging_a, &self.compute_staging_a)?;
            }
            if srv_a_opt.is_none() {
                srv_a_opt = create_srv(&*lock_a, 0)?;
            }
            
            let mut srv_b_opt = webview_srv_b_opt.clone();
            if srv_b_opt.is_none() {
                srv_b_opt = process_dxgi_rx(&self.dxgi_rx_b, &self.dxgi_staging_b)?;
            }
            if srv_b_opt.is_none() {
                srv_b_opt = process_sdi_rx(&self.sdi_rx_b, &self.sdi_staging_b, &self.compute_staging_b)?;
            }
            if srv_b_opt.is_none() {
                srv_b_opt = create_srv(&*lock_b, 1)?;
            }

            let srvs = [srv_a_opt.clone(), srv_b_opt.clone()];
            self.context.PSSetShaderResources(0, Some(&srvs));

            self.context.Draw(3, 0);

            let src_tex: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D = self.swapchain.GetBuffer(0)?;
            let src_res: ID3D11Resource = src_tex.cast()?;

            // 2. The Clean Feed Playout Tap
            if let Ok(lock) = self.output_swapchain.lock() {
                if let Some(out_swap) = lock.as_ref() {
                    if let Ok(dest_tex) = out_swap.GetBuffer::<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D>(0) {
                        let dest_res: ID3D11Resource = dest_tex.cast()?;
                        self.context.CopyResource(&dest_res, &src_res);
                        let present_result = unsafe { out_swap.Present(0, 0) }; // VSync off (0,0) to prevent UI stalling
                        if present_result.is_err() {
                            let code = present_result.0 as u32;
                            if code == 0x887A0005 || code == 0x887A0007 {
                                DEVICE_LOST_FLAG.store(true, Ordering::Release);
                            }
                        }
                    }
                }
            }

            // 3. The Clean Feed NDI Tap
            let frame = self.frame_count.load(Ordering::Relaxed);
            let mut ndi_staging_lock = self.ndi_staging_textures.lock().unwrap();
            let mut index_lock = self.ndi_staging_index.lock().unwrap();
            let current_index = *index_lock;
            let tx_opt = self.ndi_tx.lock().unwrap();

            let target_staging_opt = &ndi_staging_lock[current_index];
            if let Some(target_staging) = target_staging_opt {
                let dest_staging_res: ID3D11Resource = target_staging.cast()?;
                self.context.CopyResource(&dest_staging_res, &src_res);
            }

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

                        buffer.set_len(total_bytes);
                        std::ptr::copy_nonoverlapping(mapped_subresource.pData as *const u8, buffer.as_mut_ptr(), total_bytes);
                        
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

            *index_lock = (current_index + 1) % 3;
            self.frame_count.fetch_add(1, Ordering::Relaxed);

            // 4. DIRECT2D TYPOGRAPHY PASS (Control Feed)
            self.d2d_render_target.BeginDraw();
            
            let is_visible = crate::graphics::SHOW_OVERLAY.load(std::sync::atomic::Ordering::Relaxed);
            if is_visible {
                let text_opt: Option<Vec<u16>> = {
                    if let Ok(guard) = crate::graphics::OVERLAY_TEXT.read() {
                        if !guard.is_empty() { Some(guard.clone()) } else { None }
                    } else {
                        None
                    }
                };

                if let Some(text_vec) = text_opt {
                    let text_rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F { 
                        left: 50.0, top: 50.0, right: 1870.0, bottom: 200.0 
                    };
                    
                    self.d2d_render_target.DrawText(
                        &text_vec,
                        &self.text_format,
                        &text_rect,
                        &self.text_brush,
                        windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                        windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
            }
            
            self.d2d_render_target.EndDraw(None, None).unwrap();

            // CRITICAL HAZARD FLUSH: Unbind all SRVs from the Pixel Shader so the next frame's Compute Shader can bind them as UAVs.
            let null_ps_srvs: [Option<ID3D11ShaderResourceView>; 2] = [None, None];
            unsafe { self.context.PSSetShaderResources(0, Some(&null_ps_srvs)) };
            self.context.OMSetRenderTargets(None, None);

            let present_result = self.swapchain.Present(1, 0);
            if present_result.is_err() {
                let code = present_result.0 as u32;
                println!("M-Playlist [CRITICAL]: DXGI Present Failed! HRESULT: 0x{:X}", code);
                if code == 0x887A0005 || code == 0x887A0007 {
                    crate::ffi::DEVICE_LOST_FLAG.store(true, std::sync::atomic::Ordering::Release);
                }
            }
        }
        Ok(())
    }

    pub fn resize(&self, _new_width: u32, _new_height: u32) -> Result<()> {
        // No-op: Lock the swapchain to 1080p
        Ok(())
    }
}
