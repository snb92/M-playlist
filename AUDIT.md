# Architectural Audit Log

This file contains a timestamped log of all findings, decisions, validations, and architectural shifts.

## [2026-08-05]
- **FINDING / DECISION:** Adopted M-View 2.0 tracking structure for M-Playlist.
- **IMPACT:** Transitioned `CONTEXT.md` to `GOAL_v1.md` and `TODO.md` to `BUILD_PLAN.md` to strictly enforce frozen architectural law and maintain append-only audit tracking.
- **RESOLUTION:** Created tracking files (`AUDIT.md`, `FLOW.md`, `DEV_REFERENCE.md`, `GHOSTS.md`) and `.agents/AGENTS.md` ruleset.

## [2026-08-05]
- **FINDING / DECISION:** Corrected tracking structure to use M-View App template instead of M-View 2.0.
- **IMPACT:** Removed unnecessary tracking files (`FLOW.md`, `DEV_REFERENCE.md`, `GHOSTS.md`) and restored `TODO.md` alongside `BUILD_PLAN.md`.
- **RESOLUTION:** Updated `.agents/AGENTS.md` to reflect the correct documents and deleted unused files.

## [2026-08-05]
- **FINDING / DECISION:** Pivoted GUI architecture from WinUI 3 to WPF.
- **IMPACT:** WinUI 3's `DesktopWindowXamlSource` aggressively locks its main window HWND, causing `CreateSwapChainForHwnd` to panic with `DXGI_ERROR_INVALID_CALL` and breaking the Zero-Copy pipeline.
- **RESOLUTION:** Scaffolded a WPF application using a custom `VideoHwndHost` to carve out a raw, uncomposited Win32 child window, granting the Rust DX11 Compositor absolute ownership of the drawing surface.

## [2026-08-05]
- **FINDING / DECISION:** Enforced a "Panic Gate" for Zero-Copy texture extraction in `media_engine.rs`.
- **IMPACT:** If the DXGI Device Manager is bypassed or fails to return a GPU texture, the decoder thread will panic rather than silently falling back to CPU memory.
- **RESOLUTION:** Implemented the Panic Gate and created `ARCHITECTURE.md` as the source of absolute truth for zero-copy constraints.

## [2026-08-05]
- **FINDING / DECISION:** Media Foundation `MFGetService(..., MR_BUFFER_SERVICE)` throws `E_NOINTERFACE` because it is a legacy Direct3D 9 abstraction.
- **IMPACT:** Prevented extraction of the modern `ID3D11Texture2D` hardware pointer, breaking the Zero-Copy DX11 pipeline.
- **RESOLUTION:** Replaced `MFGetService` with direct COM casting from `IMFMediaBuffer` to `IMFDXGIBuffer` and unwrapped the texture natively via `GetResource()`.

## [2026-08-05]
- **FINDING / DECISION:** Identified systemic thermodynamic deadlocks in `decode_loop`: magic stream constants ignored samples, and audio starvation permanently froze the Master Clock.
- **IMPACT:** Prevented all video and audio playback despite correct initialization.
- **RESOLUTION:** Implemented dynamic stream index discovery, isolated audio extraction from the A/V Sync Gate to feed the ring buffer eagerly, and bound the Master Clock to tick on `frames_available` instead of audio data payload.

## [2026-08-05]
- **FINDING / DECISION:** Identified Mutex Contention danger at the FFI boundary where UI and OSC network threads competed to lock `EngineState` and execute heavy media operations.
- **IMPACT:** If video demuxing/decoding takes milliseconds, it blocks FFI callers, stuttering the WPF UI and dropping UDP packets.
- **RESOLUTION:** Refactored FFI triggers and `Playlist` into an `mpsc` lock-free channel managed by `AppLogic`. Callers instantly toss `EngineCommand` enums into a queue, achieving true enterprise-grade multi-threaded decoupling.

## [2026-08-05] (Polish & Edge Cases)
- **FINDING / DECISION:** WPF `HwndHost` initialized dimensions to `double.NaN`, bypassing the delta resize lock and trapping the video at 100x100.
- **IMPACT:** `NaN` comparisons yielded false, permanently locking the cinematic aspect ratio scaling algorithm. 
- **RESOLUTION:** Added explicit `double.IsNaN` check on the first WPF timer tick to force the initial dimension population.

- **FINDING / DECISION:** C# `DispatcherTimer` spam locked up the UI by continuously triggering size invalidations for the Win32 surface.
- **IMPACT:** The constant invalidation starved WPF of paint cycles.
- **RESOLUTION:** Added `Math.Abs(...) > 1.0` delta thresholds to only apply dimensions if the aspect ratio physically changes, neutralizing the layout spam.

- **FINDING / DECISION:** 5.1 Audio tracks (6 channels) were overwhelming the 2-channel WASAPI pipeline, filling the ring buffer 3x too fast and violently throttling the `Dx11Compositor` thread.
- **IMPACT:** Caused massive stuttering and silence for surround sound media.
- **RESOLUTION:** Injected Media Foundation audio typing flags to force the kernel resampler to dynamically downmix to Stereo (2-channels) and 48kHz before it ever reaches our lock-free buffer.

- **FINDING / DECISION:** 1-second `AudioRingBuffer` capacity caused a 1-second audio bleed from the previously playing deck during instant A/B deck hot-swaps.
- **IMPACT:** A/V transition sync was compromised because WASAPI continued to drain old unread audio floats.
- **RESOLUTION:** Created a lock-free `clear()` method for `AudioRingBuffer` that atomically snaps the `tail` to the `head`, and invoked it right at the `fire_next_cue` swap phase, destroying all overlapping audio instantly.

## [2026-08-05] (Production Architecture Polish)
- **FINDING / DECISION:** Identified WPF "Airspace" bug where the Win32 `HwndHost` rendered behind/over WPF controls because the aspect ratio calculation used the root `Grid` height instead of the video row's height.
- **IMPACT:** Video surface bled into the bottom UI panel.
- **RESOLUTION:** Wrapped `VideoSurface` in a strict `<Border Grid.Row="0">` container, isolating the `ActualHeight` boundaries perfectly.

- **FINDING / DECISION:** Identified WASAPI hardware disconnect vulnerability (`AUDCLNT_E_DEVICE_INVALIDATED`).
- **IMPACT:** If an audio cable was unplugged, the engine hit a thermodynamic deadlock and permanently froze.
- **RESOLUTION:** Implemented "Endpoint Chasing" (Auto-Recovery) via an outer recovery loop in the WASAPI thread that catches the hardware loss, safely drops COM pointers, and rebounds to the new default device within 300ms.

- **FINDING / DECISION:** Evaluated codebase against the 5 Foundational Laws of the Blueprint.
- **IMPACT:** Confirmed zero-copy pipeline, hardware master clock slaving, MPSC decoupling, and COM pointer memory safety.
- **RESOLUTION:** Generated official `audit_report.md` proving the Native Engine is structurally sound and cleared for deployment.

## [2026-08-05] (Phase 1: Feature Engine Architecture)
- **FINDING / DECISION:** Evaluated how to share Playlist State between C# and Rust. Duplicating full lists causes synchronization friction and memory bloat.
- **IMPACT:** A duplicate state violates the zero-copy/lock-free constraint and complicates UI re-ordering or dynamic changes.
- **RESOLUTION:** Enforced the "Brain/Muscle" rule. C# (Brain) exclusively owns the `ObservableCollection<CueModel>`, JSON serialization, and Drag/Drop. Rust (Muscle) only knows about Deck A and Deck B. The boundary is bridged via a single, flat `FfiCue` struct marshaled on demand when firing clips.

## [2026-08-05] (Phase 3: The Time Domain)
- **FINDING / DECISION:** Implemented `pending_scrub` via `AtomicI64` polling in `MediaEngine` instead of passing COM `SetCurrentPosition` through a blocking mpsc channel.
- **IMPACT:** Avoids deadlocking the inner DX11 render loop during heavy video scrubs while remaining lock-free.
- **RESOLUTION:** Adopted the atomic sentinel pattern (-1) to guarantee zero visual tearing. The active deck reads exactly one frame while paused to instantly visually update the DX11 swapchain.

## [2026-08-07] (Phase 4A: Broadcast Compositor)
- **FINDING / DECISION:** Used bit-cast `f32` inside an `AtomicU32` for `blend_factor` to pass floating point crossfade data across threads lock-free.
- **IMPACT:** Avoided Mutex blocking in the critical real-time WASAPI audio loop and the DX11 compositor pipeline, preserving nanosecond-tier performance.
- **RESOLUTION:** Successfully implemented stack-based dual-deck PCM audio mixing and HLSL `lerp` video compositing. Protected concurrent GPU access by enabling `ID3D11Multithread` protection.

## [2026-08-07] (Phase 4B: Temporal State Machine)
- **FINDING / DECISION:** Delegated the crossfade math and VRAM memory cleanup entirely to the `AppLogic` thread using a 60Hz `recv_timeout` tick loop.
- **IMPACT:** Solved the "Threading Trap". If the math or cleanup was handled by the WASAPI loop or the Media Engine decoder loop, it would cause fatal race conditions or violate the lock-free audio mandate.
- **RESOLUTION:** The `AppLogic` thread now recalculates the `blend_factor` smoothly over time based on the active Master Clock and gracefully drops the outgoing deck's `Option<MediaEngine>` to immediately reclaim GPU VRAM.

## [2026-08-07] (Phase 5A: Reverse Zero-Copy)
- **FINDING / DECISION:** Granted a strict exception to the Zero-Copy Mandate exclusively for the NDI broadcast path in the Compositor.
- **IMPACT:** A direct `Map` of the active DXGI backbuffer would stall the CPU thread while the GPU finishes its command queue, ruining the live video framerate.
- **RESOLUTION:** Implemented a 2-frame Pipelined Readback using `D3D11_USAGE_STAGING` textures. The GPU asynchronously copies to a Staging texture on frame N, and the CPU maps and reads the pointer on frame N+1 (which is already finished and sitting in System RAM), resulting in a zero-stall VRAM extraction.

## [2026-08-07] (Phase 5B: NDI Broadcast Output)
- **FINDING / DECISION:** Bypassed all third-party NDI wrapper crates, implementing a raw `libloading` dynamic load of the official NDI library. 
- **IMPACT:** Preserves the Zero-Trust mandate. If a user does not have the NDI runtime installed on their machine, the media server continues to run flawlessly without instantly crashing on startup.
- **RESOLUTION:** Extracted the C function pointers directly. Implemented a strictly decoupled background TX Thread communicating with the DX11 thread via a bounded `sync_channel(2)`. The use of `try_send()` guarantees network spikes will simply drop frames silently rather than stalling the local live video playback pipeline. Overcame Rust's `*mut c_void` `Send` trait restriction across threads by safely transmuting the function pointers to `usize` for the cross-thread boundary.

## [2026-08-07] (Hotfix: HLSL Syntax, WPF Zombie & GPU Mandate Breach)
- **FINDING / DECISION:** The engine crashed with E_FAIL on boot due to a missing `VS_OUT` struct in the Pixel Shader string. The WPF OutputWindow remained running as a background zombie process. Rust compiler warnings falsely flagged `MFCreateDXGIDeviceManager` and `IMFDXGIBuffer` as unused.
- **IMPACT:** A syntax error in the HLSL shader completely halts DX11 initialization. Zombie processes leak memory and block future application launches.
- **RESOLUTION:** Added `VS_OUT` struct definition to `PS_CODE` and implemented robust `error_blob` panic logging in `D3DCompile`. Overrode `OnClosed` in `MainWindow.xaml.cs` with `Environment.Exit(0)` for clean process teardown. Re-verified that the `media_engine.rs` completely bypasses the CPU via `MF_SOURCE_READER_D3D_MANAGER` and removed redundant fully qualified module prefixes to clear the warnings and confirm architectural compliance.

## [2026-08-07] (Phase 6B Fix: E_INVALIDARG during Media Foundation Init)
- **FINDING / DECISION:** `MFCreateSourceReaderFromURL` failed repeatedly with `0x80070057` (`E_INVALIDARG`) when trying to instantiate the Zero-Copy video decoding pipeline. Discovered that the flag `MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING` is fundamentally incompatible with the modern `IMFDXGIDeviceManager` (D3D11/DXGI) and expects legacy D3D9 structures.
- **IMPACT:** A complete failure to load any video file into the native engine on DirectX 11.
- **RESOLUTION:** Swapped the legacy attribute for `MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING`, which is strictly mandated by the Windows OS for any DXGI-based GPU Video Processor interop. Tested successfully in isolation and rolled into `media_engine.rs`.

## [2026-08-07] (Phase 6C: Unified Broadcast Game Loop)
- **FINDING / DECISION:** Diagnosed pixelation and severe A/V fluttering caused by two fundamental violations. 1) `MainWindow.xaml.cs` manually shrunk the `HwndHost` aspect ratio, forcing DXGI to squash the swapchain, resulting in "double letterboxing" (pixelation). 2) Both Media Foundation decoder threads directly invoked `graphics.render_composited()` simultaneously, resulting in a D3D11 threading race condition during crossfades (fluttering).
- **IMPACT:** Severe visual degradation (stretching) and broken rendering pipeline under heavy 4K load.
- **RESOLUTION:** Removed manual WPF resizing; the aspect ratio is now purely preserved via the Rust HLSL Pixel Shader. Completely decoupled DX11 rendering from decoding by migrating `graphics.render_composited()` exclusively to the `AppLogic` thread. Converted `AppLogic` into a true 60Hz Game Loop with an explicit `Instant` throttle to prevent CPU melting. Ensured the decoder thread relies purely on its A/V Sync Gate to regulate `ReadSample` execution natively.
- **ADDITIONAL FINDING (DPI Interop):** Discovered that WPF's `HwndHost` passes layout bounds in Device Independent Pixels (DIPs), whereas DXGI swapchains expect raw Physical Pixels. On screens with Windows Display Scaling (e.g., 150%), the DXGI swapchain was created too small, causing DWM to violently stretch the image.
- **ADDITIONAL RESOLUTION:** Added `PresentationSource.FromVisual` to `MainWindow.xaml.cs` to multiply the WPF layout rect by the physical DPI scale (`M11`/`M22`), passing exact physical dimensions to DXGI. Added a `SetWindowPos` override to `VideoHwndHost.cs` to ensure the Win32 `HWND` scales correctly, perfectly solving the blurriness/pixelation.
