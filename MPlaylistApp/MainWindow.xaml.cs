using System;
using System.Windows;
using Microsoft.Win32;
using System.Windows.Threading;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using Screen = System.Windows.Forms.Screen;

namespace MPlaylistApp
{
    public partial class MainWindow : Window
    {
        private DispatcherTimer? _uiTimer;
        private bool _isUserScrubbing = false;
        private ObservableCollection<MediaCue> _playlist = new ObservableCollection<MediaCue>();
        private VideoHwndHost? _videoSurface;
        private FileStateMonitor? _fileMonitor;
        private MediaCue? _activePlayingCue;
        private DateTime _lastTransitionTime = DateTime.MinValue;
        private bool _isPaused = false;

        public MainWindow()
        {
            InitializeComponent();
            PlaylistUI.ItemsSource = _playlist;
            _fileMonitor = new FileStateMonitor(_playlist);
            this.Loaded += MainWindow_Loaded;
            this.Closed += MainWindow_Closed;
        }



        private void MainWindow_Loaded(object sender, RoutedEventArgs e)
        {
            _videoSurface = new VideoHwndHost();
            VideoContainer.Child = _videoSurface;
            VideoContainer.SizeChanged += (s, args) => {
                TriggerVideoResize();
            };

            ComboEndBehavior.ItemsSource = Enum.GetValues(typeof(EndBehavior));

            var audioDevices = EngineInterop.GetAudioDevices();
            AudioDeviceCombo.ItemsSource = audioDevices;
            if (audioDevices.Count > 0)
                AudioDeviceCombo.SelectedIndex = 0;

            // 1. Boot the Rust Master Clock
            if (EngineInterop.mplaylist_init())
            {
                // 2. Pass the RAW child window handle to DXGI
                IntPtr videoHwnd = _videoSurface.Handle;
                if (EngineInterop.mplaylist_set_window(videoHwnd))
                {
                    StatusText.Text = "System Armed. DXGI Bound.";
                    StatusText.Foreground = System.Windows.Media.Brushes.LightGreen;
                    
                    // Start UI polling for timecode
                    _uiTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(50) };
                    _uiTimer.Tick += (s, args) =>
                    {
                        // Acoustic Telemetry Probe
                        if (EngineInterop.mplaylist_get_diagnostics(out double audioTime, out double videoTime))
                        {
                            EngineInterop.mplaylist_get_audio_telemetry(0, out int occupancy, out int capacity);
                            double latencyMs = occupancy / 768.0; // 16 channels * 48000 Hz / 1000 = 768 floats per ms
                            
                            TimecodeText.Text = $"A: {audioTime:F3}s | V: {videoTime:F3}s | Latency: {latencyMs:F2}ms";
                            Console.WriteLine($"AUDIO BUFFER - Capacity: {capacity} | Occupancy: {occupancy} | Latency: {latencyMs:F2} ms");
                            
                            if (!_isUserScrubbing)
                            {
                                TimelineSlider.Value = videoTime * 10000000.0; 
                            }
                            
                            // Step 2 & 3: Playhead Monitor Loop and EndBehavior Switch
                            if (_activePlayingCue != null)
                            {
                                ulong currentPlayheadHNS = (ulong)(videoTime * 10000000.0);
                                if (_activePlayingCue.OutPointHNS > 0 && currentPlayheadHNS >= _activePlayingCue.OutPointHNS)
                                {
                                    // Step 4: The Debounce Lock
                                    if ((DateTime.Now - _lastTransitionTime).TotalMilliseconds > 300)
                                    {
                                        _lastTransitionTime = DateTime.Now;
                                        
                                        switch (_activePlayingCue.EndBehavior)
                                        {
                                            case EndBehavior.Stop:
                                                EngineInterop.mplaylist_stop();
                                                _activePlayingCue.IsActivePlaying = false;
                                                _activePlayingCue = null;
                                                break;
                                            case EndBehavior.LoopForever:
                                                EngineInterop.mplaylist_scrub_to((long)_activePlayingCue.InPointHNS);
                                                break;
                                            default:
                                                // Default / Next Cue logic
                                                OnPlayClicked(this, new RoutedEventArgs());
                                                break;
                                        }
                                    }
                                }
                            }
                        }
                    };
                    _uiTimer.Start();
                }
                else
                {
                    StatusText.Text = "FATAL: DXGI Compositor Failed.";
                }
                
                // FORCE an initial resize now that DXGI is bound!
                TriggerVideoResize();
            }
        }

        private void TriggerVideoResize()
        {
            if (_videoSurface != null && VideoContainer.ActualWidth > 0 && VideoContainer.ActualHeight > 0) {
                var presentationSource = PresentationSource.FromVisual(this);
                double dpiX = 1.0;
                double dpiY = 1.0;
                
                if (presentationSource != null && presentationSource.CompositionTarget != null) {
                    dpiX = presentationSource.CompositionTarget.TransformToDevice.M11;
                    dpiY = presentationSource.CompositionTarget.TransformToDevice.M22;
                }
                
                int physicalWidth = (int)(VideoContainer.ActualWidth * dpiX);
                int physicalHeight = (int)(VideoContainer.ActualHeight * dpiY);
                
                _videoSurface.ResizeHwnd(physicalWidth, physicalHeight);
                EngineInterop.mplaylist_resize_swapchain((uint)physicalWidth, (uint)physicalHeight);
            }
        }

        private void OnSyncSliderChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
        {
            if (SyncLabel != null)
            {
                // Slider value is in milliseconds (e.g., 80ms)
                double offsetMilliseconds = e.NewValue;
                SyncLabel.Text = $"{offsetMilliseconds} ms";
                
                // Rust expects seconds (e.g., 0.080)
                EngineInterop.mplaylist_set_sync_offset(offsetMilliseconds / 1000.0);
            }
        }

        private void OnAddCueClicked(object sender, RoutedEventArgs e)
        {
            Microsoft.Win32.OpenFileDialog openFileDialog = new Microsoft.Win32.OpenFileDialog
            {
                Filter = "Media Files|*.mp4;*.mov;*.mkv;*.wmv;*.wav",
                Multiselect = true
            };

            if (openFileDialog.ShowDialog() == true)
            {
                foreach (string file in openFileDialog.FileNames)
                {
                    var cue = new MediaCue
                    {
                        FilePath = file,
                        Title = Path.GetFileNameWithoutExtension(file)
                    };
                    _playlist.Add(cue);
                    EngineInterop.LoadCueToEngine(cue);
                }
            }
        }

        private void PlaylistUI_Drop(object sender, System.Windows.DragEventArgs e)
        {
            if (e.Data.GetDataPresent(System.Windows.DataFormats.FileDrop))
            {
                string[] files = (string[])e.Data.GetData(System.Windows.DataFormats.FileDrop);
                foreach (string file in files)
                {
                    string ext = Path.GetExtension(file).ToLower();
                    if (ext == ".mp4" || ext == ".mov" || ext == ".mkv" || ext == ".wmv" || ext == ".wav")
                    {
                        var cue = new MediaCue
                        {
                            FilePath = file,
                            Title = Path.GetFileNameWithoutExtension(file)
                        };
                        _playlist.Add(cue);
                        EngineInterop.LoadCueToEngine(cue);
                    }
                }
            }
        }

        private void OnPlayClicked(object sender, RoutedEventArgs e)
        {
            if (_isPaused && _activePlayingCue != null)
            {
                EngineInterop.mplaylist_resume();
                _isPaused = false;
                StatusText.Text = $"Resumed: {_activePlayingCue.Title}";
                return;
            }

            if (PlaylistUI.Items.Count > 0)
            {
                int targetIndex = PlaylistUI.SelectedIndex;
                if (targetIndex == -1 && _playlist.Count > 0) targetIndex = 0;
                if (targetIndex < 0) return;

                var nextCue = _playlist[targetIndex];
                uint transMs = (uint)(nextCue.TransitionDuration * 1000);
                long inHnsecs = (long)nextCue.InPointHNS;
                long outHnsecs = (long)nextCue.OutPointHNS;

                foreach (var c in _playlist)
                {
                    c.IsActivePlaying = false;
                }
                nextCue.IsActivePlaying = true;

                // Fire the CURRENT target
                EngineInterop.mplaylist_fire_cue((uint)targetIndex, transMs, inHnsecs, outHnsecs);
                EngineInterop.mplaylist_set_volume_db((float)nextCue.VolumeDb);
                
                // POST-Advance the UI selection
                if (_playlist.Count > 0)
                {
                    PlaylistUI.SelectedIndex = (targetIndex + 1) % _playlist.Count;
                }
                StatusText.Text = $"Playing Cue #{targetIndex + 1}";
                
                // Assign active cue and set debounce
                _activePlayingCue = nextCue;
                _lastTransitionTime = DateTime.Now;
                _isPaused = false;
            }
        }

        private void OnPauseClicked(object sender, RoutedEventArgs e)
        {
            if (_activePlayingCue != null && !_isPaused)
            {
                EngineInterop.mplaylist_pause();
                _isPaused = true;
                StatusText.Text = $"Paused: {_activePlayingCue.Title}";
            }
        }

        private void OnStopClicked(object sender, RoutedEventArgs e)
        {
            EngineInterop.mplaylist_stop();
            if (_activePlayingCue != null)
            {
                _activePlayingCue.IsActivePlaying = false;
                _activePlayingCue = null;
            }
            _isPaused = false;
            StatusText.Text = "Stopped";
        }

        protected override void OnClosed(EventArgs e)
        {
            base.OnClosed(e);
            System.Environment.Exit(0);
        }

        private void MainWindow_Closed(object? sender, EventArgs e)
        {
            // 3.5 Dispose FileSystemWatchers
            _fileMonitor?.Dispose();
            _playlist.Clear();

            // 4. Safely kill the WASAPI and MF threads
            EngineInterop.mplaylist_shutdown();
        }

        private void AudioDeviceCombo_SelectionChanged(object sender, System.Windows.Controls.SelectionChangedEventArgs e)
        {
            if (AudioDeviceCombo.SelectedIndex >= 0)
            {
                EngineInterop.mplaylist_set_audio_device((uint)AudioDeviceCombo.SelectedIndex);
            }
        }

        private void Timeline_DragStarted(object sender, System.Windows.Controls.Primitives.DragStartedEventArgs e)
        {
            _isUserScrubbing = true;
        }

        private void Timeline_DragCompleted(object sender, System.Windows.Controls.Primitives.DragCompletedEventArgs e)
        {
            EngineInterop.mplaylist_scrub_to((long)TimelineSlider.Value);
            _isUserScrubbing = false;
        }

        private void NdiBroadcastCheckBox_Checked(object sender, RoutedEventArgs e)
        {
            EngineInterop.mplaylist_set_ndi_enabled(true);
        }

        private void NdiBroadcastCheckBox_Unchecked(object sender, RoutedEventArgs e)
        {
            EngineInterop.mplaylist_set_ndi_enabled(false);
        }

        private void OnCornerSliderChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
        {
            // WPF fires ValueChanged during InitializeComponent before all controls exist
            if (!IsLoaded) return;

            // Update labels
            LblTLX.Text = $"{SliderTLX.Value:F2}";
            LblTLY.Text = $"{SliderTLY.Value:F2}";
            LblTRX.Text = $"{SliderTRX.Value:F2}";
            LblTRY.Text = $"{SliderTRY.Value:F2}";
            LblBLX.Text = $"{SliderBLX.Value:F2}";
            LblBLY.Text = $"{SliderBLY.Value:F2}";
            LblBRX.Text = $"{SliderBRX.Value:F2}";
            LblBRY.Text = $"{SliderBRY.Value:F2}";

            // Push all 8 values to the Rust engine
            EngineInterop.mplaylist_set_geometry(
                (float)SliderTLX.Value, (float)SliderTLY.Value,
                (float)SliderTRX.Value, (float)SliderTRY.Value,
                (float)SliderBLX.Value, (float)SliderBLY.Value,
                (float)SliderBRX.Value, (float)SliderBRY.Value
            );
        }

        private void OnSetInPointClicked(object sender, RoutedEventArgs e)
        {
            if (PlaylistUI.SelectedItem is MediaCue selectedCue)
            {
                if (EngineInterop.mplaylist_get_diagnostics(out double audioTime, out double videoTime))
                {
                    selectedCue.InPointHNS = (ulong)(videoTime * 10000000.0);
                }
            }
        }

        private void OnSetOutPointClicked(object sender, RoutedEventArgs e)
        {
            if (PlaylistUI.SelectedItem is MediaCue selectedCue)
            {
                if (EngineInterop.mplaylist_get_diagnostics(out double audioTime, out double videoTime))
                {
                    selectedCue.OutPointHNS = (ulong)(videoTime * 10000000.0);
                }
            }
        }

        private void OnVolumeSliderChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
        {
            if (PlaylistUI.SelectedItem is MediaCue selectedCue && selectedCue == _activePlayingCue)
            {
                EngineInterop.mplaylist_set_volume_db((float)e.NewValue);
            }
        }
    }
}
