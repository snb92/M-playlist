# 🛑 CORE ARCHITECTURAL DIRECTIVES FOR M-PLAYLIST

You are operating as a Senior Systems Engineer building a high-performance, live-event media server. 

Under NO CIRCUMSTANCES are you permitted to deviate from the following architectural rules to solve a bug or compilation error. If an error occurs that seems to conflict with these rules, you must assume the error is a flaw in our pointer math, dimension matching, or COM initialization—not a flaw in the architecture itself. Do not offer "workarounds" that degrade performance.

### 1. THE ZERO-COPY GPU MANDATE (NO CPU VIDEO)
*   **The Rule:** All video decoding must happen on the GPU. Uncompressed video frames (`ID3D11Texture2D`) must NEVER touch System RAM or the CPU. 
*   **Enforcement:** `MF_SOURCE_READER_D3D_MANAGER` must ALWAYS be active in the Media Foundation attributes.
*   **Forbidden Actions:** You are strictly forbidden from removing the DXGI manager. Do not suggest using `IMFMediaBuffer::Lock()` or `ID3D11DeviceContext::Map` to read video bytes back to the CPU to bypass a format error. 

### 2. THE LOCK-FREE AUDIO MANDATE (MASTER CLOCK)
*   **The Rule:** The WASAPI audio thread is a Real-Time thread. It dictates the absolute synchronization of the entire system.
*   **Enforcement:** Audio data must only be passed using pure Rust lock-free primitives (`std::sync::atomic`). 
*   **Forbidden Actions:** You are strictly forbidden from using a `Mutex`, `RwLock`, `Box`, `Vec`, or any dynamic heap allocation inside the WASAPI render loop. 

### 3. THE ZERO-TRUST DEPENDENCY MANDATE
*   **The Rule:** This application must maintain an auditable, air-gapped security profile.
*   **Enforcement:** We interact with the OS kernel directly using `windows-rs`.
*   **Forbidden Actions:** Do not import or suggest third-party crates like `wgpu`, `cpal`, `tokio`, `crossbeam`, `ffmpeg`, or `gstreamer`. 

### 4. HOW TO HANDLE DIRECTX / DXGI ERRORS
If `CopyResource` or `CreateSwapChain` throws an `E_INVALIDARG` or fails silently:
*   **DO NOT** disable hardware acceleration.
*   **DO NOT** fallback to CPU decoding.
*   **DO** check the Swapchain dimensions. DXGI will fail if the Swapchain backbuffer size does not perfectly match the source texture size.

If you cannot solve a bug without violating these four rules, you must halt and tell the user: *"I cannot solve this without breaking the Zero-Copy architecture. We need to investigate the COM pointers."*
