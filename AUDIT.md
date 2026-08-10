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
- **FINDING / DECISION:** Extracted the `SubresourceIndex` from the `IMFDXGIBuffer` and passed it into the DX11 Shader Resource View to ensure we render the exact slice the decoder just wrote.
- **IMPACT:** Even with Zero-Copy active, frames played severely out of order (e.g. Frame 6, then 3, then 7) because we dropped the `IMFSample` too early, causing Media Foundation to overwrite the active slice we were looking at!
- **RESOLUTION:** Forcibly held the `IMFSample` COM object alive inside the graphics `Mutex` until the *next* frame is processed, preventing the hardware decoder from recycling and overwriting the slice.

### A/V Sync Microstutter (Windows Timer Resolution)
- **FINDING / DECISION:** The A/V Sync Gate uses `std::thread::sleep(Duration::from_millis(1))` to precisely hold a frame until its scheduled timecode. However, Windows defaults to a 15.6ms timer resolution, causing the 1ms sleep to overshoot by 14.6ms.
- **IMPACT:** Frames missed their V-Sync deadline by a full 60Hz cycle, resulting in a perceptible "millisecond pause" or micro-stutter every time the sleep overshot.
- **RESOLUTION:** Injected `timeBeginPeriod(1)` at engine startup to force the Windows OS into a high-precision 1ms timer mode, ensuring the Sync Gate wakes up on time. Restored via `timeEndPeriod(1)` on shutdown.

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

### Phase 6D: Texture Cache Poisoning & DXVA Decoding (2026-08-07)
- **FINDING / DECISION (Ghost Frames):** Discovered "Texture Cache Poisoning" where `staging_a`/`staging_b` retained the last frame of a previous cue during hard cuts, causing a ghost frame to flash while the new decoder thread spun up.
- **RESOLUTION:** Implemented `graphics.clear_deck(deck_id)` to explicitly purge the stale `ID3D11Texture2D` from the incoming deck before initializing the new `MediaEngine`.
- **FINDING / DECISION (CPU Bottleneck):** Discovered that 4K 120FPS HEVC files were fluttering because Media Foundation defaulted to software (CPU) decoding. `MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING` only enabled GPU color-conversion, not decompression.
- **IMPACT:** Severe dropped frames ("slow motion" video playback) as the CPU failed to decompress within the 16.6ms A/V Sync Gate window.
- **RESOLUTION:** Injected `MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS = 1` into the Source Reader attributes, successfully mandating DXVA2 / D3D11VA ASIC Hardware Decoding and eliminating the CPU bottleneck.
- **FINDING / DECISION (A/V Sync Scrubbing Hangs):** Discovered that resetting `has_started = false` during a timeline scrub was corrupting the A/V Sync Gate. It caused the `start_time_offset` to snapshot the wall clock time in the future, resulting in a massive artificial offset that tricked the Sync Gate into sleeping for 60+ seconds.
- **RESOLUTION:** Removed the `has_started = false` overwrite. Instead, mathematically re-aligned `start_time_offset` backwards by the exact scrub delta, keeping the monotonic Master Clock pure and preventing the thread from hanging.
- **FINDING / DECISION (Slow Decoder Protection):** Even with hardware acceleration, extremely complex 120FPS 4K codecs could occasionally fall behind real-time. The AppLogic loop would stall trying to render delayed frames.
- **RESOLUTION:** Added a strict Frame Drop failsafe in `media_engine.rs`. If a decoded frame arrives >50ms late relative to the calibrated audio clock, it is instantly discarded before it hits the D3D11 texture, ensuring real-time playback speed is preserved at the cost of dropped visual frames (standard media player behavior).

## [2026-08-07] (Senior Architect Review — Feature Triage)
- **DECISION:** Feature D (Automagic File Tracking) is VETOED for the Rust backend. Rule 3 violation (no third-party crates). Architecture violation: C# is the Brain, Rust is the Muscle. C# will use native `System.IO.FileSystemWatcher` and send `mplaylist_load_cue` commands via FFI.
- **DECISION:** Feature A (10-Bit HDR) is DEFERRED. Changing the SwapChain to `R10G10B10A2_UNORM` breaks the NDI Staging Ring Buffer because `CopyResource` requires identical source/destination formats. Requires a custom down-sampling compute/pixel shader pass first.
- **DECISION:** Feature C (LTC Timecode Chase) is DEFERRED. External clock slaving risks WASAPI starvation/overflow from drift, requiring a custom pitch/time resampler (violates zero-dependency rule).
- **DECISION:** Feature B (Corner Pinning) is AUTHORIZED. Safe extension of the existing HLSL architecture. Standard sliders first; visual canvas is a day-2 frontend task.

## [2026-08-07] (Feature B: Corner Pinning Geometry — Implementation)
- **FINDING / DECISION:** Implemented 4-corner projection mapping via HLSL constant buffer expansion and Triangle Strip topology.
- **IMPACT:** Enables real-time geometric warping of the DX11 output for projection mapping scenarios. Zero regression to existing A/B blending and letterboxing pipelines.
- **RESOLUTION:** Expanded `BlendData` with 4×`[f32;4]` corner fields (16-byte aligned). Replaced oversized triangle VS_CODE with 4-vertex Triangle Strip reading corners from cbuffer. Added `VSSetConstantBuffers` binding. Changed `Draw(3,0)` → `Draw(4,0)`. Routed `mplaylist_set_geometry` FFI through `SetGeometry` mpsc command to `AppLogic` geometry state. Added 8 WPF sliders (range -2.0 to 2.0) with real-time `ValueChanged` wiring.

## [2026-08-07] (Hotfix: Corner Pinning VRAM Smearing)
- **FINDING / DECISION:** Identified a "Hall of Mirrors" smearing effect when the video quad was warped inward via Corner Pinning. The background was retaining ghost pixels from previous frames because `render_composited` was overdrawing without a clear pass.
- **IMPACT:** Severe visual artifacts on exposed background pixels when adjusting projection mapping.
- **RESOLUTION:** Added an explicit `ClearRenderTargetView` call with a pure black clear color (`[0.0, 0.0, 0.0, 1.0]`) directly after RTV creation, ensuring a pristine slate before the Triangle Strip is drawn.

## [2026-08-07] (Architectural Upgrade: True 3D Perspective Projection)
- **FINDING / DECISION:** Identified that a standard affine Triangle Strip causes a "Perspective Divide" fold across the quad when corners are moved independently, ruining projection mapping accuracy.
- **IMPACT:** Transitioned the projection model from affine vertex manipulation to a true 3D planar homography solved on the CPU and executed per-pixel on the GPU.
- **RESOLUTION:** Reverted the Vertex Shader to an oversized Triangle `Draw(3,0)` rendering a full-screen canvas. Implemented an Adjugate Matrix (Inverse Homography) calculation on the Rust `AppLogic` thread to solve the cyclic convex quadrilateral projection. Passed this $4 \times 4$ matrix via the `BlendBuffer` into the Pixel Shader, where `final_uv = uvw.xy / uvw.z` performs a mathematically perfect 3D perspective divide, eliminating the affine fold entirely.

## [2026-08-07] (Feature D: Automagic File Tracking Architecture)
- **FINDING / DECISION:** Implemented Zero-Trust File Tracking exclusively within the C# Macro-State, fully isolating the Rust muscle from file system IO and retaining its strict adherence to Rule 3 (no third-party crates like `notify`).
- **IMPACT:** Enables silent, automatic hot-swapping of media files if external changes (e.g., render overwrites) occur on disk, without causing synchronization freezes.
- **RESOLUTION:** Bound a `System.IO.FileSystemWatcher` into the MVVM `CueModel`. Dispatched file events (Changed, Created, Renamed, Deleted) across thread boundaries via `System.Windows.Application.Current.Dispatcher.Invoke` to fire `EngineInterop.LoadCueToEngine`, silently hot-swapping the active Rust media source. Hooked `_playlist.CollectionChanged` to properly invoke `IDisposable` memory management on removed cues, preventing background handle leaks.

## [2026-08-07] (Hotfix: FileSystemWatcher E_ACCESSDENIED Debounce)
- **FINDING / DECISION:** Discovered a fatal asynchronous race condition. When an external program overwrites a media file, the OS locks the file for writing. `FileSystemWatcher` instantly fires its event, commanding Rust to read the file, which crashes `MFCreateSourceReaderFromURL` with `E_ACCESSDENIED`.
- **IMPACT:** Re-exporting media from Adobe Premiere or After Effects crashed the live broadcast engine.
- **RESOLUTION:** Implemented an asynchronous lock-check polling gate (`IsFileLocked` with `FileShare.None`) in `CueModel.cs`. The C# brain now safely loops `Task.Delay(500)` on the background thread until the external renderer releases the OS lock, before seamlessly vaulting to the UI thread to command the Rust hot-swap.

## [2026-08-10] (Hotfix: Zero-Frame Hardware Decoder Transition Buffer)
- **FINDING / DECISION:** Discovered that Hardware Decoders take up to 2 seconds of real-time to purge pre-roll frames when jumping to a distant `InPoint` keyframe, because the extraction loop is throttled by the 1-second capacity of the `AudioRingBuffer`. Because `Playlist` instantly killed the outgoing deck on a Hard Cut, the screen was completely black while the new deck warmed up.
- **IMPACT:** A 1-2 second flash of black video on every single Hard Cut or Crossfade (reported as a "jerk").
- **RESOLUTION:** Increased `AudioRingBuffer` size to 10 seconds to allow the decoder to instantly pump the pre-roll frames without blocking. Completely restructured the `Playlist` `tick()` loop to introduce a `pending_fire` state. The outgoing deck is now kept alive and rendering on screen until the incoming deck's `MediaEngine.has_started` atomic boolean flips to `true` (indicating its first valid video frame has successfully landed in VRAM). This guarantees a flawless 0ms transition with zero black frames.
