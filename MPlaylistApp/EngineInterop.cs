using System;
using System.Runtime.InteropServices;

namespace MPlaylistApp
{
    [StructLayout(LayoutKind.Sequential)]
    public struct FfiCue
    {
        public IntPtr FilePath;
        public long InPointHnsecs;
        public long OutPointHnsecs;
        public byte IsLooping;
        public byte HoldLastFrame;
        public byte AudioRouting;
        public long TransitionDurationHnsecs;
        public byte Modality;
        public byte HardwareIndex;
    }

    public static class EngineInterop
    {
        private const string DllName = "m_playlist.dll";

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_init();

        [System.Runtime.InteropServices.DllImport("m_playlist.dll", CallingConvention = System.Runtime.InteropServices.CallingConvention.Cdecl)]
        public static extern bool mplaylist_check_device_lost();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_shutdown();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_stop();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_volume_db(float db);
        [DllImport("m_playlist.dll", CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_blend(float val);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_pause();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_resume();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_set_window(IntPtr hwnd);


        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_load_cue(FfiCue cue);

        public static void LoadCueToEngine(MediaCue model)
        {
            IntPtr ptr = Marshal.StringToCoTaskMemUTF8(model.FilePath);
            try
            {
                var ffiCue = new FfiCue
                {
                    FilePath = ptr,
                    InPointHnsecs = (long)model.InPointHNS,
                    OutPointHnsecs = (long)model.OutPointHNS,
                    IsLooping = (byte)(model.IsLooping ? 1 : 0),
                    HoldLastFrame = (byte)(model.HoldLastFrame ? 1 : 0),
                    AudioRouting = 0,
                    TransitionDurationHnsecs = (long)(model.TransitionDuration * 10000000.0),
                    Modality = (byte)model.Modality,
                    HardwareIndex = (byte)0 // Default to device 0, UI logic can update this later
                };
                mplaylist_load_cue(ffiCue);
            }
            finally
            {
                Marshal.FreeCoTaskMem(ptr);
            }
        }

        public static System.Collections.Generic.List<string> GetAudioDevices()
        {
            var devices = new System.Collections.Generic.List<string>();
            uint count = mplaylist_get_audio_device_count();
            for (uint i = 0; i < count; i++)
            {
                byte[] buffer = new byte[256];
                GCHandle handle = GCHandle.Alloc(buffer, GCHandleType.Pinned);
                try
                {
                    uint bytesWritten = mplaylist_get_audio_device_name(i, handle.AddrOfPinnedObject(), 256);
                    if (bytesWritten > 0)
                    {
                        string name = System.Text.Encoding.UTF8.GetString(buffer, 0, (int)bytesWritten);
                        devices.Add(name);
                    }
                    else
                    {
                        devices.Add($"Unknown Device {i}");
                    }
                }
                finally
                {
                    handle.Free();
                }
            }
            return devices;
        }

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern uint mplaylist_get_camera_device_count();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern uint mplaylist_get_camera_device_name(uint index, IntPtr buffer, uint max_len);

        public static System.Collections.Generic.List<string> GetVideoDevices()
        {
            var devices = new System.Collections.Generic.List<string>();
            uint count = mplaylist_get_camera_device_count();
            for (uint i = 0; i < count; i++)
            {
                byte[] buffer = new byte[256];
                GCHandle handle = GCHandle.Alloc(buffer, GCHandleType.Pinned);
                try
                {
                    uint bytesWritten = mplaylist_get_camera_device_name(i, handle.AddrOfPinnedObject(), 256);
                    devices.Add(bytesWritten > 0 ? System.Text.Encoding.UTF8.GetString(buffer, 0, (int)bytesWritten) : $"Unknown Camera {i}");
                }
                finally { handle.Free(); }
            }
            return devices;
        }

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_fire_cue(FfiCue cue);
    [DllImport("m_playlist.dll", CallingConvention = CallingConvention.Cdecl)] public static extern void mplaylist_update_subtitle(IntPtr utf8_text);
    [DllImport("m_playlist.dll", CallingConvention = CallingConvention.Cdecl)] public static extern float mplaylist_calculate_lufs(IntPtr filepath);

        [DllImport("m_playlist.dll", CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_transcode_file(IntPtr in_path, IntPtr out_path);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern uint mplaylist_get_audio_device_count();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern uint mplaylist_get_audio_device_name(uint index, IntPtr buffer, uint max_len);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_audio_device(uint index);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_scrub_to(long target_hnsecs);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_get_dimensions(out uint width, out uint height);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_sync_offset(double offsetSeconds);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_get_diagnostics(out double audioTime, out double videoTime);

        [DllImport("m_playlist.dll", CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_ndi_enabled(bool enabled);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_get_audio_telemetry(int deck_id, out int out_occupancy, out int out_capacity);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_get_audio_levels(out float left, out float right);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Unicode)]
        public static extern void mplaylist_set_overlay_text(bool show, string text);


        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_resize_swapchain(uint width, uint height);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_geometry(
            float tl_x, float tl_y,
            float tr_x, float tr_y,
            float bl_x, float bl_y,
            float br_x, float br_y
        );


        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_crop(float left, float top, float right, float bottom);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_pan_zoom(float pan_x, float pan_y, float zoom);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_color(float brightness, float contrast, float saturation);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_bind_output_matrix(IntPtr hwnd);
        [DllImport("m_playlist.dll", CallingConvention = CallingConvention.Cdecl)]
        public static extern ulong mplaylist_get_ltc_timecode();
    }
}
