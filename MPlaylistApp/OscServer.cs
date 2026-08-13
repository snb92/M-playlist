using System;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;

namespace MPlaylistApp
{
    public class OscServer
    {
        private UdpClient _udpClient;
        private CancellationTokenSource _cts;
        private EngineConductor _conductor;

        public OscServer(EngineConductor conductor)
        {
            _conductor = conductor;
        }

        public void Start(int port)
        {
            _udpClient = new UdpClient(port);
            _cts = new CancellationTokenSource();
            Task.Run(() => ListenLoop(_cts.Token));
        }

        public void Stop()
        {
            _cts?.Cancel();
            _udpClient?.Close();
        }

        private async Task ListenLoop(CancellationToken token)
        {
            try
            {
                while (!token.IsCancellationRequested)
                {
                    var result = await _udpClient.ReceiveAsync();
                    ParsePacket(result.Buffer);
                }
            }
            catch (ObjectDisposedException) { /* Socket closed naturally */ }
            catch (Exception ex) { Console.WriteLine($"OSC Socket Error: {ex.Message}"); }
        }

        private void ParsePacket(byte[] data)
        {
            try
            {
                // 1. Read Address Pattern (Null-terminated, 4-byte padded)
                string address;
                int offset = ReadOscString(data, 0, out address);
                if (offset == -1) return;

                // 2. Read Type Tags (Null-terminated, 4-byte padded)
                string tags = "";
                if (offset < data.Length && data[offset] == ',')
                {
                    offset = ReadOscString(data, offset, out tags);
                }

                // 3. Route Command (Marshal to Dispatcher if UI context is needed, otherwise call Conductor directly)
                RouteCommand(address, tags, data, offset);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"OSC Parse Error: {ex.Message}");
            }
        }

        private void RouteCommand(string address, string tags, byte[] data, int offset)
        {
            Console.WriteLine($"M-Playlist [OSC]: Received {address}");
            
            if (address == "/mplaylist/play") _conductor.TransportPlay();
            else if (address == "/mplaylist/pause") _conductor.TransportPause();
            else if (address == "/mplaylist/stop") _conductor.TransportStop();
            else if (address == "/mplaylist/jump")
            {
                // Parse the first argument as an Integer
                if (tags.Length >= 2 && tags[1] == 'i' && offset + 4 <= data.Length)
                {
                    // OSC Integers are Big-Endian
                    if (BitConverter.IsLittleEndian) Array.Reverse(data, offset, 4);
                    int cueIndex = BitConverter.ToInt32(data, offset);
                    
                    // OSC systems usually 1-index their interfaces. Our array is 0-indexed.
                    _conductor.TransportJumpToCue(cueIndex - 1); 
                }
            }
        }

        private int ReadOscString(byte[] data, int startIndex, out string result)
        {
            int nullIndex = Array.IndexOf(data, (byte)0, startIndex);
            if (nullIndex == -1) 
            {
                result = string.Empty;
                return -1;
            }
            
            result = Encoding.ASCII.GetString(data, startIndex, nullIndex - startIndex);
            // OSC Spec: String length is padded to a multiple of 4 bytes
            int len = nullIndex - startIndex;
            int paddedLen = (len + 4) & ~3; 
            return startIndex + paddedLen;
        }
    }
}
