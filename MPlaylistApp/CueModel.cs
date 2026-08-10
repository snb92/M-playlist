using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Windows;
using System.Text.Json.Serialization;

namespace MPlaylistApp
{
    public class CueModel : INotifyPropertyChanged, IDisposable
    {
        private string _filePath = string.Empty;
        private string _title = string.Empty;
        private long _inPointHnsecs;
        private long _outPointHnsecs;
        private bool _isLooping;
        private bool _holdLastFrame;
        private long _transitionDurationHnsecs;
        private FileSystemWatcher? _watcher;

        public string FilePath 
        { 
            get => _filePath; 
            set 
            { 
                if (_filePath != value)
                {
                    _filePath = value; 
                    OnPropertyChanged(); 
                    SetupWatcher();
                }
            } 
        }
        
        public string Title { get => _title; set { _title = value; OnPropertyChanged(); } }
        public long InPointHnsecs { get => _inPointHnsecs; set { _inPointHnsecs = value; OnPropertyChanged(); } }
        public long OutPointHnsecs { get => _outPointHnsecs; set { _outPointHnsecs = value; OnPropertyChanged(); } }

        private double _inPoint;
        public double InPoint { get => _inPoint; set { _inPoint = value; OnPropertyChanged(); } }
        
        private double _outPoint;
        public double OutPoint { get => _outPoint; set { _outPoint = value; OnPropertyChanged(); } }
        public bool IsLooping { get => _isLooping; set { _isLooping = value; OnPropertyChanged(); } }
        public bool HoldLastFrame { get => _holdLastFrame; set { _holdLastFrame = value; OnPropertyChanged(); } }
        public long TransitionDurationHnsecs { get => _transitionDurationHnsecs; set { _transitionDurationHnsecs = value; OnPropertyChanged(); } }
        
        private double _transitionDuration;
        public double TransitionDuration { get => _transitionDuration; set { _transitionDuration = value; OnPropertyChanged(); } }

        private void SetupWatcher()
        {
            _watcher?.Dispose();
            _watcher = null;

            if (string.IsNullOrEmpty(_filePath)) return;

            string? dir = Path.GetDirectoryName(_filePath);
            string file = Path.GetFileName(_filePath);

            if (string.IsNullOrEmpty(dir) || !Directory.Exists(dir)) return;

            _watcher = new FileSystemWatcher(dir, file)
            {
                NotifyFilter = NotifyFilters.LastWrite | NotifyFilters.FileName | NotifyFilters.DirectoryName | NotifyFilters.Size
            };

            _watcher.Changed += OnFileEvent;
            _watcher.Created += OnFileEvent;
            _watcher.Renamed += OnFileRenamed;
            _watcher.Deleted += OnFileEvent;
            _watcher.EnableRaisingEvents = true;
        }

        private bool IsFileLocked(string filePath)
        {
            if (!System.IO.File.Exists(filePath)) return false;
            try
            {
                using (System.IO.FileStream stream = System.IO.File.Open(filePath, System.IO.FileMode.Open, System.IO.FileAccess.Read, System.IO.FileShare.None))
                {
                    stream.Close();
                }
            }
            catch (System.IO.IOException)
            {
                return true;
            }
            return false;
        }

        private void OnFileEvent(object sender, FileSystemEventArgs e)
        {
            System.Threading.Tasks.Task.Run(async () =>
            {
                // Wait for the rendering application to release the file OS lock
                while (IsFileLocked(this.FilePath))
                {
                    await System.Threading.Tasks.Task.Delay(500);
                }

                // Now that the file is safe to read, marshal back to the UI thread to command Rust
                System.Windows.Application.Current.Dispatcher.Invoke(() =>
                {
                    EngineInterop.LoadCueToEngine(this);
                });
            });
        }

        private void OnFileRenamed(object sender, RenamedEventArgs e)
        {
            System.Windows.Application.Current.Dispatcher.Invoke(() =>
            {
                // Temporarily disable watcher setup while updating the path
                _watcher!.EnableRaisingEvents = false;
                _filePath = e.FullPath;
                OnPropertyChanged(nameof(FilePath));
                
                // Re-initialize the watcher for the new file name
                SetupWatcher();
            });

            System.Threading.Tasks.Task.Run(async () =>
            {
                while (IsFileLocked(this.FilePath))
                {
                    await System.Threading.Tasks.Task.Delay(500);
                }

                System.Windows.Application.Current.Dispatcher.Invoke(() =>
                {
                    EngineInterop.LoadCueToEngine(this);
                });
            });
        }

        public void Dispose()
        {
            _watcher?.Dispose();
            _watcher = null;
        }

        public event PropertyChangedEventHandler? PropertyChanged;
        protected void OnPropertyChanged([CallerMemberName] string? name = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        }
    }
}
