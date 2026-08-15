using System;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Threading;

namespace MPlaylistApp
{
    public class ArtNetReceiver
    {
        private UdpClient _udpClient;
        private CancellationTokenSource _cts;
        private Task _receiveTask;
        private EngineConductor _conductor;
        private Dispatcher _dispatcher;

        private int _lastTriggerValue = 0;

        public ArtNetReceiver(EngineConductor conductor, Dispatcher dispatcher)
        {
            _conductor = conductor;
            _dispatcher = dispatcher;
            _cts = new CancellationTokenSource();
            _udpClient = new UdpClient();
            
            _udpClient.Client.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.ReuseAddress, true);
            _udpClient.Client.Bind(new IPEndPoint(IPAddress.Any, 6454));

            _receiveTask = Task.Run(() => ReceiveLoop(_cts.Token));
        }

        private async Task ReceiveLoop(CancellationToken token)
        {
            try
            {
                while (!token.IsCancellationRequested)
                {
                    var result = await _udpClient.ReceiveAsync(token);
                    byte[] bytes = result.Buffer;

                    if (bytes.Length >= 18)
                    {
                        string header = Encoding.ASCII.GetString(bytes, 0, 8);
                        if (header == "Art-Net\0")
                        {
                            ushort opCode = BitConverter.ToUInt16(bytes, 8);
                            if (opCode == 0x5000)
                            {
                                ushort universe = BitConverter.ToUInt16(bytes, 14);
                                if (universe == 0) // Universe 0
                                {
                                    if (bytes.Length >= 18 + 2)
                                    {
                                        byte ch1 = bytes[18]; // Opacity/Volume
                                        byte ch2 = bytes[19]; // Trigger

                                        // Channel 1 (Opacity/Volume): 0-255 scales mathematically to 0.0-1.0 (Volume)
                                        float volume = (float)(ch1 / 255.0);
                                        // Send to EngineInterop (scaling -60 to +12dB logic if strictly required, but prompt says 0.0-1.0 (Volume). We'll map to dB for mplaylist_set_volume_db)
                                        float db = -60.0f + (volume * 72.0f);
                                        EngineInterop.mplaylist_set_volume_db(db);

                                        // Channel 2 (Trigger): If crosses 128
                                        if (ch2 >= 128 && _lastTriggerValue < 128)
                                        {
                                            _dispatcher.BeginInvoke(new Action(() => 
                                            {
                                                if (_conductor != null) _conductor.TransportFireNext();
                                            }));
                                        }
                                        _lastTriggerValue = ch2;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            catch (OperationCanceledException) { }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine("Art-Net UDP error: " + ex.Message);
            }
        }

        public void Stop()
        {
            _cts.Cancel();
            _udpClient.Close();
        }
    }
}
