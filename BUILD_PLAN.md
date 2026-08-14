# M-Playlist Build Plan (Feature Roadmap)

This living document tracks the architectural implementation of the M-Playlist feature set.

## ✅ Fully Implemented (Core Engine Complete)

- [x] **Video & Audio Files**: Hardware-accelerated Media Foundation pipeline.
- [x] **Hardware Acceleration**: Direct3D 11, Media Foundation, DXGI.
- [x] **In & Out Points**: Hardware-level trimming and scrubbing.
- [x] **Network Video Output**: Asynchronous NDI output, 1080p immutable swapchain.
- [x] **Multi-Channel & Embedded Routing**: 16-channel zero-allocation planar multiplexer routing matrix.
- [x] **OSC & UDP**: Bidirectional feedback over UDP.
- [x] **Seamless Looping**: Dual-Decoder A/B state machine.

## 🚧 Foundation Laid (Engine Ready, Needs Logic/UI)

- [ ] **Subtitles & Closed Captions**: Direct2D/DirectWrite bridge built; needs `.SRT`/`.SCC` parser in C#.
- [ ] **Timecode Overlays & Color Mattes**: Direct2D engine ready; needs "Remaining/Elapsed" string injection.
- [ ] **Stereo Downmixing**: 16-channel matrix ready; needs UI toggle.
- [ ] **10-Bit Rendering Mode**: Needs HDR compute shader (R10G10B10A2 -> 8-bit).
- [ ] **Global Controls (Main Fader)**: `mplaylist_set_volume_db` implemented; needs WPF UI fader.

## 🏗️ TIER 1: CORE LOGISTICS & STABILITY (The Engine Must Drive Itself)
Before we can visualize data or project it to physical monitors, the C# Brain must become a deterministic, autonomous state machine. The UI thread must not govern playback.

### Phase 7: The Conductor (Decoupled Execution Logic)
**The Mission:** Dismantle the UI-bound DispatcherTimer. Architect a dedicated, high-priority background thread in C# that polls the Rust FFI and orchestrates the A/B deck independently of WPF rendering.
- [x] Autonomous B-Deck Pre-load (Lookahead)
- [x] O(1) Cue ID Dictionary routing
- [x] Auto-Advance
- [x] Play Count (Looping)
- [x] Frame-accurate FFI trigger timing

### Phase 8: Telemetry & The Audio Dashboard
**The Mission:** Safely extract thermodynamic data (Peak/RMS) from the Rust lock-free buffers and route it to the WPF UI.
- [x] 60fps Visual Waveforms & VU Meters
- [x] Global Controls (Master Fader)
- [x] Stereo Downmix Toggle

## 🟡 TIER 2: PIXEL MANIPULATION & OVERLAYS (Formatting the Signal)
Before we output the signal to secondary monitors, the internal visual composition pipeline must be feature-complete in VRAM.

### Phase 9: The Typography Bridge & Static Assets
**The Mission:** Overlay dynamic timecode, network logos (Bugs), and lower-thirds natively on the GPU before SDI/NDI transmission.
- [x] Lock-Free C-ABI String Bridge (`RwLock<Vec<u16>>`)
- [x] Elapsed/Remaining Timecode Overlay
- [ ] Network Bug (Logo) Alpha-Channel Rendering
- [ ] Zero-allocation .SRT Subtitle Parser
- [x] Static Asset Rasterization (.PNG/.JPG holding slides bypassing the Media Foundation video decoder)

### Phase 10: Spatial Geometry & GPU Color
**The Mission:** Inject ID3D11Buffer (Constant Buffers) into the HLSL pixel/vertex shaders to manipulate the active swapchain mathematically.
- [ ] Video Cropping
- [ ] Pan/Zoom scaling
- [ ] Live RGB/Brightness/Contrast shaders

## 🟢 TIER 3: TOPOLOGICAL OUTPUT (Projecting the Signal)
The internal engine is now mathematically and visually complete. It is time to externalize it to physical hardware.

### Phase 11: Static Asset Polymorphism & C-ABI
**The Mission:** Routing static textures alongside dynamic video with zero-copy efficiency.
- [x] The Static Asset Pipeline (WIC to DX11 Integration)
- [x] Static Asset Polymorphism & C# Routing
- [x] The Pure State Realignment (C-ABI Modality Enforcement)

### Phase 12: Topological Output & Clean Feed
**The Mission:** Spawning hardware-accelerated Win32 windows and maintaining immutable swapchains for broadcast.
- [x] Immutable 1080p Swapchain & Direct2D Typography
- [x] Topological Output & Clean Feed routing
- [ ] Borderless fullscreen mapping across Windows OS monitor topologies
- [ ] Checkerboard Alpha Previews in WPF
- [x] 10-Bit HDR Compute Shader conversion

## 🔵 TIER 4: THE MITTI-KILLERS (Advanced Sync & Integrations)
The commercial-grade foundation is hermetically sealed. We now ingest external triggers, chaotic I/O, and automate the workflow.

### Phase 13: External Control & Network State
**The Mission:** Slaving the M-Playlist engine to external show-control protocols.
- [x] Phase 13a: OSC State Injection (C# UDP Listener & EngineConductor API)
- [ ] Network Sync (MTC/LTC master/follower)
- [ ] MIDI & DMX (Art-Net/sACN) triggers
- [ ] ATEM Switcher TCP/UDP integration
- [ ] NDI Tally integration
- [ ] HyperDeck emulation

### Phase 14: Specialized Ingestion (Live Inputs & NDI)
**The Mission:** Opening the engine to real-time, non-standard media streams.
- [x] Phase 14a: NDI Ingestion (Structural Primitive & Receiver FFI)
- [x] Phase 14b: NDI Ingestion (VRAM Blasting & Transition Lock)
- [x] Phase 14c: Local Hardware Ingestion (UVC/Capture Cards)
- [ ] Blackmagic DeckLink SDK (SDI In/Out with Key & Fill) <!-- ARCHITECT FLAG: No community wrappers. Use raw C-compatible FFI bindings only. -->
- [ ] Browser Cues (Chromium CEF) <!-- ARCHITECT FLAG: CEF prohibited. Use pure Win32 COM ICoreWebView2 (Edge WebView2) mapped to DX11 shared surface. -->
- [ ] Desktop Capture (DXGI Desktop Duplication)

### Phase 15: Workflow Automations (The Studio Tools)
**The Mission:** High-value quality-of-life features executed entirely outside the real-time rendering loop.
- [ ] Automagic File Tracking (FileSystemWatcher)
- [ ] Offline Audio Normalization (LUFS scanning)
- [ ] "Bundle Playlist" packaging tool
- [ ] Background FFmpeg Transcoding <!-- ARCHITECT FLAG: FFmpeg prohibited. Use native WMF Transcode Topology (IMFSourceReader to IMFSinkWriter). -->
