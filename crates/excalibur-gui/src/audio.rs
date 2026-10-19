use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::io::Cursor;
use std::time::Duration;

/// Procedural stereo audio synthesizer generating warm, harmonic tones for browser playback.
pub struct HarmonicSynthSource {
    sample_rate: u32,
    sample_index: u64,
    scale_notes: [f32; 8],
}

impl HarmonicSynthSource {
    pub fn new() -> Self {
        Self {
            sample_rate: 44100,
            sample_index: 0,
            // C pentatonic / A minor harmonic frequencies (Hz)
            scale_notes: [220.0, 261.63, 293.66, 329.63, 392.0, 440.0, 523.25, 587.33],
        }
    }
}

impl Default for HarmonicSynthSource {
    fn default() -> Self {
        Self::new()
    }
}

impl Iterator for HarmonicSynthSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let t = self.sample_index as f32 / self.sample_rate as f32;
        let is_left_channel = self.sample_index.is_multiple_of(2);
        self.sample_index = self.sample_index.wrapping_add(1);

        // Transition through scale notes every 0.5 seconds
        let note_idx = ((t * 2.0) as usize) % self.scale_notes.len();
        let base_freq = self.scale_notes[note_idx];

        // Soft amplitude envelope per note
        let note_phase = (t * 2.0).fract();
        let envelope = (-(note_phase * 4.0)).exp() * 0.7 + 0.15;

        // Fundamental tone + gentle octave harmonic
        let pan_offset = if is_left_channel { 0.0 } else { 0.25 };
        let omega = 2.0 * std::f32::consts::PI * base_freq;
        let s1 = (omega * t + pan_offset).sin();
        let s2 = (omega * 2.0 * t).sin() * 0.35;
        let s3 = (omega * 0.5 * t).sin() * 0.25;

        let sample = (s1 + s2 + s3) * envelope * 0.25;
        Some(sample.clamp(-0.8, 0.8))
    }
}

impl Source for HarmonicSynthSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// In-browser audio playback engine integrating hardware sound output with media controls.
pub struct AudioEngine {
    _stream: Option<OutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    active_id: String,
    volume: f32,
    is_muted: bool,
    speed: f32,
    is_playing: bool,
    is_synth_mode: bool,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (stream, stream_handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(e) => {
                tracing::warn!("Audio output device unavailable: {}", e);
                (None, None)
            }
        };

        let sink = stream_handle.as_ref().and_then(|h| Sink::try_new(h).ok());
        if let Some(s) = &sink {
            s.pause();
            s.set_volume(0.85);
        }

        Self {
            _stream: stream,
            stream_handle,
            sink,
            active_id: String::new(),
            volume: 0.85,
            is_muted: false,
            speed: 1.0,
            is_playing: false,
            is_synth_mode: false,
        }
    }

    pub fn is_available(&self) -> bool {
        self.sink.is_some()
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        self.apply_volume();
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.is_muted = muted;
        self.apply_volume();
    }

    pub fn set_speed(&mut self, spd: f32) {
        self.speed = spd.clamp(0.25, 3.0);
        if let Some(sink) = &self.sink {
            sink.set_speed(self.speed);
        }
    }

    fn apply_volume(&self) {
        if let Some(sink) = &self.sink {
            if self.is_muted {
                sink.set_volume(0.0);
            } else {
                sink.set_volume(self.volume);
            }
        }
    }

    pub fn play(&mut self) {
        self.is_playing = true;
        if let Some(sink) = &self.sink {
            sink.play();
        }
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
        if let Some(sink) = &self.sink {
            sink.pause();
        }
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.active_id.clear();
        self.is_synth_mode = false;
        if let Some(sink) = &self.sink {
            sink.stop();
        }
    }

    /// Starts or resumes playback for a given track ID using the procedural synthesizer if no stream is loaded.
    pub fn ensure_playback(&mut self, track_id: &str, should_play: bool) {
        if self.active_id != track_id || self.sink.as_ref().map(|s| s.empty()).unwrap_or(true) {
            self.start_synth_track(track_id);
        }

        if should_play {
            self.play();
        } else {
            self.pause();
        }
    }

    /// Initializes a synthesized harmonic sound track matching the media playback visualizer.
    pub fn start_synth_track(&mut self, track_id: &str) {
        self.active_id = track_id.to_string();
        self.is_synth_mode = true;

        if let Some(handle) = &self.stream_handle {
            // Re-create sink to ensure a clean queue
            if let Ok(new_sink) = Sink::try_new(handle) {
                let source = HarmonicSynthSource::new();
                new_sink.append(source);
                self.sink = Some(new_sink);
                self.apply_volume();
                self.set_speed(self.speed);
            }
        }
    }

    /// Loads and plays real audio bytes (e.g. MP3, WAV, Vorbis, FLAC).
    pub fn load_audio_bytes(&mut self, track_id: &str, bytes: &[u8]) -> Result<(), String> {
        let cursor = Cursor::new(bytes.to_vec());
        let decoder = Decoder::new(cursor).map_err(|e| format!("Failed to decode audio: {}", e))?;

        if let Some(handle) = &self.stream_handle {
            if let Ok(new_sink) = Sink::try_new(handle) {
                new_sink.append(decoder);
                self.sink = Some(new_sink);
                self.active_id = track_id.to_string();
                self.is_synth_mode = false;
                self.apply_volume();
                self.set_speed(self.speed);
                if self.is_playing {
                    self.play();
                } else {
                    self.pause();
                }
                return Ok(());
            }
        }
        Err("Audio sink unavailable".to_string())
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harmonic_synth_source_samples() {
        let mut source = HarmonicSynthSource::new();
        assert_eq!(source.channels(), 2);
        assert_eq!(source.sample_rate(), 44100);

        let mut has_non_zero = false;
        for _ in 0..1000 {
            if let Some(sample) = source.next() {
                assert!((-0.8..=0.8).contains(&sample));
                if sample.abs() > 0.001 {
                    has_non_zero = true;
                }
            }
        }
        assert!(
            has_non_zero,
            "Harmonic synthesizer must generate audible samples"
        );
    }

    #[test]
    fn test_audio_engine_controls() {
        let mut engine = AudioEngine::new();
        engine.set_volume(0.75);
        assert!((engine.volume - 0.75).abs() < 0.001);

        engine.set_muted(true);
        assert!(engine.is_muted);

        engine.set_speed(1.5);
        assert!((engine.speed - 1.5).abs() < 0.001);

        engine.play();
        assert!(engine.is_playing());

        engine.pause();
        assert!(!engine.is_playing());

        engine.ensure_playback("test_video_123", true);
        assert_eq!(engine.active_id, "test_video_123");
        assert!(engine.is_synth_mode);
        assert!(engine.is_playing());
    }
}
