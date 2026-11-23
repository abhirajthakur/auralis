//! Audio processing: ring buffer, resampling, WAV encoding, VAD.
//!
//! All functions are pure and sample-based so they are unit-testable without
//! an audio device. The capture thread in `src-tauri/src/audio` feeds cpal
//! samples into these primitives.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("unsupported channel count: {0}")]
    UnsupportedChannels(u16),
    #[error("empty buffer")]
    Empty,
}

/// A lock-free-ish capture buffer: the cpal callback pushes samples, the
/// encoder drains. Uses a `std::sync::Mutex<Vec<f32>>` behind a tiny API —
/// holds the lock for microseconds, which is fine at callback rates.
#[derive(Default)]
pub struct CaptureBuffer {
    inner: std::sync::Mutex<Vec<f32>>,
}

impl CaptureBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push interleaved samples from the realtime callback.
    pub fn push(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        self.inner.lock().unwrap().extend_from_slice(samples);
    }

    /// Drain everything captured so far.
    pub fn drain(&self) -> Vec<f32> {
        std::mem::take(&mut *self.inner.lock().unwrap())
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Naive linear resampler. Good enough for speech-to-STT downmixing
/// (e.g. 48 kHz → 16 kHz); providers tolerate the slight aliasing.
pub fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if input.is_empty() || from_rate == to_rate {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = ((input.len() as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        let frac = (pos - idx as f64) as f32;
        let s0 = input[idx];
        let s1 = input.get(idx + 1).copied().unwrap_or(s0);
        out.push(s0 + (s1 - s0) * frac);
    }
    out
}

/// Convert interleaved stereo (or N-channel) f32 input to mono by averaging.
pub fn to_mono(input: &[f32], channels: u16) -> Result<Vec<f32>, AudioError> {
    match channels {
        0 => Err(AudioError::UnsupportedChannels(0)),
        1 => Ok(input.to_vec()),
        n => {
            let n = n as usize;
            if input.len() % n != 0 {
                return Err(AudioError::UnsupportedChannels(channels));
            }
            let frames = input.len() / n;
            let mut out = Vec::with_capacity(frames);
            for f in 0..frames {
                let sum: f32 = input[f * n..(f + 1) * n].iter().sum();
                out.push(sum / n as f32);
            }
            Ok(out)
        }
    }
}

/// Root mean square of a sample window — the basis of the VAD gate.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Simple RMS voice-activity gate over sliding windows.
/// Returns the indexes (in samples) where speech starts and ends,
/// or `None` when the whole clip is below the threshold.
pub fn detect_speech_bounds(
    samples: &[f32],
    sample_rate: u32,
    silence_threshold: f32,
) -> Option<(usize, usize)> {
    let window = (sample_rate as usize / 100).max(1); // 10 ms windows
    let mut first = None;
    let mut last = None;
    for (w, chunk) in samples.chunks(window).enumerate() {
        if rms(chunk) > silence_threshold {
            if first.is_none() {
                first = Some(w * window);
            }
            last = Some(((w + 1) * window).min(samples.len()));
        }
    }
    match (first, last) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}

/// Trim leading/trailing silence, keeping `preroll_ms` before speech onset
/// so the first syllable is never clipped.
pub fn trim_silence(
    samples: &[f32],
    sample_rate: u32,
    silence_threshold: f32,
    preroll_ms: u32,
) -> Vec<f32> {
    let Some((start, end)) = detect_speech_bounds(samples, sample_rate, silence_threshold) else {
        return Vec::new();
    };
    let preroll = (sample_rate as usize / 1000) * preroll_ms as usize;
    let start = start.saturating_sub(preroll);
    samples[start..end].to_vec()
}

/// Encode mono 16-bit PCM as a complete WAV file (RIFF header + data).
/// Every remote STT provider in Auralis accepts this container.
pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let pcm: Vec<i16> = samples.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
    let data_len = pcm.len() * 2;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Parse the sample rate from a WAV header — used in tests and for
/// validating recorded files before upload.
pub fn wav_sample_rate(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    Some(u32::from_le_bytes(bytes[24..28].try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_push_and_drain() {
        let buf = CaptureBuffer::new();
        assert!(buf.is_empty());
        buf.push(&[0.1, 0.2, 0.3]);
        buf.push(&[0.4]);
        assert_eq!(buf.len(), 4);
        let drained = buf.drain();
        assert_eq!(drained, vec![0.1, 0.2, 0.3, 0.4]);
        assert!(buf.is_empty());
    }

    #[test]
    fn resample_48k_to_16k() {
        // 4800 samples at 48 kHz = 0.1 s → 1600 samples at 16 kHz.
        let input: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.01).sin()).collect();
        let out = resample_linear(&input, 48_000, 16_000);
        assert_eq!(out.len(), 1600);
    }

    #[test]
    fn resample_same_rate_is_noop() {
        let input = vec![0.5_f32; 100];
        assert_eq!(resample_linear(&input, 16_000, 16_000), input);
    }

    #[test]
    fn stereo_to_mono_averages() {
        let stereo = [1.0, 0.0, 0.5, 0.5];
        assert_eq!(to_mono(&stereo, 2).unwrap(), vec![0.5, 0.5]);
        assert!(to_mono(&stereo, 0).is_err());
        assert!(to_mono(&[1.0, 2.0, 3.0], 2).is_err());
    }

    #[test]
    fn rms_zero_for_silence() {
        assert_eq!(rms(&[0.0; 100]), 0.0);
        assert!(rms(&[0.5; 100]) > 0.4);
    }

    #[test]
    fn speech_bounds_found_in_middle() {
        let sr: u32 = 16_000;
        let mut samples = vec![0.0_f32; sr as usize]; // 1 s silence
        samples[(sr as usize) / 2..(sr as usize) / 2 + (sr as usize) / 4].fill(0.5); // 250 ms speech
        let (a, b) = detect_speech_bounds(&samples, sr, 0.01).unwrap();
        assert!(a >= sr as usize / 2 - 100 && a <= sr as usize / 2 + 100);
        assert!(b >= sr as usize * 3 / 4 - 100);
    }

    #[test]
    fn all_silence_yields_no_bounds() {
        let samples = vec![0.0_f32; 16_000];
        assert!(detect_speech_bounds(&samples, 16_000, 0.01).is_none());
    }

    #[test]
    fn trim_keeps_preroll() {
        let sr: u32 = 16_000;
        let mut samples = vec![0.0_f32; sr as usize];
        samples[(sr as usize) / 2..].fill(0.5); // speech in second half
        let trimmed = trim_silence(&samples, sr, 0.01, 300);
        // 500 ms of preroll available but only 300 ms kept.
        assert_eq!(trimmed.len(), sr as usize / 2 + 300 * (sr as usize) / 1000);
    }

    #[test]
    fn trim_all_silence_is_empty() {
        let samples = vec![0.0_f32; 16_000];
        assert!(trim_silence(&samples, 16_000, 0.01, 300).is_empty());
    }

    #[test]
    fn wav_round_trip() {
        let samples = vec![0.1_f32, -0.2, 0.3];
        let wav = encode_wav(&samples, 16_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(wav_sample_rate(&wav), Some(16_000));
        // data chunk: 3 samples * 2 bytes
        assert_eq!(wav.len(), 44 + 6);
    }

    #[test]
    fn wav_clamps_out_of_range() {
        let wav = encode_wav(&[1.5_f32, -1.5], 16_000);
        let pcm = &wav[44..];
        let s0 = i16::from_le_bytes([pcm[0], pcm[1]]);
        let s1 = i16::from_le_bytes([pcm[2], pcm[3]]);
        assert_eq!(s0, 32767);
        assert_eq!(s1, -32767);
    }
}
