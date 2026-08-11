using System;
using System.Collections.Generic;
using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Threading;

namespace MPlaylistApp
{
    public class EngineConductor
    {
        private ConcurrentDictionary<string, MediaCue> _cueIndex = new ConcurrentDictionary<string, MediaCue>();
        private List<MediaCue> _playlistOrder = new List<MediaCue>();
        private readonly object _playlistLock = new object();

        private double _currentVideoTime = 0.0;
        private double _currentAudioTime = 0.0;
        
        public double CurrentVideoTime 
        { 
            get => Volatile.Read(ref _currentVideoTime); 
            set => Volatile.Write(ref _currentVideoTime, value); 
        }
        
        public double CurrentAudioTime 
        { 
            get => Volatile.Read(ref _currentAudioTime); 
            set => Volatile.Write(ref _currentAudioTime, value); 
        }

        private Thread _conductorThread;
        private volatile bool _isRunning = false;

        private MediaCue? _activeCue = null;
        private MediaCue? _nextCue = null;
        private bool _bDeckLoaded = false;

        public Action<MediaCue>? OnCueActivated;

        public EngineConductor()
        {
            _conductorThread = new Thread(ConductorLoop)
            {
                IsBackground = true,
                Priority = ThreadPriority.Highest
            };
        }

        public void Start()
        {
            if (!_isRunning)
            {
                _isRunning = true;
                _conductorThread.Start();
            }
        }

        public void Stop()
        {
            _isRunning = false;
        }

        public void UpdatePlaylistTopology(IEnumerable<MediaCue> cues)
        {
            lock (_playlistLock)
            {
                _playlistOrder.Clear();
                _cueIndex.Clear();
                foreach (var cue in cues)
                {
                    _playlistOrder.Add(cue);
                    _cueIndex[cue.CueID] = cue;
                }
            }
        }

        public void SetActiveCue(MediaCue? cue)
        {
            _activeCue = cue;
            _bDeckLoaded = false;
            _nextCue = null;
        }

        private void ConductorLoop()
        {
            while (_isRunning)
            {
                Thread.Sleep(5); // ~200Hz tick

                if (EngineInterop.mplaylist_get_diagnostics(out double audioTime, out double videoTime))
                {
                    CurrentAudioTime = audioTime;
                    CurrentVideoTime = videoTime;

                    if (_activeCue != null)
                    {
                        ulong currentPlayheadHNS = (ulong)(CurrentVideoTime * 10000000.0);

                        if (_activeCue.OutPointHNS > 0)
                        {
                            long timeRemaining = (long)_activeCue.OutPointHNS - (long)currentPlayheadHNS;

                            // The Lookahead Math (B-Deck Pre-load): 5 seconds (50,000,000 HNS)
                            if (timeRemaining <= 50_000_000 && timeRemaining > 0 && !_bDeckLoaded)
                            {
                                ResolveNextCue();
                                if (_nextCue != null)
                                {
                                    IntPtr ptr = Marshal.StringToCoTaskMemUTF8(_nextCue.FilePath);
                                    try
                                    {
                                        var ffiCue = new FfiCue
                                        {
                                            FilePath = ptr,
                                            InPointHnsecs = (long)_nextCue.InPointHNS,
                                            OutPointHnsecs = (long)_nextCue.OutPointHNS,
                                            IsLooping = (byte)(_nextCue.IsLooping ? 1 : 0),
                                            HoldLastFrame = (byte)(_nextCue.HoldLastFrame ? 1 : 0),
                                            TransitionDurationHnsecs = (long)(_nextCue.TransitionDuration * 10000000.0)
                                        };
                                        EngineInterop.mplaylist_load_cue(ffiCue);
                                        _bDeckLoaded = true;
                                    }
                                    finally
                                    {
                                        Marshal.FreeCoTaskMem(ptr);
                                    }
                                }
                            }

                            // The Execution Guillotine
                            if (currentPlayheadHNS >= _activeCue.OutPointHNS)
                            {
                                switch (_activeCue.EndBehavior)
                                {
                                    case EndBehavior.Stop:
                                        EngineInterop.mplaylist_stop();
                                        _activeCue.IsActivePlaying = false;
                                        _activeCue = null;
                                        _bDeckLoaded = false;
                                        break;
                                    case EndBehavior.LoopForever:
                                        EngineInterop.mplaylist_scrub_to((long)_activeCue.InPointHNS);
                                        break;
                                    case EndBehavior.LoopCount:
                                        _activeCue.CurrentLoopCount++;
                                        if (_activeCue.CurrentLoopCount < _activeCue.TargetLoopCount)
                                        {
                                            EngineInterop.mplaylist_scrub_to((long)_activeCue.InPointHNS);
                                        }
                                        else
                                        {
                                            FireNextCue();
                                        }
                                        break;
                                    default:
                                    case EndBehavior.GotoTarget:
                                        FireNextCue();
                                        break;
                                }
                            }
                        }
                    }
                }
            }
        }

        private void ResolveNextCue()
        {
            _nextCue = null;
            if (_activeCue == null) return;

            if (_activeCue.EndBehavior == EndBehavior.GotoTarget && !string.IsNullOrEmpty(_activeCue.TargetCueID))
            {
                if (_cueIndex.TryGetValue(_activeCue.TargetCueID, out var target))
                {
                    _nextCue = target;
                    return;
                }
            }

            // Default sequential lookup
            lock (_playlistLock)
            {
                int index = _playlistOrder.IndexOf(_activeCue);
                if (index >= 0 && index < _playlistOrder.Count - 1)
                {
                    _nextCue = _playlistOrder[index + 1];
                }
                else if (index == _playlistOrder.Count - 1 && _playlistOrder.Count > 0)
                {
                    _nextCue = _playlistOrder[0];
                }
            }
        }

        private void FireNextCue()
        {
            if (_nextCue != null)
            {
                lock (_playlistLock)
                {
                    int targetIndex = _playlistOrder.IndexOf(_nextCue);
                    
                    uint transMs = (uint)(_nextCue.TransitionDuration * 1000);
                    long inHnsecs = (long)_nextCue.InPointHNS;
                    long outHnsecs = (long)_nextCue.OutPointHNS;

                    EngineInterop.mplaylist_fire_cue((uint)targetIndex, transMs, inHnsecs, outHnsecs);
                    EngineInterop.mplaylist_set_volume_db((float)_nextCue.VolumeDb);

                    if (_activeCue != null) _activeCue.CurrentLoopCount = 0; // reset loop count
                    
                    // Dispatch to UI thread just for visual updates (no execution logic)
                    OnCueActivated?.Invoke(_nextCue);
                    
                    _activeCue = _nextCue;
                    _bDeckLoaded = false;
                    _nextCue = null;
                }
            }
            else
            {
                EngineInterop.mplaylist_stop();
                if (_activeCue != null) _activeCue.IsActivePlaying = false;
                _activeCue = null;
                _bDeckLoaded = false;
            }
        }
    }
}
