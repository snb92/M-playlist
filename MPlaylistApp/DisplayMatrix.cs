using System;
using System.Runtime.InteropServices;
using System.Windows;

namespace MPlaylistApp
{
    public class DisplayMatrix
    {
        [DllImport("user32.dll", SetLastError = true)]
        private static extern IntPtr CreateWindowEx(
            uint dwExStyle, string lpClassName, string lpWindowName, uint dwStyle,
            int x, int y, int nWidth, int nHeight, IntPtr hWndParent, IntPtr hMenu, IntPtr hInstance, IntPtr lpParam);

        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        public static IntPtr SpawnBorderlessWindow(int x, int y, int width, int height)
        {
            IntPtr hwnd = IntPtr.Zero;
            
            // Must spawn on the UI thread to inherit the WPF message pump
            System.Windows.Application.Current.Dispatcher.Invoke(() => 
            {
                // WS_EX_TOPMOST = 0x00000008, WS_POPUP = 0x80000000, WS_VISIBLE = 0x10000000
                hwnd = CreateWindowEx(
                    0x00000008, "STATIC", "M-Playlist Clean Feed",
                    0x80000000 | 0x10000000, 
                    x, y, width, height,
                    IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero
                );
                ShowWindow(hwnd, 5); 
            });
            
            return hwnd;
        }
    }
}
