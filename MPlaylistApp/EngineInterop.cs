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
        public long TransitionDurationHnsecs;
    }

    public static class EngineInterop
    {
        private const string DllName = "m_playlist.dll";

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_init();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_shutdown();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_set_window(IntPtr hwnd);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern bool mplaylist_load_cue(FfiCue cue);

        public static void LoadCueToEngine(CueModel model)
        {
            IntPtr ptr = Marshal.StringToCoTaskMemUTF8(model.FilePath);
            try
            {
                var ffiCue = new FfiCue
                {
                    FilePath = ptr,
                    InPointHnsecs = model.InPointHnsecs,
                    OutPointHnsecs = model.OutPointHnsecs,
                    IsLooping = (byte)(model.IsLooping ? 1 : 0),
                    HoldLastFrame = (byte)(model.HoldLastFrame ? 1 : 0),
                    TransitionDurationHnsecs = model.TransitionDurationHnsecs
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
        public static extern bool mplaylist_fire_next();

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

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_ndi_output(byte enabled);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_resize_swapchain(uint width, uint height);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void mplaylist_set_geometry(
            float tl_x, float tl_y,
            float tr_x, float tr_y,
            float bl_x, float bl_y,
            float br_x, float br_y
        );
    }
}
