using System;
using System.Collections.Concurrent;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Threading.Tasks;

namespace MPlaylistApp
{
    public class FileStateMonitor : IDisposable
    {
        private readonly ObservableCollection<MediaCue> _playlist;
        private readonly ConcurrentDictionary<string, bool> _lockedFiles = new ConcurrentDictionary<string, bool>();
        private readonly ConcurrentDictionary<string, FileSystemWatcher> _watchers = new ConcurrentDictionary<string, FileSystemWatcher>(StringComparer.OrdinalIgnoreCase);

        public FileStateMonitor(ObservableCollection<MediaCue> playlist)
        {
            _playlist = playlist;
            _playlist.CollectionChanged += Playlist_CollectionChanged;
            
            // Initial scan
            foreach (var cue in _playlist)
            {
                EnsureWatching(cue.FilePath);
            }
        }

        private void Playlist_CollectionChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            if (e.NewItems != null)
            {
                foreach (MediaCue cue in e.NewItems)
                {
                    EnsureWatching(cue.FilePath);
                }
            }
            // Optional: Implement garbage collection of unused watchers on e.OldItems if needed.
        }

        private void EnsureWatching(string filePath)
        {
            if (string.IsNullOrEmpty(filePath)) return;
            string? dir = Path.GetDirectoryName(filePath);
            if (string.IsNullOrEmpty(dir) || !Directory.Exists(dir)) return;

            if (!_watchers.ContainsKey(dir))
            {
                var watcher = new FileSystemWatcher(dir)
                {
                    NotifyFilter = NotifyFilters.LastWrite | NotifyFilters.FileName | NotifyFilters.DirectoryName | NotifyFilters.Size,
                    Filter = "*.*",
                    EnableRaisingEvents = true
                };

                watcher.Changed += OnFileEvent;
                watcher.Created += OnFileEvent;
                watcher.Renamed += OnFileRenamed;

                _watchers.TryAdd(dir, watcher);
            }
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
            HandleFileChangeAsync(e.FullPath);
        }

        private void OnFileRenamed(object sender, RenamedEventArgs e)
        {
            HandleFileChangeAsync(e.FullPath);
        }

        private async void HandleFileChangeAsync(string fullPath)
        {
            if (!_lockedFiles.TryAdd(fullPath, true)) return;

            try
            {
                while (IsFileLocked(fullPath))
                {
                    await Task.Delay(500);
                }

                System.Windows.Application.Current.Dispatcher.Invoke(() =>
                {
                    var matchingCues = _playlist.Where(c => string.Equals(c.FilePath, fullPath, StringComparison.OrdinalIgnoreCase)).ToList();
                    foreach (var cue in matchingCues)
                    {
                        Console.WriteLine($"[FileStateMonitor] Hot-swapping updated file: {cue.FilePath}");
                        EngineInterop.LoadCueToEngine(cue);
                    }
                });
            }
            finally
            {
                _lockedFiles.TryRemove(fullPath, out _);
            }
        }

        public void Dispose()
        {
            foreach (var watcher in _watchers.Values)
            {
                watcher.Dispose();
            }
            _watchers.Clear();
        }
    }
}
