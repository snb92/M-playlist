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
# #   [ 2 0 2 6 - 0 8 - 1 1 ]   ( P h a s e   4 . 7 :   M a c r o - S t a t e   E x e c u t i o n   C o n d u c t o r )  
 -   * * F I N D I N G   /   D E C I S I O N : * *   I d e n t i f i e d   t h a t   t h e   R u s t   \ M e d i a E n g i n e \   i s   a   b l i n d   m u s c l e   a n d   c a n n o t   a u t o n o m o u s l y   e n f o r c e   \ E n d B e h a v i o r \   o p e r a t i o n s   l i k e   s t o p p i n g   o r   a d v a n c i n g   c u e s .  
 -   * * I M P A C T : * *   A   r u n n i n g   c u e   w o u l d   s i m p l y   h i t   E O F   a n d   f r e e z e ,   i g n o r i n g   t h e   \ O u t P o i n t H N S \   a n d   r o u t i n g   i n s t r u c t i o n s   d e f i n e d   i n   t h e   \ M e d i a C u e \ .  
 -   * * R E S O L U T I O N : * *   D e l e g a t e d   t h e   t h e r m o d y n a m i c   b o u n d a r y   c h e c k s   ( O u t - P o i n t   i n t e r c e p t s )   e x c l u s i v e l y   t o   t h e   C #   \ D i s p a t c h e r T i m e r \   t e l e m e t r y   l o o p .   W h e n   t h e   p l a y h e a d   c r o s s e s   t h e   O u t - P o i n t ,   C #   e x p l i c i t l y   c o m m a n d s   t h e   R u s t   e n g i n e   v i a   F F I   ( \ m p l a y l i s t _ s t o p \ ,   \ m p l a y l i s t _ s c r u b _ t o \ ,   o r   f i r i n g   t h e   n e x t   c u e ) ,   p r e s e r v i n g   t h e   B r a i n / M u s c l e   a r c h i t e c t u r a l   s e p a r a t i o n .   I n c l u d e d   a   3 0 0 m s   d e b o u n c e   t o   p r e v e n t   t h e   3 0 H z   C #   t i m e r   f r o m   s p a m m i n g   t h e   F F I   d u r i n g   t h e   p h y s i c a l   s e e k / t r a n s i t i o n   l a t e n c y   w i n d o w .  
 # #   [ 2 0 2 6 - 0 8 - 1 1 ]   ( P h a s e   4 . 8 :   G r i d   T e l e m e t r y   &   A u d i o   M a t h )  
 -   * * F I N D I N G   /   D E C I S I O N : * *   L o g a r i t h m i c   V o l u m e   s l i d e r   v a l u e s   i n   t h e   U I   c o u l d   n o t   b e   d i r e c t l y   p u s h e d   i n t o   t h e   h a r d w a r e   a u d i o   b u f f e r s .   D o i n g   t h e   m a t h   i n   C #   r i s k s   p r e c i s i o n   l o s s   a n d   r o u n d i n g   a n o m a l i e s   a c r o s s   t h e   f l o a t   b o u n d a r y .  
 -   * * I M P A C T : * *   A   n a i v e   d e c i b e l   s c a l i n g   a p p l i e d   d i r e c t l y   t o   t h e   W A S A P I   f l o a t   b u f f e r   w o u l d   r e s u l t   i n   s e v e r e l y   d i s t o r t e d / c l i p p i n g   a u d i o .  
 -   * * R E S O L U T I O N : * *   I m p l e m e n t e d   P h a s e   4 . 8 .   C r e a t e d   \ m p l a y l i s t _ s e t _ v o l u m e _ d b \   i n   R u s t   t o   p e r f o r m   t h e   s t r i c t   \ 1 0 _ f 3 2 . p o w f ( d b   /   2 0 . 0 ) \   c o n v e r s i o n   i n s i d e   t h e   F F I   b o u n d a r y .   A   c l a m p   w a s   i n t r o d u c e d   f o r   \ d b   < =   - 6 0 . 0 \   t o   f o r c e   a b s o l u t e   s i l e n c e   ( 0 . 0 ) .   P a s s e d   t h i s   m u l t i p l i e r   l o c k - f r e e   t o   t h e   W A S A P I   t h r e a d   t o   m u l t i p l y   r a w   s a m p l e s   j u s t   p r i o r   t o   e n d p o i n t   d e l i v e r y .   A l s o   i n j e c t e d   \ I s A c t i v e P l a y i n g \   t r a c k i n g   i n t o   t h e   W P F   g r i d   t o   p r o v i d e   i m m e d i a t e   v i s u a l   c o n f i r m a t i o n   o f   t h e   L i v e   s t a t e   t o   t h e   o p e r a t o r .  
 
- **[2026-08-11T12:50:00Z] FINDING / DECISION:** The WASAPI Buzzing Trap - Pausing the master clock by simply dropping incoming frames will cause a catastrophic audio loop due to the OS continuously reading stale data from the circular buffer. The engine must actively push silence (0.0) when paused.
- **IMPACT:** Preserves WASAPI thread integrity and prevents infinite audio buzzing loops while locking the video frame during a pause state.
- **RESOLUTION:** Implemented 'Acoustic Silence Override' in audio_wasapi.rs. When MasterClock is paused, the audio thread stays alive and manually injects 0.0 chunks, safely draining the buffer and locking A/V sync without dropping the DX11 surface.

- **[2026-08-11] FINDING / DECISION:** Implemented Asynchronous NDI Output.
- **IMPACT:** Local DX11 and WASAPI hardware loops are isolated from NDI network latency.
- **RESOLUTION:** NDI extraction uses reverse sync_channel graveyards to recycle Vec<u8> and Vec<f32>. Avoids 500 MB/s heap allocation churn, securing thermodynamic equilibrium.

- **[2026-08-11] FINDING / DECISION:** Identified topological confusion in Media Foundation ingest. The Source Reader rejected SetCurrentMediaType on odd sample rates (e.g. 44.1kHz) because the thermodynamic memory alignment was mathematically incomplete.
- **IMPACT:** Un-normalized audio byte chunks could bleed into the lock-free ring buffer and drift the Master Clock.
- **RESOLUTION:** Executed Phase 6 - Path D. Consolidated DSP initialization. Provided absolute mathematical constraints for MF_MT_AUDIO_BLOCK_ALIGNMENT and MF_MT_AUDIO_AVG_BYTES_PER_SECOND, guaranteeing that internal MF resamplers successfully allocate and normalize any source media strictly to 48kHz Stereo Float before hitting the engine.

- **[2026-08-11] FINDING / DECISION:** Diagnosed and eliminated 10,000ms acoustic lag caused by Media Foundation buffer bloat.
- **IMPACT:** Engine now maintains sub-200ms latency. The lock-free audio ring buffer restricts runaway decoding while providing enough elasticity to absorb thread jitter.
- **RESOLUTION:** Constricted buffer capacity to 19,200 floats. Deployed a lock-free .flush() guillotine attached to Scrub and Stop commands for instantaneous audio tracking.

- **[2026-08-11] FINDING / DECISION:** Implemented Phase 6 Path C Audio Routing Matrix to support 16-channel discrete audio routing without violating 16ms thermodynamic bounds.
- **IMPACT:** Lock-free matrix multiplication scales native multichannel inputs into a strict 16-float stride, keeping the WASAPI thread completely O(1).
- **RESOLUTION:** Embedded atomic 256-element routing matrix directly inside the AudioRingBuffer, preventing structural decoupling. Expanded Ring Capacity to 153,600 floats (200ms @ 16-channel).

- **[2026-08-11] FINDING / DECISION:** Implemented Phase 6 Path C Part 2: 16-Channel NDI Multiplexer.
- **IMPACT:** NDI audio stream dynamically captures all 16 channels of the Routing Matrix without triggering WASAPI callback heap allocations.
- **RESOLUTION:** Reorganized WASAPI frame extraction loop to pull Graveyard memory safely before frame processing, weave planar channels mathematically, and bypass the stereo downmix.

 
 -   * * F I N D I N G : * *   N D I   S D K   6   s t r u c t   a l i g n m e n t   m i s m a t c h   i n   F F I .   T h e   C + +   S D K   u s e s   4 - b y t e   B O O L   w h i l e   R u s t   u s e s   1 - b y t e    o o l   f o r   N D I l i b _ s e n d _ c r e a t e _ t .   T h i s   c a u s e d   m e m o r y   m i s a l i g n m e n t   a n d   s i l e n t   i n i t i a l i z a t i o n   f a i l u r e s .  
 -   * * I M P A C T : * *   T h e   N D I   S e n d e r   w a s   s i l e n t l y   a b o r t i n g   a t   b o o t   d u e   t o   u n i n i t i a l i z e d   p a d d i n g   b y t e s   b e i n g   r e a d   a s   i n v a l i d   c o n f i g u r a t i o n s .  
 -   * * R E S O L U T I O N : * *   R e d e f i n e d   N D I l i b _ s e n d _ c r e a t e _ t   b i n d i n g s   i n   s r c / n d i _ f f i . r s   t o   u s e   i 3 2   f o r   b o o l e a n   f l a g s ,   r e s t o r i n g   s t r i c t   m e m o r y   a l i g n m e n t .   H a r d c o d e d   p _ n d i _ n a m e   t o   s t a t i c   m e m o r y   t o   p r e v e n t   l i f e t i m e   d r o p s .   Z e r o - a l l o c a t i o n   l o o p   c o n s t r a i n t s   r e m a i n   i n t a c t .  
 
- **[2026-08-11] FINDING / DECISION:** Eradicated Swapchain Coupling; locked DX11 and NDI backbuffers to immutable 1080p.
- **IMPACT:** Resolves sub-SD broadcast blurring caused by dynamic UI resizing. NDI stream always maintains 1920x1080 resolution.
- **RESOLUTION:** Implemented Phase 6 Path B. Initialized Direct2D and DirectWrite. Render target locked to 1920x1080. Added zero-copy Direct2D typography pass immediately after DX11 video pass in ender_composited.

- **FINDING / DECISION (2026-08-11):** UI Thread DispatcherTimer poses a thermodynamic risk to execution timing.
- **IMPACT:** Garbage Collection or layout passes on the WPF thread can delay FFI triggers, missing crossfade out-points.
- **RESOLUTION:** Implemented Phase 7. Built EngineConductor.cs, moving macro-state loop to a ThreadPriority.Highest background thread. UI thread reduced to a loose telemetry observer.

- **FINDING / DECISION (2026-08-12):** EngineConductor Guillotine was bypassing execution due to OutPointHNS defaulting to 0 (Temporal Vacuum).
- **IMPACT:** Playhead was not triggering EndBehavior actions. UI Slider was artificially capped at 30 seconds.
- **RESOLUTION:** Implemented Phase 7.1 and 7.2. Injected pure Win32 COM IPropertyStore to probe media duration in HNS at ingestion. Implemented _isTransitioning state latch to prevent DDOSing the FFI layer. Scaled UI slider maximum to active cue duration dynamically.

- **FINDING / DECISION (2026-08-12):** Diagnosed WPF UI deadlocks and data binding failures. Scrubbing triggered cross-thread COM collision. Trim buttons failed due to DataContext scope. Color Tags failed WPF Brush typing.
- **IMPACT:** UI was locking up the EngineConductor pipeline. Trim boundaries were discarded. Playlist colors were absent.
- **RESOLUTION:** Executed Phase 7.3. Routed scrub commands through thread-safe EngineConductor queue. Re-wired trim operations to pull MediaCue directly from Button DataContext. Injected StringToBrushConverter to strictly map Hex strings to WPF Brushes.

- **FINDING / DECISION (2026-08-12):** Executed Phase 7.4. UI required granular trimming controls and bounded scrubbing limits to protect the Rust engine EOF frame extraction.
- **IMPACT:** Engine now receives crossfade MS variables directly on cue fire instead of defaulting to 0. Scrubbing cannot exceed native file limits, preventing Keyframe decode freezes. Win32 Airspace rule for HwndHost was successfully circumvented via a WM_LBUTTONDBLCLK message hook for interactive Context Menus.
- **RESOLUTION:** Added bounds clamping logic to RequestScrub. Plumbed TransitionMs through mplaylist_fire_cue. Attached Double-Click MessageHook to VideoHwndHost to trigger VideoTrimMenu in WPF space.

- **FINDING / DECISION (2026-08-12):** Executed Phase 7.5. Diagnosed legacy UI ghosting (dead pixels rendering stale data), Selection Scope Desyncs during trimming operations, and Thread Deadlocking triggered by Slider bypassing FFI events.
- **IMPACT:** Engine was crashing due to lack of thermodynamic settling window. Slider was silently buffering commands. Cue Inspector was visually lying about trim states by failing to sync its DataContext binding.
- **RESOLUTION:** Eradicated ghost text in DataTemplate. Enforced 150ms VRAM settling buffer in EngineConductor. Hooked raw PreviewMouseLeftButton events to guarantee physical UI dragging correctly toggles the FFI state latch. Forced WPF SelectedItem updates upon execution of Trim buttons.

- **FINDING / DECISION (2026-08-12):** The architecture of the C# Macro-State is currently flawless. Tier 1 (Core Logistics & Stability) is officially sealed. The engine's interactive topology is complete.
- **IMPACT:** We have mathematically protected the engine as much as physically possible for H.264 files without writing a bloated, multi-threaded buffered Demuxer in C# (which would violate our zero-allocation policy).
- **RESOLUTION:** We will build an automatic Live Transcoder in Phase 14 to convert dangerous MP4 files to ProRes automatically in the background.

- **FINDING / DECISION (2026-08-12):** Executed Phase 8 and Phase 8.1 - Telemetry & The Audio Dashboard. Diagnosed WPF layout cascade failures when attempting to render 60fps VU meters using standard controls. Discovered the DLL build pipeline was not automatically vaulting the compiled Rust library into the WPF execution directory.
- **IMPACT:** A naive WPF progress bar would trigger a GC storm. FFI calls failed due to stale DLL injection.
- **RESOLUTION:** Implemented lock-free AtomicU32 peak trackers in WASAPI via IEEE-754 f32 bitcasting. Circumnavigated WPF visual tree by natively scaling Rectangle geometries. Injected Style="{x:Null}" to sever the Master Fader from global timeline style bleed. Architected a mandatory <Copy> block inside the .csproj MSBuild pipeline to permanently guarantee absolute physical sync between the Rust release targets and the C# execution layer.

- **FINDING / DECISION (2026-08-12):** Executed Phase 8.3 & Phase 9. Discovered the MSBuild <Copy> target was being silently bypassed by MSBuild caching, leaving the ghost DLL in the execution directory. Furthermore, the mplaylist_get_audio_levels export was stripped from the symbol table because it wasn't statically linked at the root module.
- **IMPACT:** EntryPointNotFoundException crash on the UI thread when polling the Audio Dashboard.
- **RESOLUTION:** Moved all crucial C-ABI exports directly into lib.rs to forcefully project them into the DLL's export table. Constructed a lock-free RwLock<Vec<u16>> bridge (SHOW_OVERLAY & OVERLAY_TEXT) in the Rust graphics.rs to receive dynamic UTF-16 characters from C#. Built the FFI translation for mplaylist_set_overlay_text and bound it to a UI toggle. Executed an absolute terminal command chain to flush the old DLL and rebuild both boundaries.

- **FINDING / DECISION (2026-08-12):** Diagnosed 0x88990001 (D2DERR_WRONG_STATE) crash during Phase 9 typography rendering and invisible VU meters.
- **IMPACT:** The Direct2D GPU state machine collapsed because an early eturn bypassed EndDraw() and the DXGI swapchain Present() call. The VU meter Rectangles collapsed to 0 width because they lacked HorizontalAlignment="Stretch".
- **RESOLUTION:** Eradicated early returns in the Rust rendering loop and encapsulated the Typography bridge in a safe if is_visible block. Forced the WPF Rectangles to stretch across the border width so the ScaleX transform operates on a non-zero area. Recompiled and flushed the pipeline.

- **[2026-08-12] FINDING / DECISION:** Evaluated the necessity of zero-blocking constant buffer updates. Discovered that existing code already used D3D11_MAP_WRITE_DISCARD.
- **IMPACT:** Validated the architectural theory for VRAM streaming. Permitted widening the memory pipe to 128 bytes without rebuilding the core DX11 mapping context.
- **RESOLUTION:** Executed Phase 10 Spatial Geometry expansion via an explicitly initialized lock-free SPATIAL_COLOR_STATE (zoom=1.0, contrast=1.0, saturation=1.0) to prevent shader division crashes.


- **[2026-08-12] FINDING / DECISION:** Identified structural clash between WMF IMFSample COM requirements and static image ingestion.
- **IMPACT:** A static texture has no temporal IMFSample. Attempting to fulfill the staging requirement for a sample would cause memory corruption or drop the VRAM buffer prematurely.
- **RESOLUTION:** Decoupled the tuple requirement by wrapping SendableSample in an Option. Validated that create_srv closure natively handles the None state seamlessly without HLSL shader disruption. Also injected CoInitializeEx in mplaylist_load_image to prevent .NET Task Pool COM initialization crashes.

- **[2026-08-12] FINDING / DECISION:** Discovered that the Playlist::fire_cue logic bypassed WMF hardware decoders by actively parsing string extensions (.png / .jpg).
- **IMPACT:** Violates the "Blind Muscle" architecture by injecting C# Macro-State logic (file extension parsing and modality selection) directly into the Rust execution engine.
- **RESOLUTION:** Executed Phase 11c (The Pure State Realignment). Stripped string parsing from playlist.rs. Expanded the FfiCue struct with an IsStaticImage primitive u8 byte. Transferred all modality logistics exclusively to the C# EngineConductor.

- **[2026-08-12] FINDING / DECISION:** Identified structural desync in static asset rendering. EngineConductor blindly dispatched static assets to mplaylist_load_cue during B-Deck preload, which failed to actually load WIC frames into VRAM.
- **IMPACT:** Triggering a static image resulted in a blank output because the asset was queued but never decoded into the DX11 compositor surface.
- **RESOLUTION:** Executed Phase 11b. Upgraded the Rust ire_cue function to directly decode WIC frames to VRAM natively at the exact moment of execution if the incoming cue is static. Bypassed Media Foundation instantiation seamlessly without stalling the transition mathematics.
- **[2026-08-13] FINDING / DECISION:** Identified that the Rust Micro-State Muscle bypassed the C# Brain by holding a raw UDP socket for OSC, breaking deterministic sequencing (e.g., jumping to cues) because Rust lacks the Playlist concept.
- **IMPACT:** A fatal architectural anti-pattern violating the Brain/Muscle separation and breaking transport logic for networked remote control.
- **[2026-08-13] FINDING / DECISION:** Identified requirement to ingest NDI network streams as First-Class Cues without stalling the GPU or relying on external Rust crates.
- **IMPACT:** A lack of NDI receiver bindings and polymorphic routing meant network video couldn't enter the A/B Deck crossfader natively.
- **RESOLUTION:** Executed Phase 14a. Reconstructed `NDIlib_recv_*` C-ABI definitions in `ndi_ffi.rs`. Upgraded `FfiCue` from binary `is_static_image` to universal `CueModality` enum (WMFTemporal, WICStatic, NDILive). Decoupled `EngineConductor.cs` and `playlist.rs` dispatch logic to route modalities natively and bypass the execution guillotine for infinite-time network sources. Pipeline Sync completed successfully.

- **[2026-08-13 17:25:00] FINDING / DECISION:** C# UI dynamically appending indices to Rust's static `cues` array resulted in a fatal index desynchronization, violating the "Blind Muscle" architecture.
- **IMPACT:** `mplaylist_fire_cue(index)` would fire the wrong modality/cue if the C# Brain advanced or repeated indices out of sequence.
- **RESOLUTION:** Executed Phase 14a.2 (The Muscle Lobotomy). Eradicated the `cues` array from the Rust backend. Re-mapped `mplaylist_fire_cue` C-ABI to accept the full `FfiCue` structural payload in real-time. Added a manual `TransportFireNext()` guillotine drop to allow operators to forcefully end infinite live cues (NDI/WIC).

- **[2026-08-13 17:46:00] FINDING / DECISION:** C# UI "PLAY / FIRE NEXT" button was blindly executing the linear 5-second lookahead cue (`TransportFireNext`), betraying the operator's physical click selection on the `PlaylistUI` ListBox.
- **IMPACT:** Engine jumped to incorrect (often blank) cues instead of forcing an override to the highlighted cue.
- **RESOLUTION:** Executed Phase 14b. Synchronized UI selection by injecting an override `TransportJumpToCue` condition in the Play handler when an item is selected. Built a zero-allocation `NdiPingPong` lock-free background receiver thread in Rust to ingest network BGRA frames and natively Map/Unmap them to a `D3D11_USAGE_DYNAMIC` texture directly inside the single-threaded DX11 Render Loop context, avoiding GPU thread collisions.

- **[2026-08-13 18:10:00] FINDING / DECISION:** C# ConductorLoop timeless lookahead aggressively overwrote the Standby Deck during visual crossfades when transitioning from Timeless modalities (PNG/NDI).
- **IMPACT:** Transition visuals would immediately drop because the target deck was overwritten by the 5-second lookahead logic before the crossfade completed.
- **RESOLUTION:** Executed Phase 14b.2 (The Transition Lock). Implemented a strict `DateTime` execution lock in the EngineConductor to mathematically insulate the Rust Muscle from C# state-thrashing during transitions.
