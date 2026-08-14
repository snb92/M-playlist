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
        private bool _isTransitioning = false;
        
        private volatile bool _scrubRequested = false;
        private long _scrubTargetHNS = 0;

        public void RequestScrub(long targetHNS)
        {
            if (_activeCue != null && _activeCue.DurationHNS > 0)
            {
                // Clamp the scrub target to 500ms BEFORE the EOF to guarantee a video keyframe
                long maxScrub = (long)_activeCue.DurationHNS - 5_000_000; 
                if (maxScrub < 0) maxScrub = 0;
                
                _scrubTargetHNS = targetHNS > maxScrub ? maxScrub : targetHNS;
            }
            else
            {
                _scrubTargetHNS = targetHNS;
            }
            _scrubRequested = true;
        }

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

                if (_scrubRequested)
                {
                    _scrubRequested = false;
                    _isTransitioning = false; // Reset DDOS latch
                    
                    EngineInterop.mplaylist_scrub_to(_scrubTargetHNS);
                    
                    // THERMODYNAMIC WINDOW: Give the hardware engine 150ms to flush the VRAM swapchain 
                    // and hunt for the next I-Frame before bombarding the COM interface with telemetry requests.
                    System.Threading.Thread.Sleep(150); 
                    continue;
                }

                if (EngineInterop.mplaylist_get_diagnostics(out double audioTime, out double videoTime))
                {
                    CurrentAudioTime = audioTime;
                    CurrentVideoTime = videoTime;

                    if (_activeCue != null)
                    {
                        ulong currentPlayheadHNS = (ulong)(CurrentVideoTime * 10000000.0);

                        // Modality 1: Static Image (Infinite Time)
                        if (_activeCue.Modality == CueModality.WICStatic || _activeCue.OutPointHNS == 0)
                        {
                            // We have infinite time, so pre-load the next cue immediately.
                            if (!_bDeckLoaded)
                            {
                                ResolveNextCue();
                                if (_nextCue != null)
                                {
                                    LoadPolymorphicCue(_nextCue);
                                    _bDeckLoaded = true;
                                }
                            }
                            // Note: The execution Guillotine is entirely bypassed. 
                            // The static image holds perfectly in VRAM until the operator manually fires the next cue.
                        }
                        // Modality 2: Temporal Video (Ticking Time)
                        else if (_activeCue.OutPointHNS > 0)
                        {
                            long timeRemaining = (long)_activeCue.OutPointHNS - (long)currentPlayheadHNS;

                            // The Lookahead Math (B-Deck Pre-load): 5 seconds
                            if (timeRemaining <= 50_000_000 && timeRemaining > 0 && !_bDeckLoaded)
                            {
                                ResolveNextCue();
                                if (_nextCue != null)
                                {
                                    LoadPolymorphicCue(_nextCue);
                                    _bDeckLoaded = true;
                                }
                            }
                            
                            // The Execution Guillotine (50ms margin)
                            ulong marginHNS = 500_000; 
                            ulong triggerPoint = _activeCue.OutPointHNS > marginHNS ? _activeCue.OutPointHNS - marginHNS : _activeCue.OutPointHNS;

                            if (currentPlayheadHNS >= triggerPoint && !_isTransitioning)
                            {
                                _isTransitioning = true;
                                switch (_activeCue.EndBehavior) {
                                    case EndBehavior.Stop:
                                        EngineInterop.mplaylist_stop();
                                        _activeCue.IsActivePlaying = false;
                                        _activeCue = null;
                                        _bDeckLoaded = false;
                                        _isTransitioning = false; // Release immediately
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
                                    case EndBehavior.GotoTarget:
                                        FireNextCue();
                                        break;
                                    default:
                                        FireNextCue();
                                        break;
                                }
                            }

                            // Latch Release (Hysteresis): Release when playhead mathematically drops well below the trigger point
                            if (_isTransitioning && currentPlayheadHNS < triggerPoint - 10_000_000) 
                            {
                                _isTransitioning = false;
                            }
                        }
                    }
                }
            }
        }

        private void LoadPolymorphicCue(MediaCue targetCue)
        {
            IntPtr ptr = Marshal.StringToCoTaskMemUTF8(targetCue.FilePath ?? string.Empty);
            try
            {
                var ffiCue = new FfiCue 
                { 
                    FilePath = ptr,
                    InPointHnsecs = (long)targetCue.InPointHNS,
                    OutPointHnsecs = (long)targetCue.OutPointHNS,
                    IsLooping = (byte)(targetCue.EndBehavior == EndBehavior.LoopForever ? 1 : 0),
                    HoldLastFrame = 1,
                    TransitionDurationHnsecs = (long)(targetCue.TransitionMs * 10000.0),
                    Modality = (byte)targetCue.Modality 
                };
                
                // ONE UNIFIED ENDPOINT FOR ALL ASSETS
                EngineInterop.mplaylist_load_cue(ffiCue); 
            }
            finally { Marshal.FreeCoTaskMem(ptr); }
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
                    
                    IntPtr ptr = Marshal.StringToCoTaskMemUTF8(_nextCue.FilePath ?? string.Empty);
                    try
                    {
                        var ffiCue = new FfiCue
                        {
                            FilePath = ptr,
                            InPointHnsecs = (long)_nextCue.InPointHNS,
                            OutPointHnsecs = (long)_nextCue.OutPointHNS,
                            IsLooping = (byte)(_nextCue.EndBehavior == EndBehavior.LoopForever ? 1 : 0),
                            HoldLastFrame = 1,
                            TransitionDurationHnsecs = (long)(_nextCue.TransitionMs * 10000.0),
                            Modality = (byte)_nextCue.Modality
                        };
                        EngineInterop.mplaylist_fire_cue(ffiCue);
                    }
                    finally
                    {
                        Marshal.FreeCoTaskMem(ptr);
                    }
                    EngineInterop.mplaylist_set_volume_db((float)_nextCue.VolumeDb);

                    if (_activeCue != null) _activeCue.CurrentLoopCount = 0; // reset loop count
                    
                    // Dispatch to UI thread just for visual updates (no execution logic)
                    OnCueActivated?.Invoke(_nextCue);
                    
                    _activeCue = _nextCue;
                    _bDeckLoaded = false;
                    _nextCue = null;
                    _isTransitioning = false;
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

        public void TransportPlay()
        {
            EngineInterop.mplaylist_resume();
        }

        public void TransportPause()
        {
            EngineInterop.mplaylist_pause();
        }

        public void TransportStop()
        {
            EngineInterop.mplaylist_stop();
            lock (_playlistLock)
            {
                if (_activeCue != null) _activeCue.IsActivePlaying = false;
                _activeCue = null;
                _bDeckLoaded = false;
                _nextCue = null;
                _isTransitioning = false;
            }
        }

        public void TransportFireNext()
        {
            lock (_playlistLock)
            {
                ResolveNextCue();
                FireNextCue();
            }
        }

        public void TransportJumpToCue(int index)
        {
            lock (_playlistLock)
            {
                if (index >= 0 && index < _playlistOrder.Count)
                {
                    _nextCue = _playlistOrder[index];
                    FireNextCue();
                }
            }
        }
    }
}
