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

## ❌ Left to Build (To Be Scheduled)

### Media & Inputs
- [ ] **Still Images & PDF Files**: Static asset rendering and PDF rasterization.
- [ ] **Live Cameras**: UVC webcam and Blackmagic Design (DeckLink) capture ingestion.
- [ ] **Network Streams (Ingest)**: NDI and Syphon *Receivers*.
- [ ] **Browser Cues & Window Source Cues**: Chromium embedding (CEF) and Desktop Duplication API.

### Playback & Cue Management
- [ ] **Cue IDs, Search & "Goto" Target Commands**: Logical timeline router.
- [ ] **Advanced Looping & Play Count**: Loop iteration tracking and exit behaviors.
- [ ] **Playback Speed**: Dynamically manipulating Media Foundation playback rate.
- [ ] **Geometry & Color Controls**: Video cropping, pan/zoom, and live RGB pixel shaders.
- [ ] **Color Tags & Notes**: UI/Metadata attributes for the Cue list.

### Core Engine & Workflow
- [ ] **Automagic File Tracking**: Filesystem watching for moved/renamed files.
- [ ] **Live Transcoding**: Built-in FFmpeg/ProRes conversion.
- [ ] **Bundle Playlist**: Packaging JSON and copying media to a portable directory.
- [ ] **Checkerboard Preview**: UI background brush for Alpha channels.

### Video Output
- [ ] **Multi-Display Output**: Spawning borderless fullscreen Win32 windows on secondary monitors.
- [ ] **SDI Playout with Key & Fill**: Blackmagic DeckLink SDK hardware SDI output.
- [ ] **Corner Pinning & Edge Blending**: Advanced projection mapping vertex shaders.

### Audio Capabilities
- [ ] **Audio Normalization**: LUFS/RMS analysis and auto-gain leveling.
- [ ] **Visual Waveforms & Level Meters**: Extracting peak data from WASAPI to render VU meters in WPF.

### Integrations & Sync
- [ ] **ATEM Switcher Integration**: TCP/UDP networking to control Blackmagic ATEM switchers.
- [ ] **HyperDeck Emulation**: TCP server for standard HyperDeck commands.
- [ ] **NDI Triggering**: Reading NDI tally data to auto-play/pause.
- [ ] **Network Sync & Timecode Follower (MTC/LTC)**: MIDI Timecode ingestion and multi-machine clock locking.
- [ ] **MIDI & DMX**: Integrating MIDI input and Art-Net / sACN.
