using System;
using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace MPlaylistApp
{
    public enum EndBehavior
    {
        Stop,
        LoopForever,
        LoopCount,
        GotoTarget,
        FadeOut
    }

    public class MediaCue : INotifyPropertyChanged
    {
        private string _cueID = Guid.NewGuid().ToString();
        private string _title = string.Empty;
        private string _filePath = string.Empty;
        private string _colorTag = "#444444";
        private string _notes = string.Empty;
        private EndBehavior _endBehavior = EndBehavior.Stop;
        private ulong _inPointHNS;
        private ulong _outPointHNS;
        private ulong _durationHNS;
        private uint _transitionMs = 1000; // Default 1 second crossfade
        private double _volumeDb = 0.0;
        private bool _isActivePlaying = false;
        
        // Phase 7 O(1) Routing Support
        public string TargetCueID { get; set; } = string.Empty;
        public int TargetLoopCount { get; set; } = 0;
        public int CurrentLoopCount { get; set; } = 0;
        
        // FFI interop mapping (previously bound directly from UI in CueModel)
        private long _transitionDurationHnsecs;
        private double _transitionDuration;
        private bool _isLooping;
        private bool _holdLastFrame = true;

        public string CueID { get => _cueID; set { _cueID = value; OnPropertyChanged(); } }
        public string Title { get => _title; set { _title = value; OnPropertyChanged(); } }
        public string FilePath { get => _filePath; set { _filePath = value; OnPropertyChanged(); } }
        public string ColorTag { get => _colorTag; set { _colorTag = value; OnPropertyChanged(); } }
        public string Notes { get => _notes; set { _notes = value; OnPropertyChanged(); } }
        public EndBehavior EndBehavior { get => _endBehavior; set { _endBehavior = value; OnPropertyChanged(); } }
        public ulong InPointHNS { get => _inPointHNS; set { _inPointHNS = value; OnPropertyChanged(); } }
        public ulong OutPointHNS { get => IsStaticImage ? 0 : _outPointHNS; set { _outPointHNS = value; OnPropertyChanged(); } }
        public ulong DurationHNS { get => IsStaticImage ? 0 : _durationHNS; set { _durationHNS = value; OnPropertyChanged(); } }
        public uint TransitionMs { get => _transitionMs; set { _transitionMs = value; OnPropertyChanged(); } }
        public double VolumeDb { get => _volumeDb; set { _volumeDb = value; OnPropertyChanged(); } }
        public bool IsActivePlaying { get => _isActivePlaying; set { _isActivePlaying = value; OnPropertyChanged(); } }

        public bool IsStaticImage 
        {
            get 
            {
                if (string.IsNullOrEmpty(_filePath)) return false;
                string ext = System.IO.Path.GetExtension(_filePath).ToLowerInvariant();
                return ext == ".png" || ext == ".jpg" || ext == ".jpeg";
            }
        }

        // FFI mapping compatibility properties
        public long TransitionDurationHnsecs { get => _transitionDurationHnsecs; set { _transitionDurationHnsecs = value; OnPropertyChanged(); } }
        public double TransitionDuration { get => _transitionDuration; set { _transitionDuration = value; OnPropertyChanged(); } }
        public bool IsLooping { get => _isLooping; set { _isLooping = value; OnPropertyChanged(); } }
        public bool HoldLastFrame { get => _holdLastFrame; set { _holdLastFrame = value; OnPropertyChanged(); } }

        public event PropertyChangedEventHandler? PropertyChanged;

        protected void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }
    }
}
