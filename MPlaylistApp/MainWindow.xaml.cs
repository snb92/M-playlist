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
        private EngineConductor _conductor;
        private bool _isUserScrubbing = false;
        private ObservableCollection<MediaCue> _playlist = new ObservableCollection<MediaCue>();
        private VideoHwndHost? _videoSurface;
        private FileStateMonitor? _fileMonitor;
        private MediaCue? _activePlayingCue;
        private bool _isPaused = false;

        private float _smoothedVuL = 0;
        private float _smoothedVuR = 0;

        public MainWindow()
        {
            InitializeComponent();
            PlaylistUI.ItemsSource = _playlist;
            _fileMonitor = new FileStateMonitor(_playlist);
            
            _conductor = new EngineConductor();
            _conductor.OnCueActivated += (cue) => 
            {
                Dispatcher.Invoke(() => 
                {
                    _activePlayingCue = cue;
                    int idx = _playlist.IndexOf(cue);
                    if (idx >= 0) {
                        PlaylistUI.SelectedIndex = idx;
                        StatusText.Text = $"Playing Cue #{idx + 1}";
                    }
                });
            };
            
            _playlist.CollectionChanged += (s, e) => {
                _conductor.UpdatePlaylistTopology(_playlist);
            };

            this.Loaded += MainWindow_Loaded;
            this.Closed += MainWindow_Closed;
        }



        private void MainWindow_Loaded(object sender, RoutedEventArgs e)
        {
            _videoSurface = new VideoHwndHost();
            _videoSurface.MessageHook += VideoHost_MessageHook;
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
                    
                    _conductor.UpdatePlaylistTopology(_playlist);
                    _conductor.Start();
                    
                    // Start UI polling for timecode (loose 33ms interval purely for UI updates)
                    _uiTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(33) };
                    _uiTimer.Tick += (s, args) =>
                    {
                        double audioTime = _conductor.CurrentAudioTime;
                        double videoTime = _conductor.CurrentVideoTime;
                        
                        EngineInterop.mplaylist_get_audio_telemetry(0, out int occupancy, out int capacity);
                        double latencyMs = occupancy / 768.0; 
                        
                        if (_activePlayingCue != null && _activePlayingCue.DurationHNS > 0)
                        {
                            if (TimelineSlider.Maximum != _activePlayingCue.DurationHNS)
                            {
                                TimelineSlider.Maximum = _activePlayingCue.DurationHNS;
                            }
                            
                            TimecodeText.Text = $"A: {TimeSpan.FromSeconds(audioTime).ToString(@"hh\:mm\:ss\:ff")} | V: {TimeSpan.FromSeconds(videoTime).ToString(@"hh\:mm\:ss\:ff")}";
                            
                            if (!_isUserScrubbing)
                                TimelineSlider.Value = _conductor.CurrentVideoTime * 10000000.0;

                            if (OverlayToggle.IsChecked == true && _activePlayingCue != null && _conductor != null)
                            {
                                long currentHNS = (long)(_conductor.CurrentVideoTime * 10000000.0);
                                long remainingHNS = _activePlayingCue.DurationHNS > (ulong)currentHNS 
                                    ? (long)_activePlayingCue.DurationHNS - currentHNS 
                                    : 0;
                                
                                TimeSpan elapsed = TimeSpan.FromTicks(currentHNS);
                                TimeSpan remain = TimeSpan.FromTicks(remainingHNS);
                                
                                string overlayStr = $"CUE: {_activePlayingCue.Title}\nELAPSED: {elapsed:hh\\:mm\\:ss\\.ff} | REMAINING: -{remain:hh\\:mm\\:ss\\.ff}";
                                try { EngineInterop.mplaylist_set_overlay_text(true, overlayStr); } catch { }
                            }
                            
                            // Acoustic VU Meter Polling
                            try
                            {
                                EngineInterop.mplaylist_get_audio_levels(out float rawL, out float rawR);
                                
                                float dbL = rawL > 0.0001f ? (float)(20.0 * Math.Log10(rawL)) : -60f;
                                float dbR = rawR > 0.0001f ? (float)(20.0 * Math.Log10(rawR)) : -60f;
                                
                                float targetL = Math.Clamp((dbL + 60f) / 60f, 0f, 1f);
                                float targetR = Math.Clamp((dbR + 60f) / 60f, 0f, 1f);

                                _smoothedVuL = targetL > _smoothedVuL ? targetL : Math.Max(targetL, _smoothedVuL - 0.05f);
                                _smoothedVuR = targetR > _smoothedVuR ? targetR : Math.Max(targetR, _smoothedVuR - 0.05f);
                                
                                // Applying to ScaleX for the horizontal UI bars
                                MeterLeftScale.ScaleX = float.IsNaN(_smoothedVuL) ? 0 : _smoothedVuL;
                                MeterRightScale.ScaleX = float.IsNaN(_smoothedVuR) ? 0 : _smoothedVuR;
                                
                                // Update Raw Numeric Probes
                                if (VuTextL != null && VuTextR != null)
                                {
                                    VuTextL.Text = dbL <= -59.0f ? "L: -INF" : $"L: {dbL:0.1} dB";
                                    VuTextR.Text = dbR <= -59.0f ? "R: -INF" : $"R: {dbR:0.1} dB";
                                    
                                    VuTextL.Foreground = dbL > -0.1f ? System.Windows.Media.Brushes.Red : System.Windows.Media.Brushes.LimeGreen;
                                    VuTextR.Foreground = dbR > -0.1f ? System.Windows.Media.Brushes.Red : System.Windows.Media.Brushes.LimeGreen;
                                }
                            }
                            catch (Exception ex) 
                            { 
                                if (StatusText != null) StatusText.Text = $"AUDIO FFI TRAP: {ex.Message}"; 
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
                Filter = "Media Files|*.mp4;*.mov;*.mkv;*.wmv;*.wav;*.png;*.jpg;*.jpeg",
                Multiselect = true
            };

            if (openFileDialog.ShowDialog() == true)
            {
                foreach (string file in openFileDialog.FileNames)
                {
                    ulong detectedDuration = MediaMetadataProbe.GetDurationHNS(file);
                    var cue = new MediaCue
                    {
                        FilePath = file,
                        Title = Path.GetFileNameWithoutExtension(file),
                        DurationHNS = detectedDuration,
                        OutPointHNS = detectedDuration // Default OutPoint to the physical end of the file
                    };
                    _playlist.Add(cue);
                    EngineInterop.LoadCueToEngine(cue);
                }
                UpdatePlaylistTotalDuration();
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
                        ulong detectedDuration = MediaMetadataProbe.GetDurationHNS(file);
                        var cue = new MediaCue
                        {
                            FilePath = file,
                            Title = Path.GetFileNameWithoutExtension(file),
                            DurationHNS = detectedDuration,
                            OutPointHNS = detectedDuration // Default OutPoint to the physical end of the file
                        };
                        _playlist.Add(cue);
                        EngineInterop.LoadCueToEngine(cue);
                    }
                }
                UpdatePlaylistTotalDuration();
            }
        }

        private void OnTestCleanFeedClicked(object sender, RoutedEventArgs e)
        {
            // Use the vestigial WinForms reference you already have to discover screens
            var screens = System.Windows.Forms.Screen.AllScreens;
            if (screens.Length > 1) {
                // Target the secondary physical monitor (Index 1)
                var target = screens[1].Bounds;
                IntPtr targetHwnd = DisplayMatrix.SpawnBorderlessWindow(target.X, target.Y, target.Width, target.Height);
                EngineInterop.mplaylist_bind_output_matrix(targetHwnd);
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
                uint transMs = nextCue.TransitionMs;
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
                
                // Inform the Conductor of the manual override
                _conductor.SetActiveCue(nextCue);
                
                // POST-Advance the UI selection
                if (_playlist.Count > 0)
                {
                    PlaylistUI.SelectedIndex = (targetIndex + 1) % _playlist.Count;
                }
                StatusText.Text = $"Playing Cue #{targetIndex + 1}";
                
                // Assign active cue and remove debounce
                _activePlayingCue = nextCue;
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
            _conductor.SetActiveCue(null);
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
            _conductor.Stop();
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

        private void TimelineSlider_PreviewMouseLeftButtonDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            _isUserScrubbing = true;
        }

        private void TimelineSlider_PreviewMouseLeftButtonUp(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            if (_conductor != null)
            {
                _conductor.RequestScrub((long)TimelineSlider.Value);
            }
            _isUserScrubbing = false;
        }

        private void Timeline_DragStarted(object sender, System.Windows.Controls.Primitives.DragStartedEventArgs e)
        {
            _isUserScrubbing = true;
        }

        private void Timeline_DragCompleted(object sender, System.Windows.Controls.Primitives.DragCompletedEventArgs e)
        {
            if (_conductor != null)
            {
                _conductor.RequestScrub((long)TimelineSlider.Value);
            }
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
            if ((sender as System.Windows.Controls.Button)?.DataContext is MediaCue targetCue && _conductor != null)
            {
                targetCue.InPointHNS = (ulong)(_conductor.CurrentVideoTime * 10000000.0);
            }
        }

        private void OnSetOutPointClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as System.Windows.Controls.Button)?.DataContext is MediaCue targetCue && _conductor != null)
            {
                targetCue.OutPointHNS = (ulong)(_conductor.CurrentVideoTime * 10000000.0);
            }
        }

        private void OnVolumeSliderChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
        {
            EngineInterop.mplaylist_set_volume_db((float)e.NewValue);
        }

        private IntPtr VideoHost_MessageHook(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            const int WM_LBUTTONDBLCLK = 0x0203;
            if (msg == WM_LBUTTONDBLCLK)
            {
                // Open the trim context menu at the mouse position
                if (FindResource("VideoTrimMenu") is System.Windows.Controls.ContextMenu menu)
                {
                    menu.IsOpen = true;
                }
                handled = true;
            }
            return IntPtr.Zero;
        }

        private void ContextMenuSetIn_Click(object sender, RoutedEventArgs e)
        {
            if (_conductor != null && _activePlayingCue != null)
            {
                _activePlayingCue.InPointHNS = (ulong)(_conductor.CurrentVideoTime * 10000000.0);
                PlaylistUI.SelectedItem = _activePlayingCue; // Force UI Selection Sync
            }
        }

        private void ContextMenuSetOut_Click(object sender, RoutedEventArgs e)
        {
            if (_conductor != null && _activePlayingCue != null)
            {
                _activePlayingCue.OutPointHNS = (ulong)(_conductor.CurrentVideoTime * 10000000.0);
                PlaylistUI.SelectedItem = _activePlayingCue; // Force UI Selection Sync
            }
        }

        private void UpdatePlaylistTotalDuration()
        {
            ulong totalHns = 0;
            foreach (var cue in _playlist) totalHns += cue.DurationHNS;
            if (TotalDurationText != null) 
            {
                TimeSpan ts = TimeSpan.FromTicks((long)totalHns);
                TotalDurationText.Text = $"TOTAL: {ts:hh\\:mm\\:ss}";
            }
        }

        private void OverlayToggle_Changed(object sender, RoutedEventArgs e)
        {
            if (OverlayToggle?.IsChecked == false)
            {
                try { EngineInterop.mplaylist_set_overlay_text(false, null); } catch { }
            }
        }
    }

    public class StringToBrushConverter : System.Windows.Data.IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, System.Globalization.CultureInfo culture)
        {
            if (value is string colorStr && !string.IsNullOrEmpty(colorStr))
            {
                try { return new System.Windows.Media.BrushConverter().ConvertFromString(colorStr); } catch { }
            }
            return System.Windows.Media.Brushes.Transparent;
        }
        public object ConvertBack(object value, Type targetType, object parameter, System.Globalization.CultureInfo culture) => null;
    }
}
