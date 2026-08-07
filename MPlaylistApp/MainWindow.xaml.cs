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
        private DispatcherTimer _uiTimer;
        private bool _isUserScrubbing = false;
        private ObservableCollection<CueModel> _playlist = new ObservableCollection<CueModel>();
        private VideoHwndHost _videoSurface;

        public MainWindow()
        {
            InitializeComponent();
            PlaylistUI.ItemsSource = _playlist;
            this.Loaded += MainWindow_Loaded;
            this.Closed += MainWindow_Closed;
        }

        private void MainWindow_Loaded(object sender, RoutedEventArgs e)
        {
            _videoSurface = new VideoHwndHost();
            VideoContainer.Child = _videoSurface;
            VideoContainer.SizeChanged += (s, args) => {
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
            };

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
                        // Update the A/V Diagnostic Timecode
                        if (EngineInterop.mplaylist_get_diagnostics(out double audioTime, out double videoTime))
                        {
                            TimecodeText.Text = $"A: {audioTime:F3}s | V: {videoTime:F3}s";
                            
                            if (!_isUserScrubbing)
                            {
                                TimelineSlider.Value = videoTime * 10000000.0; 
                            }
                        }
                    };
                    _uiTimer.Start();
                }
                else
                {
                    StatusText.Text = "FATAL: DXGI Compositor Failed.";
                }
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
                    var cue = new CueModel
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
                        var cue = new CueModel
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

        private void OnFireNextClicked(object sender, RoutedEventArgs e)
        {
            if (PlaylistUI.Items.Count > 0)
            {
                EngineInterop.mplaylist_fire_next();
                
                // Visually select the active cue
                int nextIndex = PlaylistUI.SelectedIndex + 1;
                if (nextIndex >= PlaylistUI.Items.Count) nextIndex = 0;
                PlaylistUI.SelectedIndex = nextIndex;
                
                StatusText.Text = $"Playing Cue #{nextIndex + 1}";
            }
        }

        protected override void OnClosed(EventArgs e)
        {
            base.OnClosed(e);
            System.Environment.Exit(0);
        }

        private void MainWindow_Closed(object sender, EventArgs e)
        {
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
            EngineInterop.mplaylist_set_ndi_output(1);
        }

        private void NdiBroadcastCheckBox_Unchecked(object sender, RoutedEventArgs e)
        {
            EngineInterop.mplaylist_set_ndi_output(0);
        }
    }
}
