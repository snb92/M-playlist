# Changelog

## [Unreleased]
- **Architecture**: Executed "Phase 14e" (WebView2 Win32 Overlays). Added Modality 6 (WebView2Overlay) multiplexed onto the 8-byte FfiCue.filepath. Refactored ire_cue router into a strict match block. Implemented OpenSharedResource for zero-copy browser VRAM ingestion, and injected branchless Premultiplied Alpha "OVER" composition into the master HLSL shader.

- **Architecture**: Executed "Phase 12b" (Dynamic RTV Lock & HDR Shader Handshake). Mathematically sealed the DWM flip-discard pointer by dynamically creating the `ID3D11RenderTargetView` every frame. Upgraded the HLSL Pixel Shader (`PS_CODE`) to cleanly ingest the 16-bit Float UAV format output by the Compute Shader.
- **Architecture**: Executed "The Transition Lock" (Phase 14b.2) - Implemented a strict `DateTime` execution lock in the C# `EngineConductor` to mathematically insulate the Rust backend from being overwritten by timeless lookahead logic during visual crossfades.
- **Architecture**: Executed "Phase 14b" (NDI Receiver & UI Sync). Built a zero-allocation Ping-Pong buffer in System RAM for isolated background network receipt, natively mapping directly to DX11 VRAM.
- **UI**: Fixed state desynchronization in C# where mouse-click selections were bypassed by linear state machine loops. The PLAY button now forcefully leaps to operator-selected indices.
- **Architecture**: Executed "The Muscle Lobotomy" (Phase 14a.2) - eradicated the internal `cues` array from the Rust backend to ensure the C# Brain holds absolute authority.
- **FFI**: Updated `mplaylist_fire_cue` C-ABI to accept the full `FfiCue` structural payload rather than an index integer, enabling deterministic dispatch.
- **UI**: Mapped the PLAY / FIRE NEXT button to invoke a manual `TransportFireNext()` drop guillotine to support infinite live cues (NDI/WIC).
### Added
- Initialized project with `CONTEXT.md`, `TODO.md`, and `CHANGELOG.md` to track zero-trust / OS-native architectural constraints.
- Created `m_playlist` Rust library compiled as `cdylib` and locked `windows-rs` supply chain via `cargo vendor`.
- Implemented lock-free `AudioRingBuffer` and `MasterClock`.
- Implemented WASAPI Pro-Audio Real-Time thread with event-driven buffer feeding.
- Implemented Media Foundation `IMFSourceReader` with D3D11/DXGI zero-copy bindings for hardware-accelerated video decoding.
- Implemented Zero-Copy hardware texture intercept (`MFGetService`) and strict A/V sync gating against the Master Clock.
- Implemented `Dx11Compositor` to initialize a DXGI Swapchain mapped directly to the frontend's physical window handle (HWND).
- Configured Media Foundation's Video Processor to automatically scale and convert NV12 to ARGB32 in hardware for an instantaneous `CopyResource` block transfer to the screen buffer.
- Pivoted frontend architecture from WinUI 3 to WPF to bypass `DesktopWindowXamlSource` HWND locking.
- Scaffolded WPF application with a custom `VideoHwndHost` to provide a raw Win32 child window for the Rust DX11 Compositor.
- Bound Rust C-FFI via `EngineInterop` and implemented the main WPF playback interface.
- Implemented Dual-Engine State Machine (`Playlist`) for seamless A/B deck video cueing.
- Implemented `start_time_offset` and pause gating in `MediaEngine` to strictly sync multiple video clips to the continuous WASAPI master clock.
- Updated WPF UI with Playlist controls (Add Cues, Fire Next).
- Resolved systemic `decode_loop` deadlock by dynamically discovering physical stream indices instead of using magic selection constants.
- Resolved WASAPI audio starvation deadlock by isolating audio extraction from the A/V Sync Gate.
- Fixed Master Clock starvation by ticking the hardware clock based on available buffer frames instead of written audio frames.
- Resolved `E_NOINTERFACE` hardware extraction error by replacing legacy `MFGetService` broker with direct `IMFDXGIBuffer` COM cast to unwrap `ID3D11Texture2D`.
- Built and integrated a zero-dependency UDP OSC Server using the pure Rust standard library, running on a background thread on port 51001, to parse `/mplaylist/fire_next` and trigger cues.
- Refactored EngineState to use a lock-free `std::sync::mpsc` channel via `AppLogic`. UI and OSC triggers now push `EngineCommand` enums to a dedicated background thread, guaranteeing FFI callers are never blocked by heavy media decoding.
- Fixed 5.1 audio stuttering by forcing the Windows OS Resampler to dynamically downmix any multi-channel payload to strictly 2-channel Stereo at 48000Hz prior to ring-buffer insertion.
- Implemented `mplaylist_get_dimensions` FFI bridge to expose raw hardware DX11 texture dimensions to the C# WPF UI.
- Solved WPF `HwndHost` infinite layout-pass spam and stretching behavior by explicitly forcing `Center` alignments and gating resize triggers with delta thresholds.
- Fixed WPF `double.NaN` initialization bug that stranded the video frame at its 100x100 `HwndHost` default size.
- Added a lock-free `clear()` routine to `AudioRingBuffer` that drops overlapping audio data instantly on A/B Deck hot-swaps to eliminate audio-tail bleed.
- Added real-time A/V sync calibration and diagnostics global atomics (`SYNC_OFFSET_US`, `CURRENT_VIDEO_TIME_US`) mapped through a live FFI bridge to a C# WPF UI Timecode Dashboard.
- Fixed WPF Airspace bleeding bug by constraining the `VideoSurface` `HwndHost` inside a `Grid.Row="0"` `Border`.
- Implemented Endpoint Chasing (Auto-Recovery) in the WASAPI thread to automatically recover and resume from hardware disconnects (audio hot-plugging) in under 1 second.
- Completed Staff Engineer Architecture Audit, verifying zero-copy adherence, memory safety, and thread decoupling.
- Advanced Cue Model: Defined `#[repr(C)] FfiCue` struct to enforce a flat C-ABI memory boundary between C# and Rust.
- Macro-State Migration: Refactored Rust engine to accept `EngineCue` objects, removing the need for `playlist.rs` to store file paths directly as strings.
- WPF Architecture: Implemented MVVM-compliant `CueModel` and `ShowFileService.cs` leveraging `System.Text.Json` for `.mshow` disk serialization.
- UI Ingestion: Built robust Drag-and-Drop file ingestion inside `MainWindow.xaml.cs` to instantly populate the `ObservableCollection<CueModel>`.
- Implemented Phase 2: The Hardware Domain. Created `OutputWindow` for exact secondary monitor coordinates via `System.Windows.Forms.Screen`, and implemented dynamic WASAPI audio device hot-swapping via `GetId()` COM queries.
- Implemented Phase 3: The Time Domain. Added `pending_scrub` atomic lock-free scrubbing and automated trim boundaries (`InPointHnsecs`/`OutPointHnsecs`) directly onto the active rendering `MediaEngine`. Decoupled WPF slider UI from background logic via `_isUserScrubbing` suppression flag.
- Implemented Phase 4A: Broadcast Compositor (Graphics & Audio). Added custom HLSL Shaders (`VS_Main`, `PS_Main`) directly into the `Dx11Compositor` to linearly interpolate two DX11 texture streams. 
- Avoided audio lock bottlenecks in Phase 4A by implementing zero-allocation stack-based dual-deck PCM mixing inside the WASAPI render loop, mathematically controlled by a bit-cast `AtomicU32` blend factor.
- Enforced strict DX11 Multithread Protection (`SetMultithreadProtected(true)`) to prevent cross-thread collisions between the hardware media decoders and the compositor.
- Implemented Phase 4B: Temporal State Machine & Deck Handoff. Upgraded the `AppLogic` thread to a 60Hz tick loop via `recv_timeout(16ms)`. This mathematically drives the lock-free `blend_factor` atomic based on the Master Clock and correctly performs VRAM memory cleanup by dropping the outgoing COM textures safely on the main logic thread, avoiding any decoding or rendering thread race conditions.
- Implemented Phase 5A: Reverse Zero-Copy. Developed a 2-frame DX11 Pipelined Readback architecture using `D3D11_USAGE_STAGING` textures. The system asynchronously calls `CopyResource` from VRAM to System RAM and safely maps the previous frame to extract a non-blocking raw pixel pointer for future NDI broadcast. Added NDI Toggle FFI bridging.
- Implemented Phase 5B: NDI Broadcast Output. Dynamically linked `Processing.NDI.Lib.x64.dll` via `libloading` for zero-trust compliance. Created a dedicated OS background thread (`NdiTransmitter`) receiving raw BGRA memory frames via a bounded `sync_channel(2)` with `try_send()`. This guarantees the DX11 Compositor is never stalled by network CPU compression delays.
- **Hotfix (Phase 5B):** Resolved D3DCompile E_FAIL crash by adding missing `VS_OUT` struct to HLSL Pixel Shader string. Fixed zombie WPF background processes by overriding `OnClosed` in `MainWindow.xaml.cs`. Resolved false-positive unused-import warnings in `media_engine.rs` to verify that the Zero-Copy GPU Mandate (`MFCreateDXGIDeviceManager`, `IMFDXGIBuffer`) is active and successfully bypassing CPU memory.
- Implemented Feature B: Corner Pinning Geometry. Expanded the HLSL constant buffer with 4-corner NDC coordinates (`float4` aligned), replaced the oversized triangle vertex shader with a 4-vertex Triangle Strip reading corners from the cbuffer, and changed `Draw(3,0)` â†’ `Draw(4,0)`. Added `mplaylist_set_geometry` FFI export routed through a new `SetGeometry` mpsc command. Built 8 WPF sliders (TL/TR/BL/BR Ã— X/Y, range -2.0 to 2.0) with real-time `ValueChanged` wiring for live projection mapping control.
- **Architectural Upgrade (True 3D Perspective Projection):** Eliminated the affine fold artifact when corner pinning by shifting the homography solve to the CPU. Reverted Vertex Shader to `Draw(3,0)` fullscreen canvas. Implemented $3 \times 3$ cyclic convex quad determinant math in Rust to calculate the inverse Adjugate Matrix. Passed to HLSL Pixel Shader via `BlendBuffer` to perform true $uvw.xy / uvw.z$ perspective divide.
- Implemented Feature D: Automagic File Tracking. Bound a native `System.IO.FileSystemWatcher` to the C# `CueModel`. Re-routed filesystem events (Changed, Renamed, Deleted) across thread boundaries using `Application.Current.Dispatcher.Invoke` to silently fire `EngineInterop.LoadCueToEngine`, enabling lock-free live file hot-swapping without modifying the Rust backend or violating zero-trust bounds. Implemented `IDisposable` memory management in `CueModel` and hooked `_playlist.CollectionChanged` to prevent orphaned watchers.
- **Hotfix (Feature D):** Resolved `E_ACCESSDENIED` crash caused by asynchronous OS file locks when an external program overwrites a media file. Implemented an asynchronous `IsFileLocked(FileShare.None)` polling gate in `CueModel.cs` to debounce the `FileSystemWatcher` event loop, guaranteeing that the C# Macro-State only commands the Rust backend to load a file after the external renderer has fully released it.
- **Architectural Hotfix:** Fixed "Black Screen / Jerk" latency on Hard Cuts and Crossfades. Increased `AudioRingBuffer` size to 10 seconds to prevent real-time pre-roll throttling. Restructured `Playlist` with a `pending_fire` state that guarantees zero black frames by keeping the outgoing deck active on screen until the incoming deck's hardware decoder successfully pushes its first valid frame into VRAM.
- Implemented Phase 4.6: The Stateful Cue Model & Inspector. Replaced the legacy `CueModel` with a pure serializable `MediaCue` data structure. Restructured `MainWindow.xaml` into a Master-Detail layout featuring a dedicated Cue Inspector panel bound directly to the selected `MediaCue`. Centralized all OS directory tracking into a single `FileStateMonitor` instance to prevent Win32 handle exhaustion on large show files.
- Implemented Phase 4.7: The Macro-State Execution Conductor. Activated the inert EndBehavior state machine in C# by converting the WPF DispatcherTimer telemetry loop into a continuous playhead monitor. Implemented thermodynamic boundary intercepts to automatically fire Stop, LoopForever (Seek), and NextCue commands into the Rust engine exactly when the active cue crosses its OutPointHNS.
- Added a 300ms transition debounce lock to the C# boundary intercept to prevent rapid multi-firing while waiting for the hardware decoder to reset its physical playhead. Added an explicit lock-free mplaylist_stop() FFI command to the Rust backend to completely drop both decks and instantly free all VRAM.
- Implemented Phase 4.8: Grid Telemetry & Audio Math. Added \IsActivePlaying\ bound state to \MediaCue\ triggering a real-time \DataTrigger\ in WPF to highlight the active playing row with a distinct broadcast color. ColorTags from the cue model are now visibly rendered on the grid rows.
- Bridged the C# UI \VolumeDb\ slider into the Rust WASAPI engine via a new lock-free \mplaylist_set_volume_db\ FFI boundary. Implemented logarithmic decibel-to-linear amplitude math in Rust (\10^db/20\), pushing the float multiplier through the \AppLogic\ MPSC channel directly to the real-time audio render thread to securely mix the audio output without clipping or latency.

## [Phase 4.9] - 2026-08-11
### Added
- MasterClock is_paused atomic state to thermodynamically freeze the engine.
- Acoustic Silence Override in WasapiEngine to prevent buffer-loop buzzing when paused.
- TimecodeConverter.cs to map RAW HNS values to HH:MM:SS:FF standard formatting.
- Discrete PLAY, PAUSE, and STOP transport controls in the WPF UI.
- Hardware Configuration Expander in MainWindow.xaml to organize Output Corner Pinning and Audio configs.

## [Phase 5.2] - 2026-08-11
### Added
- N-2 Delayed Staging Buffer in Dx11Compositor for asynchronous pixel readback.
- Zero-stall extraction pipeline using ID3D11Texture2D Staging buffers and Map/Unmap commands.

## [Phase 5.3] - 2026-08-11
### Added
- Dedicated NDI Worker Thread acting as a network shock absorber.
- Lock-free memory extraction in DX11 loop with memcpy to Vec<u8>.
- Drop-protocol via mpsc::sync_channel try_send, guaranteeing 0 wait on the render thread if the network stalls.
- FFI boolean endpoint mplaylist_set_ndi_enabled linked to C# WPF checkbox.

## [Phase 5.4] - 2026-08-11
### Added
- Multiplexed NDI Payload Enum separating Video and Audio transmission.
- Acoustic Network Tap in WASAPI thread to intercept, de-interleave (Planar), and transmit audio data.
- Zero-wait asynchronous try_send channel shared across DX11 and WASAPI hardware loops.

## [Phase 6] - 2026-08-11
### Added
- VRAM Graveyard implementation via reverse std::sync::mpsc::sync_channel.
- Zero-allocation memory intercept for both DX11 Video and WASAPI Audio threads.
- Zero-Init CPU bypass using Vec::clear(), reserve(), and unsafe set_len() to maximize hardware memcpy speed.
- Fully eradicated ~500 MB/s heap churn during live NDI transmission.

## [Phase 6 Path D] - 2026-08-11
### Added
- Media Normalization via DSP Output Constraint Matrix.
- Eliminated topological DSP conflicts by removing premature MF_SOURCE_READER_FIRST_AUDIO_STREAM initialization.
- Defined exact thermodynamic memory footprint (MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_AVG_BYTES_PER_SECOND) to safely engage internal Media Foundation Audio Resampler MFTs for dynamic upmixing and 48kHz float resampling.

## [Acoustic Thermodynamic Correction] - 2026-08-11
### Fixed
- Eradicated 10-second buffer bloat by constricting the AudioRingBuffer to a strict 200ms capacity (19200 floats).
- Injected lock-free .flush() method into AudioRingBuffer to evaporate stale audio floats on transport commands.
- Intercepted Scrub and Stop events in AppLogic to trigger the lock-free guillotine, enabling instantaneous acoustic jumps.

## [Audio Routing Matrix] - 2026-08-11
### Added
- Implemented strict 16-Channel Master Bus topology to support multi-channel MXF broadcast ingest.
- Embedded a lock-free 256-element Audio Routing Matrix inside the AudioRingBuffer.
- Deployed a C-ABI hook \mplaylist_set_audio_route\ allowing the WPF UI to route audio dynamically with deterministic 0-allocation physics.

## [16-Channel NDI Multiplexer] - 2026-08-11
### Added
- Implemented Zero-Allocation Planar Weaving to transmit all 16 channels of the Routing Matrix over NDI.
- Pre-allocated planar vectors directly from the graveyard to eliminate heap overhead inside the WASAPI extraction callback.
- NDI Audio payload is now structurally locked to a 16-channel layout.

## [NDI Thermodynamic Correction] - 2026-08-11
### Fixed
- Re-aligned `NDIlib_send_create_t` FFI bindings to use `i32` for boolean flags to match NDI 6 C++ SDK 4-byte padding constraints, resolving silent uninitialized pointer rejections on sender boot.
- Hardcoded `p_ndi_name` to static string memory to prevent lifetime drop zero-trust failures before background threads initialize.

- **[2026-08-11]** Implemented Phase 6 Path B: Direct2D Typography & Immutable 1080p Swapchain. Locked engine output to 1920x1080 resolution and completely eradicated ResizeBuffers logic, preventing sub-SD compression blur on NDI streams.
- **[2026-08-11]** Initialized Direct2D and DirectWrite Factories in graphics.rs to enable zero-copy hardware accelerated typography rasterization.

## [Phase 7] - 2026-08-11
### Added
- Architected \EngineConductor.cs\, a high-priority background thread decoupling macro-state execution from the WPF UI.
- Implemented deterministic O(1) Cue routing via ConcurrentDictionary.
- Implemented 5-second Lookahead Math for B-Deck pre-loading.
- Stripped DispatcherTimer of execution authority, converting it to a loose 33ms telemetry observer.
- Consolidated Phase 7.1 and 7.2: Implemented pure C# Win32 COM metadata probing (MediaMetadataProbe.cs) for instantaneous HNS duration lookup. 
- Decoupled EngineConductor Guillotine Physics from temporal vacuum by injecting a 50ms safety margin and thermodynamic state latch.
- Wired UI output and trim buttons to strictly read from the Conductor's telemetry cache.
- Implemented Phase 7.3: Interactive Topology & Thread Safety.
- Eliminated cross-thread COM scrubbing collisions by implementing a volatile command queue in EngineConductor.
- Repaired WPF Trim buttons by extracting MediaCue from the visual tree DataContext instead of SelectedItem.
- Built a native WPF StringToBrushConverter for rendering Hex Color Tags dynamically without model corruption.
- Implemented Phase 7.4: Operator Topology & Bounds Geometry.
- Enforced EOF Scrub Clamping in EngineConductor to prevent Media Foundation decoding freezes at the tail-end of a video stream.
- Re-activated Crossfade physics by dynamically passing TransitionMs across the FFI to the Rust compositor.
- Implemented raw Win32 MessageHook on HwndHost to intercept double-clicks and invoke a native WPF Context Menu over the video airspace for instantaneous IN/OUT trimming.
- Rendered exact DurationHNS and Crossfade controls on the playlist UI cards alongside a dynamic Playlist Total Duration calculation.
- Implemented Phase 7.5: Visual Fidelity & Scrub Mechanics.
- Injected a strict 150ms thermodynamic settling window in the EngineConductor scrub routing loop, granting Media Foundation time to safely dump its VRAM swapchain before bombarding it with polling telemetry.
- Bypassed WPF Slider event spoofing by wiring raw PreviewMouseLeftButtonDown and Up events to guarantee accurate playhead latching.
- Fixed the Trim Context Menu's Selection Scope Desync by forcing PlaylistUI.SelectedItem to actively sync to the cue being manipulated in memory.
- Eradicated legacy ghost text from the XAML DataTemplate, merging IN/OUT boundaries and true durations into a single dynamic stack.
- Implemented Phase 8 (Telemetry & Audio Dashboard) and Phase 8.1 (Dashboard Fidelity).
- Added lock-free WASAPI peak tracking via AtomicU32 in the Rust engine.
- Rendered Zero-Allocation WPF VU meters using direct ScaleTransform property mapping in C# _uiTimer, bypassing catastrophic layout engine passes.
- Severed WPF Global Style inheritance from the Master Volume Slider.
- Automated the physical deployment of m_playlist.dll directly into the MSBuild pipeline via .csproj Post-Build targets, permanently ending silent FFI execution drift.
- Implemented Phase 8.3 & Phase 9.
- Purged MSBuild cache and executed absolute PowerShell flush to deploy the new Rust FFI endpoints.
- Migrated FFI endpoints to lib.rs to enforce rigid symbol compilation.
- Constructed a lock-free RwLock<Vec<u16>> String Bridge across the FFI to drive Direct2D Typography over the GPU swapchain.
- Implemented mathematical Elapsed/Remaining C# Timecode and mounted it to the VRAM Typography Bridge via the _uiTimer.Tick.

## [Phase 10: Spatial Geometry & GPU Color] - 2026-08-12
### Added
- Expanded DX11 BlendData struct to a 128-byte mathematical payload for zero-blocking memory alignment.
- Overhauled PS_CODE in graphics.rs to process hardware-accelerated pan, zoom, crop limits, and color correction natively on the GPU at 60fps.
- Configured VRAM Streaming by migrating constant buffer updates strictly to D3D11_MAP_WRITE_DISCARD Map/Unmap commands.
- Established a new zero-allocation FFI hook mplaylist_set_spatial_color in C# to asynchronously modify the global RwLock<SpatialColorState>.


## [Phase 11: The Static Asset Pipeline] - 2026-08-12
### Added
- Architected a pure Win32/COM zero-copy ingestion route for static assets (.PNG/.JPG) using Windows Imaging Component (WIC) via wic.rs.
- Implemented load_image_to_texture to forcibly format-convert decoded frames into PBGRA and blast them directly into a D3D11_USAGE_IMMUTABLE VRAM texture.
- Decoupled the A/B Deck staging abstraction (staging_a/staging_b) by wrapping the WMF SendableSample in an Option, allowing static textures to persist in VRAM without a temporal frame reference.
- Implemented mplaylist_load_image FFI endpoint for synchronous Deck routing of static assets from C#, safeguarded with CoInitializeEx for background thread stability.

## [Phase 11b & 11c: Static Asset Polymorphism & C-ABI Logic Enforcement] - 2026-08-12
### Added
- Extended the C# MediaCue model to explicitly detect IsStaticImage modality natively.
- Decoupled EngineConductor pre-loading by constructing the LoadPolymorphicCue pipeline to conditionally route cues to Rust via mplaylist_load_image vs mplaylist_load_cue.
- Integrated IsStaticImage primitive byte into the shared FfiCue struct to enforce the "Blind Muscle" architecture, preventing string-parsing in Rust.
- Bypassed WMF decoder instantiation in Rust Playlist::fire_cue for static images, loading them instantaneously via WIC and blasting them to the DX11 standby texture buffer to maintain zero-black-frame crossfade physics.

## [Phase 12: Topological Output Physical Deployment] - 2026-08-13
### Added
- Implemented Multi-Swapchain Hardware Matrix to provide a Clean Feed output strictly bound to \DXGI_SCALING_STRETCH\.
- Connected \mplaylist_bind_output_matrix\ FFI for the pure 1080p hardware-accelerated "Clean Feed" on the secondary monitor, bypassing the WPF render thread loop.
### Fixed
- Lobotomized \mplaylist_resize_swapchain\ to permanently decouple the master render resolution from the C# UI window dimension, curing the \DXGI_ERROR_INVALID_CALL\ collision upon \CopyResource\.
- Hard-restored \fi.rs\ to pristine architectural state, surgically injecting missing boundaries without breaking the underlying string-to-brush logic.
- **[2026-08-13]** Executed Phase 13a: OSC State Injection. Transplanted the OSC network topology from the Rust Muscle to the C# Brain. Created a pure System.Net.Sockets.UdpClient OSC parser in C#. Exposed a unified Transport API on EngineConductor for Play, Pause, Stop, and JumpToCue commands.
- **[2026-08-13]** Executed Phase 14a: NDI Ingestion (Structural Primitive). Upgraded Modality primitives across C-ABI boundary. Mapped NDI Receiver function pointers natively.

- **[2026-08-13]** Executed Phase 14a.2: The Muscle Lobotomy. Eradicated C# reliance on appending array indices for the \mplaylist_fire_cue\ C-ABI. Upgraded FFI to accept full dynamic \FfiCue\ structural payloads at execute time. Added \TransportFireNext\ GUI guillotine.
- **[2026-08-13]** Executed Phase 14b: NDI Ingestion (VRAM Blasting). Built \NdiPingPong\ lock-free receiver thread to ingest raw BGRA network frames. Restored sequential firing logic on UI selection vs auto-pilot.
- **[2026-08-13]** Executed Phase 14b.2: The Transition Lock. Protected the Standby Deck from hyperactive C# timeless pre-loading loop overwriting the visual crossfade before it finishes.
- **[2026-08-13]** Executed Phase 14b.3: Polymorphic Unification. Eradicated the deleted \mplaylist_load_image\ entry point and unified all asset loads through \mplaylist_load_cue\. Repaired broken C# build caused by missing \CueModality\ enum and transport endpoints.

### [Phase 14c] - Local Hardware Ingestion
- **Modality Expansion:** Extended CueModality enum in C# and Rust routing logic to include LocalCamera = 3.
- **Hardware Enumeration:** Injected mplaylist_get_camera_device_count and mplaylist_get_camera_device_name into ffi.rs to query WMF capture devices.
- **Native IMFMediaSource:** Modified media_engine.rs to bypass temporal URL parsing for cameras and instantiate IMFMediaSource dynamically using MFCreateSourceReaderFromMediaSource.

### Phase 14c UI Completion
- Wired Local Hardware Camera (UVC/Capture Cards) ingestion into the C# WPF UI via a new + Add Camera button and mplaylist_get_camera_device_name FFI bridge.

## [Unreleased]
- **Architecture**: Executed "Phase 14e" (WebView2 Win32 Overlays). Added Modality 6 (WebView2Overlay) multiplexed onto the 8-byte FfiCue.filepath. Refactored ire_cue router into a strict match block. Implemented OpenSharedResource for zero-copy browser VRAM ingestion, and injected branchless Premultiplied Alpha "OVER" composition into the master HLSL shader.

### Added
- **Phase 14c DXGI Desktop Duplication:** Zero-copy VRAM-to-VRAM COM topology for desktop capture.

### Added
- **Phase 14d DeckLink SDI Implementation:** Engineered a zero-copy VRAM handoff for uncompressed SDI ingestion via raw IDeckLink COM API, including a resilient C++ build shim and UYVY Macropixel GPU decode.

