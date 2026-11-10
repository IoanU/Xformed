//! Procedural instrument voices and stereo mix. No external samples or soundfonts.
use anyhow::Result;
use melody_core::{Composition, Instrument as I};
use std::{
    f32::consts::{PI, TAU},
    io::Cursor,
};

fn noise(mut x: u64) -> f32 {
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    (x.wrapping_mul(2685821657736338717) >> 40) as f32 / 8388608.0 - 1.0
}
fn voice(instrument: I, phase: f32, t: f32, gate: f32, pitch: u8, noise_value: f32) -> f32 {
    let s = phase.sin();
    let release = (1.0
        - (t - gate).max(0.0)
            / match instrument {
                I::Strings | I::WarmPad => 0.65,
                I::Bell => 0.55,
                _ => 0.18,
            })
    .clamp(0.0, 1.0);
    let attack = (t / match instrument {
        I::Strings | I::WarmPad => 0.13,
        I::Flute => 0.035,
        I::Brass => 0.045,
        _ => 0.005,
    })
    .min(1.0);
    let tone = match instrument {
        I::Piano => {
            (s + 0.42 * (phase * 2.001).sin() * (-t * 2.0).exp()
                + 0.18 * (phase * 3.006).sin() * (-t * 4.0).exp())
                * (-t * 1.4).exp()
        }
        I::ElectricPiano => {
            (phase + 2.1 * (phase * 2.0).sin() * (-t * 4.5).exp()).sin() * (-t * 1.5).exp()
        }
        I::Marimba => (s + 0.38 * (phase * 4.01).sin() * (-t * 18.0).exp()) * (-t * 7.0).exp(),
        I::Bell => (s + 0.5 * (phase * 2.76).sin() + 0.24 * (phase * 5.4).sin()) * (-t * 3.1).exp(),
        I::Guitar => {
            (s + 0.45 * (phase * 2.0).sin()
                + 0.22 * (phase * 3.0).sin()
                + 0.12 * (phase * 4.0).sin())
                * (-t * 3.5).exp()
        }
        I::Strings => {
            (s + 0.3 * (phase * 2.0).sin()
                + 0.16 * (phase * 3.0).sin()
                + 0.08 * (phase * 4.0).sin())
                * (0.92 + 0.08 * (t * TAU * 5.0).sin())
        }
        I::WarmPad => 0.6 * s + 0.25 * (phase * 1.003).sin() + 0.15 * (phase * 0.5).sin(),
        I::Flute => {
            (phase + 0.025 * (TAU * 5.2 * t).sin()).sin()
                + 0.12 * (phase * 2.0).sin()
                + noise_value * 0.025
        }
        I::Organ => {
            0.6 * s
                + 0.25 * (phase * 2.0).sin()
                + 0.18 * (phase * 3.0).sin()
                + 0.12 * (phase * 4.0).sin()
        }
        I::Brass => {
            (s + 0.4 * (phase * 2.0).sin() + 0.2 * (phase * 3.0).sin())
                * (0.7 + 0.3 * (-t * 4.0).exp())
        }
        I::Bass => (s + 0.3 * (phase * 2.0).sin() + 0.12 * (phase * 3.0).sin()) * (-t * 1.1).exp(),
        I::Drums => match pitch {
            36 => {
                (TAU * (48.0 * t + 6.0 * (1.0 - (-t * 30.0).exp()))).sin() * (-t * 17.0).exp()
                    + noise_value * 0.12 * (-t * 90.0).exp()
            }
            38 => {
                noise_value * (-t * 23.0).exp() * 0.8
                    + (TAU * 185.0 * t).sin() * (-t * 30.0).exp() * 0.35
            }
            39 => noise_value * (-t * 32.0).exp() * (0.5 + 0.5 * (TAU * 110.0 * t).sin().abs()),
            42 | 46 | 49 => {
                let decay = if pitch == 42 {
                    65.0
                } else if pitch == 46 {
                    17.0
                } else {
                    5.0
                };
                (noise_value * 0.65
                    + (TAU * 7331.0 * t).sin() * 0.12
                    + (TAU * 10421.0 * t).sin() * 0.12)
                    * (-t * decay).exp()
            }
            _ => {
                (TAU * (100.0 + (pitch as f32 - 45.0) * 25.0) * t).sin() * (-t * 18.0).exp()
                    + noise_value * 0.1 * (-t * 25.0).exp()
            }
        },
    };
    tone * attack * release
}
pub fn render_composition(score: &Composition, sr: u32) -> Result<Vec<u8>> {
    score.validate()?;
    anyhow::ensure!(
        (8000..=96000).contains(&sr),
        "sample rate must be 8000..96000"
    );
    let seconds = score.duration_beats * 60.0 / score.tempo_bpm as f32;
    let frames = ((seconds + 0.8) * sr as f32).ceil() as usize;
    let mut left = vec![0.0f32; frames];
    let mut right = vec![0.0f32; frames];
    for (ti, track) in score.tracks.iter().enumerate() {
        let angle = (track.pan + 1.0) * PI / 4.0;
        let (gl, gr) = (angle.cos() * track.gain, angle.sin() * track.gain);
        for (ni, n) in track.notes.iter().enumerate() {
            let start = (n.start * 60.0 / score.tempo_bpm as f32 * sr as f32).round() as usize;
            let gate = (n.end - n.start) * 60.0 / score.tempo_bpm as f32;
            let tail = if matches!(track.instrument, I::WarmPad | I::Strings) {
                0.65
            } else if track.instrument == I::Bell {
                0.55
            } else {
                0.18
            };
            let count = ((gate + tail) * sr as f32).ceil() as usize;
            let freq = 440.0 * 2.0f32.powf((n.pitch as f32 - 69.0) / 12.0);
            let velocity = (n.velocity as f32 / 127.0).powf(1.3) * 0.20;
            let mut phase = 0.0f32;
            let increment = TAU * freq / sr as f32;
            for i in 0..count.min(frames.saturating_sub(start)) {
                let t = i as f32 / sr as f32;
                let sample = voice(
                    track.instrument,
                    phase,
                    t,
                    gate,
                    n.pitch,
                    noise((i as u64 + 1) ^ ((ni as u64 + 1) << 20) ^ ((ti as u64 + 1) << 40)),
                ) * velocity;
                left[start + i] += sample * gl;
                right[start + i] += sample * gr;
                phase += increment;
            }
        }
    }
    // Cross-fed early reflections and a tempo-locked echo; both stay below the dry mix.
    for delay_seconds in [0.043, 0.071, 60.0 / score.tempo_bpm as f32 * 0.75] {
        let delay = (delay_seconds * sr as f32) as usize;
        for i in delay..frames {
            let l = left[i - delay];
            let r = right[i - delay];
            left[i] += r * score.identity.as_ref().map(|i| i.space).unwrap_or(0.10);
            right[i] += l * score.identity.as_ref().map(|i| i.space).unwrap_or(0.10);
        }
    }
    let peak = left
        .iter()
        .chain(&right)
        .fold(0.0f32, |a, &s| a.max(s.abs()));
    let gain = if peak > 0.0 { 0.92 / peak } else { 1.0 };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 2,
                sample_rate: sr,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        for i in 0..frames {
            let fade = ((frames - i) as f32 / (sr as f32 * 0.12)).min(1.0);
            for v in [left[i], right[i]] {
                writer.write_sample(
                    (v * gain * fade * 32767.0).round().clamp(-32767.0, 32767.0) as i16,
                )?;
            }
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melody_core::{Note, Track};
    #[test]
    fn all_instruments_are_distinct_finite_stereo() {
        let mut fingerprints = std::collections::HashSet::new();
        for instrument in [
            I::Piano,
            I::ElectricPiano,
            I::Marimba,
            I::Bell,
            I::Guitar,
            I::Strings,
            I::WarmPad,
            I::Flute,
            I::Organ,
            I::Brass,
            I::Bass,
            I::Drums,
        ] {
            let s = Composition {
                meter: [4, 4],
                identity: None,
                tempo_bpm: 120,
                key: "C major".into(),
                duration_beats: 1.0,
                sections: vec![],
                notes: vec![],
                tracks: vec![Track {
                    name: "test".into(),
                    instrument,
                    channel: if instrument == I::Drums { 9 } else { 0 },
                    gain: 0.7,
                    pan: -0.5,
                    notes: vec![Note {
                        pitch: if instrument == I::Drums { 38 } else { 60 },
                        start: 0.0,
                        end: 1.0,
                        velocity: 90,
                    }],
                }],
            };
            let wav = render_composition(&s, 8000).unwrap();
            let mut reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
            assert_eq!(reader.spec().channels, 2);
            let samples = reader
                .samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(samples.iter().any(|s| *s != 0));
            assert!(samples.iter().all(|s| s.abs() < 32767));
            assert!(samples.chunks_exact(2).any(|c| c[0] != c[1]));
            fingerprints.insert(
                samples
                    .iter()
                    .fold(0u64, |h, &s| h.wrapping_mul(31).wrapping_add(s as u64)),
            );
        }
        assert_eq!(fingerprints.len(), 12);
    }
}
