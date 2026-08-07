using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace MPlaylistApp
{
    public class CueModel : INotifyPropertyChanged
    {
        private string _filePath = string.Empty;
        private string _title = string.Empty;
        private long _inPointHnsecs;
        private long _outPointHnsecs;
        private bool _isLooping;
        private bool _holdLastFrame;
        private long _transitionDurationHnsecs;

        public string FilePath { get => _filePath; set { _filePath = value; OnPropertyChanged(); } }
        public string Title { get => _title; set { _title = value; OnPropertyChanged(); } }
        public long InPointHnsecs { get => _inPointHnsecs; set { _inPointHnsecs = value; OnPropertyChanged(); } }
        public long OutPointHnsecs { get => _outPointHnsecs; set { _outPointHnsecs = value; OnPropertyChanged(); } }
        public bool IsLooping { get => _isLooping; set { _isLooping = value; OnPropertyChanged(); } }
        public bool HoldLastFrame { get => _holdLastFrame; set { _holdLastFrame = value; OnPropertyChanged(); } }
        public long TransitionDurationHnsecs { get => _transitionDurationHnsecs; set { _transitionDurationHnsecs = value; OnPropertyChanged(); } }

        public event PropertyChangedEventHandler? PropertyChanged;
        protected void OnPropertyChanged([CallerMemberName] string? name = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        }
    }
}
