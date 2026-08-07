# Zero-Trust / OS-Native Architectural Blueprint

## Core Philosophy
Eliminate third-party open-source hardware abstractions (e.g., GStreamer, wgpu, cpal) to mitigate security risks, particularly around media parsing. The engine must communicate directly with the OS's proprietary frameworks using Rust, shifting the security burden to the OS.

## Architecture Priorities

### 1. OS-Native Core Physics (The Gotchas)
- **Master Clock**: Direct C-FFI bindings to OS audio (CoreAudio on Mac / WASAPI on Windows). Real-time sync achieved by querying a Rust `AtomicU64` counter updated in the OS audio callback.
- **Real-Time Threading**: A wait-free ring buffer built purely with the Rust standard library (`std::sync::atomic`). No locks, no third-party ring-buffer crates.
- **Memory Bottleneck (Zero-Copy)**: Decode frames directly into OS-specific shared memory (IOSurface on Mac / DXGI Surface on Windows) and map them directly to GPU textures (Metal/DirectX). The CPU must not touch raw video bytes.

### 2. Secure Media Engine
- **Decoding & Demuxing**: Bind directly to AVFoundation/Media Foundation. The OS handles parsing corrupted/malicious `.mp4` files in a secure hardware-accelerated sandbox.
- **Dual-Decoder Architecture**: Run a pre-loaded secondary decoder in the background to achieve seamless looping without 1-frame stutters.

### 3. Graphics Pipeline
- **Direct GPU Compositing**: Direct FFI bindings to Metal (Mac) or DirectX 12 (Windows).
- **10-Bit Color & Shaders**: Custom MSL or HLSL shaders compiled natively by the OS to convert YUV to RGB directly on the GPU, avoiding banding.
- **Broadcast Integrations**: Wrap closed-source SDKs (NDI, Blackmagic SDI) in strict Rust `unsafe` blocks on isolated threads. GPU readbacks provide raw buffers to these SDKs. Error handling must catch any panics from their SDKs to protect the main app.

### 4. App Logic & Control
- **Message-Passing**: Use `std::sync::mpsc` channels to pass Enum commands from the UI thread to the main engine loop, preventing data races.
- **Secure Network Control (OSC)**: Use `std::net::UdpSocket` and a strict custom parser that only accepts specific, known commands to prevent buffer-overflow attacks.

### 5. Native OS UI
- **Headless Rust Server**: The entire engine is compiled as a static C-library (`.dylib` or `.dll`).
- **Native GUI Remote**: The UI is built using OS-native tools (SwiftUI on Mac, WinUI on Windows). Button clicks call C-functions exposed by the Rust engine.

## Supply Chain Security
- **Vendor Dependencies**: Use `cargo vendor` to download source code for necessary FFI crates (e.g., `objc2`, `windows-rs`).
- **Air-Gapping**: Commit vendored code to version control and disable network access for the compiler.
- **Automated CVE Auditing**: Run `cargo audit` locally to track vulnerabilities in vendored crates.
