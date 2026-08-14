using System;
using System.Runtime.InteropServices;
using System.Windows;

namespace MPlaylistApp
{
    public class DisplayMatrix : IDisposable
    {
        [DllImport("user32.dll", SetLastError = true)]
        private static extern IntPtr CreateWindowEx(
            uint dwExStyle, string lpClassName, string lpWindowName, uint dwStyle,
            int x, int y, int nWidth, int nHeight, IntPtr hWndParent, IntPtr hMenu, IntPtr hInstance, IntPtr lpParam);

        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool DestroyWindow(IntPtr hWnd);

        public IntPtr Handle { get; private set; }

        public DisplayMatrix(int x, int y, int width, int height)
        {
            // Must spawn on the UI thread to inherit the WPF message pump
            System.Windows.Application.Current.Dispatcher.Invoke(() => 
            {
                // WS_EX_TOPMOST = 0x00000008, WS_POPUP = 0x80000000, WS_VISIBLE = 0x10000000
                Handle = CreateWindowEx(
                    0x00000008, "STATIC", "M-Playlist Clean Feed",
                    0x80000000 | 0x10000000, 
                    x, y, width, height,
                    IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero
                );
                ShowWindow(Handle, 5); 
            });
        }

        public void Dispose()
        {
            if (Handle != IntPtr.Zero)
            {
                System.Windows.Application.Current.Dispatcher.Invoke(() =>
                {
                    DestroyWindow(Handle);
                    Handle = IntPtr.Zero;
                });
            }
        }
    }
}
