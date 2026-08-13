using System;
using System.Runtime.InteropServices;

namespace MPlaylistApp 
{
    public static class MediaMetadataProbe 
    {
        [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IPropertyStore 
        {
            uint GetCount([Out] out uint cProps);
            uint GetAt([In] uint iProp, out PROPERTYKEY pkey);
            uint GetValue([In] ref PROPERTYKEY key, [Out] out PROPVARIANT pv);
            uint SetValue([In] ref PROPERTYKEY key, [In] ref PROPVARIANT propvar);
            uint Commit();
        }

        [StructLayout(LayoutKind.Sequential, Pack = 4)]
        private struct PROPERTYKEY 
        {
            public Guid fmtid;
            public uint pid;
        }

        [StructLayout(LayoutKind.Explicit, Size = 24)]
        private struct PROPVARIANT 
        {
            [FieldOffset(0)] public ushort vt;
            [FieldOffset(8)] public ulong uhVal; 
        }

        [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = true)]
        private static extern int SHGetPropertyStoreFromParsingName(
            [In][MarshalAs(UnmanagedType.LPWStr)] string pszPath,
            [In] IntPtr pbc,
            [In] int flags,
            [In] ref Guid riid,
            [Out][MarshalAs(UnmanagedType.Interface)] out IPropertyStore ppv);

        public static ulong GetDurationHNS(string filePath) 
        {
            try 
            {
                Guid IID_IPropertyStore = new Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99");
                int hr = SHGetPropertyStoreFromParsingName(filePath, IntPtr.Zero, 0, ref IID_IPropertyStore, out IPropertyStore store);
                
                if (hr >= 0 && store != null) 
                {
                    // Property Key for System.Media.Duration
                    PROPERTYKEY durationKey = new PROPERTYKEY { 
                        fmtid = new Guid("64440490-4C8B-11D1-8B70-080036B11A03"), 
                        pid = 3 
                    };
                    
                    store.GetValue(ref durationKey, out PROPVARIANT variant);
                    if (variant.vt == 21) // VT_UI8 (ulong)
                    {
                        ulong duration = variant.uhVal;
                        Marshal.ReleaseComObject(store);
                        return duration; // Already natively in 100-nanosecond (HNS) units!
                    }
                    Marshal.ReleaseComObject(store);
                }
            } 
            catch { /* Silently fail and fallback to 0 */ }
            return 0;
        }
    }
}
