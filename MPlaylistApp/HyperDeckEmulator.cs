using System;
using System.IO;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace MPlaylistApp
{
    public class HyperDeckEmulator
    {
        private TcpListener _listener;
        private CancellationTokenSource _cts;
        private EngineConductor _conductor;

        public HyperDeckEmulator(EngineConductor conductor)
        {
            _conductor = conductor;
            _listener = new TcpListener(IPAddress.Any, 9993);
            _cts = new CancellationTokenSource();
        }

        public void Start()
        {
            _listener.Start();
            Task.Run(() => AcceptLoop(_cts.Token));
        }

        public void Stop()
        {
            _cts.Cancel();
            _listener.Stop();
        }

        private async Task AcceptLoop(CancellationToken token)
        {
            try
            {
                while (!token.IsCancellationRequested)
                {
                    TcpClient client = await _listener.AcceptTcpClientAsync(token);
                    _ = HandleClientAsync(client, token);
                }
            }
            catch (OperationCanceledException) { }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine($"HyperDeck Listener Error: {ex.Message}");
            }
        }

        private async Task HandleClientAsync(TcpClient client, CancellationToken token)
        {
            using (client)
            using (NetworkStream stream = client.GetStream())
            using (StreamReader reader = new StreamReader(stream, Encoding.ASCII))
            using (StreamWriter writer = new StreamWriter(stream, Encoding.ASCII) { AutoFlush = true })
            {
                try
                {
                    await writer.WriteAsync("500 connection info:\r\nprotocol version: 1.6\r\nmodel: M-Playlist HyperDeck\r\n\r\n");

                    while (!token.IsCancellationRequested && client.Connected)
                    {
                        string? line = await reader.ReadLineAsync();
                        if (line == null) break;

                        line = line.Trim().ToLower();

                        if (line.StartsWith("ping"))
                        {
                            await writer.WriteAsync("200 ok\r\n\r\n");
                        }
                        else if (line.StartsWith("play"))
                        {
                            System.Windows.Application.Current.Dispatcher.Invoke(() =>
                            {
                                _conductor.TransportFireNext();
                            });
                            await writer.WriteAsync("200 ok\r\n\r\n");
                        }
                        else if (line.StartsWith("stop"))
                        {
                            System.Windows.Application.Current.Dispatcher.Invoke(() =>
                            {
                                _conductor.TransportStop();
                            });
                            await writer.WriteAsync("200 ok\r\n\r\n");
                        }
                        // Ignore other commands
                    }
                }
                catch (Exception ex)
                {
                    System.Diagnostics.Debug.WriteLine($"HyperDeck Client Error: {ex.Message}");
                }
            }
        }
    }
}
