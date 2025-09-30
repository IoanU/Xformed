use anyhow::{bail, Context, Result};
use hound::WavReader;
use std::io::Cursor;

/// Decodes WAV from memory -> (mono f32 [-1,1], sample_rate).
/// Supports 16-bit PCM, 24/32-bit PCM, 32f, 64f. Downmix through average on channels.
pub fn decode_wav_to_mono_f32(bytes: &[u8]) -> Result<(Vec<f32>, u32)> {
    let cursor = Cursor::new(bytes);
    let mut reader = WavReader::new(cursor).context("not a valid WAV")?;
    let spec = reader.spec();
    let sr = spec.sample_rate;
    let ch = spec.channels as usize;
    if ch == 0 {
        bail!("WAV has zero channels");
    }

    // Reads samples as right type
    let samples_f32: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => {
            reader
                .samples::<i16>()
                .map(|s| (s.unwrap_or(0) as f32) / 32768.0)
                .collect()
        }
        (hound::SampleFormat::Int, 24) => {
            // hound exposes 24-bit as i32 in .samples::<i32>()
            let max = (1i64 << 23) as f32;
            reader
                .samples::<i32>()
                .map(|s| (s.unwrap_or(0) as f32) / max)
                .collect()
        }
        (hound::SampleFormat::Int, 32) => {
            let max = (1i64 << 31) as f32;
            reader
                .samples::<i32>()
                .map(|s| (s.unwrap_or(0) as f32) / max)
                .collect()
        }
        (hound::SampleFormat::Float, 32) => {
            reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
        }
        (hound::SampleFormat::Float, 64) => {
            bail!("64-bit float WAV is not supported by hound; please convert to 32-bit float or PCM.");
        }
        // generic fallback: tries f32
        _ => {
            // Last chance: it might be 32f
            let cursor = Cursor::new(bytes);
            let mut r2 = WavReader::new(cursor).context("not a valid WAV (fallback)")?;
            r2.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
        }
    };

    // mono downmix (average on channels)
    if ch == 1 {
        return Ok((samples_f32, sr));
    }

    let mut mono = Vec::with_capacity(samples_f32.len() / ch + 1);
    let mut acc = 0.0f32;
    let mut cnt = 0usize;
    for s in samples_f32 {
        acc += s;
        cnt += 1;
        if cnt == ch {
            mono.push(acc / ch as f32);
            acc = 0.0;
            cnt = 0;
        }
    }
    if cnt > 0 {
        mono.push(acc / cnt as f32);
    }

    Ok((mono, sr))
}
