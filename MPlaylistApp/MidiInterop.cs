using System;
using System.Runtime.InteropServices;

namespace MPlaylistApp
{
    public delegate void MidiInProc(IntPtr hMidiIn, uint wMsg, IntPtr dwInstance, uint dwParam1, uint dwParam2);

    public static class MidiInterop
    {
        public const uint CALLBACK_FUNCTION = 0x00030000;
        public const uint MIM_DATA = 0x3C3;

        [DllImport("winmm.dll")]
        public static extern uint midiInGetNumDevs();

        [DllImport("winmm.dll")]
        public static extern uint midiInOpen(out IntPtr hMidiIn, uint uDeviceID, MidiInProc callback, IntPtr dwInstance, uint dwFlags);

        [DllImport("winmm.dll")]
        public static extern uint midiInStart(IntPtr hMidiIn);

        [DllImport("winmm.dll")]
        public static extern uint midiInStop(IntPtr hMidiIn);

        [DllImport("winmm.dll")]
        public static extern uint midiInClose(IntPtr hMidiIn);
    }
}
