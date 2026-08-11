# Project TODOs

## Initialization
- [x] Create foundational context document (CONTEXT.md)
- [x] Create TODO.md and CHANGELOG.md
- [x] Create Implementation Plan based on architectural blueprint

## Phase 1: Core OS-Native Infrastructure Setup
- [x] Setup Rust project with C-library output (`.dll` for Windows)
- [x] Setup `cargo vendor` for necessary FFI crates (`windows-rs`)
- [x] Implement Wait-Free Ring Buffer using `std::sync::atomic`
- [x] Implement Master Clock using direct WASAPI bindings

## Phase 2: Secure Media Engine
- [x] Implement Media Foundation bindings for video demuxing/decoding
- [x] Implement Dual-Decoder state machine for seamless looping

## Phase 3: Graphics & Memory Pipeline
- [x] Implement Dx11Compositor with IDXGISwapChain1 bound to physical UI HWND
- [x] Implement Zero-Copy pipeline (DXGI Surface to DirectX Texture)
- [x] Hardware Video Processor scaling and YUV to ARGB32 conversion

## Phase 4: App Logic & Interfaces
- [x] Implement MPSC message-passing architecture
- [x] Implement Secure OSC UDP server

## Phase 5: Advanced Hardware & Time Manipulation
- [x] Phase 2: Dynamic Audio Routing & Output Windowing
- [x] Phase 3: Hardware-Level Trimming & Scrubbing
- [x] Expose C-FFI for UI interactions

## Phase 5: Native GUI
- [x] Create WPF project with `HwndHost` for raw uncomposited drawing surface
- [x] Bridge WPF to Rust Headless Server via C-FFI (`EngineInterop`)

## Master Roadmap: Feature Engine
### Phase 1: The Brain (Data, State & Ingestion)
- [x] The Advanced Cue Model (FfiCue struct and EngineCue)
- [x] Drag-and-Drop Ingestion UI
- [x] JSON Saving (CueModel and ShowFileService)

### Phase 2: The Venue (Hardware & Output)
- [ ] Secondary Output Window
- [ ] Dynamic Audio Routing

### Phase 3: The Time Domain (Physics & Manipulation)
- [x] Clip Trimming (In / Out Points)
- [x] Seeking / Scrubbing

### Phase 4: Broadcast Capabilities
- [x] Crossfades & Shaders (A/B Deck Compositor)
- [x] NDI Broadcast Output
  - [ ] Implement format-conversion compute shader before readback (10-bit R10G10B10A2 -> 8-bit BGRA)
  - [x] Phase 6 Path B: Direct2D Typography & Immutable 1080p Swapchain
