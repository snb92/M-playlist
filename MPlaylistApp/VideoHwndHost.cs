using System;
using System.Runtime.InteropServices;
using System.Windows.Interop;

namespace MPlaylistApp
{
    public class VideoHwndHost : HwndHost
    {
        protected override HandleRef BuildWindowCore(HandleRef hwndParent)
        {
            const int WS_CHILD = 0x40000000;
            const int WS_VISIBLE = 0x10000000;

            int w = (int)Math.Max(this.ActualWidth, 100);
            int h = (int)Math.Max(this.ActualHeight, 100);

            // Create a raw, uncomposited child window
            IntPtr hwnd = CreateWindowEx(
                0, "Static", "",
                WS_CHILD | WS_VISIBLE, 
                0, 0, w, h,
                hwndParent.Handle, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
                
            return new HandleRef(this, hwnd);
        }

        protected override IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            if (msg == 0x0014) // WM_ERASEBKGND
            {
                handled = true;
                return new IntPtr(1);
            }
            return base.WndProc(hwnd, msg, wParam, lParam, ref handled);
        }

        protected override void DestroyWindowCore(HandleRef hwnd)
        {
            DestroyWindow(hwnd.Handle);
        }

        [DllImport("user32.dll", EntryPoint = "CreateWindowEx", CharSet = CharSet.Unicode)]
        private static extern IntPtr CreateWindowEx(
            int dwExStyle, string lpszClassName, string lpszWindowName, 
            int style, int x, int y, int width, int height, 
            IntPtr hwndParent, IntPtr hMenu, IntPtr hInst, IntPtr pvParam);
            
        [DllImport("user32.dll", EntryPoint = "DestroyWindow", CharSet = CharSet.Unicode)]
        private static extern bool DestroyWindow(IntPtr hwnd);
    }
}
