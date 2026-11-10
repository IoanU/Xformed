//! Creative transformations share source-derived atmosphere; measurements are kept on /json routes.
use crate::mood::{self, Atmosphere};
use crate::{ConvertResponse, OutputArtifact, TransformOpts};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use image::ImageFormat;
use std::io::Cursor;

fn creative_info(a: &Atmosphere, title: String, description: &str) -> OutputArtifact {
    OutputArtifact::CreativeInfo {
        title,
        mood: a.mood().name().into(),
        description: description.into(),
        seed: a.seed.to_string(),
    }
}
fn image_response(a: &Atmosphere, opts: &TransformOpts) -> Result<ConvertResponse> {
    let a = crate::fractal::variation(a, opts.variation);
    let style = opts.image_style.as_deref().unwrap_or("nebula");
    let img = crate::fractal::render(&a, style);
    let mut bytes = Cursor::new(Vec::new());
    img.write_to(&mut bytes, ImageFormat::Png)?;
    let title = crate::fractal::identity(&a, style);
    Ok(ConvertResponse {
        artifacts: vec![
            OutputArtifact::PngBase64 {
                data_b64: B64.encode(bytes.into_inner()),
            },
            creative_info(&a, title, "1280 × 832 · Source-derived composition"),
        ],
    })
}
fn writing_response(a: Atmosphere, opts: &TransformOpts, source: &str) -> Result<ConvertResponse> {
    let a = a.variant(opts.variation);
    let (title, text) = mood::writing(&a, opts.text_style.as_deref() == Some("vignette"), source);
    Ok(ConvertResponse {
        artifacts: vec![
            OutputArtifact::Text { text },
            creative_info(
                &a,
                title,
                "An imaginative reading of the mood, rather than a literal description.",
            ),
        ],
    })
}
pub fn text_to_image(text: &str, opts: &TransformOpts) -> Result<ConvertResponse> {
    image_response(&mood::text_atmosphere(text), opts)
}
pub fn image_to_text(bytes: &[u8], opts: &TransformOpts) -> Result<ConvertResponse> {
    let img = visual_features::load_image_bounded(bytes)?;
    let f = visual_features::analyze_image(&img)?;
    let sample = img.thumbnail(16, 16).to_rgb8();
    let mut contour = Vec::new();
    for x in 0..sample.width() {
        let v = sample
            .enumerate_pixels()
            .filter(|(xx, _, _)| *xx == x)
            .map(|(_, _, p)| (p[0] as f32 + p[1] as f32 + p[2] as f32) / 765.0)
            .sum::<f32>()
            / sample.height() as f32;
        contour.push(v);
    }
    let warm = ((f.hsv_mean_h.to_radians().cos() + 1.0) * 0.5) * f.hsv_mean_s;
    let valence = (f.hsv_mean_v * 1.3 - 0.7 + warm * 0.5).clamp(-1.0, 1.0);
    let energy = (0.12 + f.edge_density * 0.35 + f.contrast_luma_std * 0.8 + f.hsv_mean_s * 0.2)
        .clamp(0.0, 1.0);
    let a = Atmosphere {
        valence,
        energy,
        complexity: f.entropy_luma_bits / 8.0,
        hue: f.hsv_mean_h,
        seed: mood::hash(bytes),
        rising: contour.last() > contour.first(),
        contour,
    };
    writing_response(a, opts, "image")
}
fn audio_atmosphere(mono: &[f32], sr: u32) -> Result<Atmosphere> {
    anyhow::ensure!(!mono.is_empty() && sr > 0, "empty audio");
    // The creative engine needs a broad impression, not expensive pitch analysis.
    let energy = (mono.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / mono.len() as f64)
        .sqrt()
        .min(1.0) as f32;
    let mut contour = Vec::new();
    let block = (mono.len() / 96).max(1);
    for chunk in mono.chunks(block).take(96) {
        contour.push(
            (chunk.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / chunk.len() as f64)
                .sqrt()
                .min(1.0) as f32,
        );
    }
    let max = contour.iter().copied().fold(1e-6f32, f32::max);
    for v in &mut contour {
        *v /= max;
    }
    let crossings = mono
        .windows(2)
        .filter(|s| (s[0] >= 0.0) != (s[1] >= 0.0))
        .count() as f32
        / mono.len() as f32;
    let variation =
        contour.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / contour.len() as f32;
    // Sample the spectrum across the whole clip; linear RMS alone made ordinary
    // recordings collapse into the same quiet mood regardless of timbre or dynamics.
    use rustfft::{num_complex::Complex, FftPlanner};
    let n = 2048usize;
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);
    let mut buffer = vec![Complex::new(0.0, 0.0); n];
    let mut centroid = 0.0;
    let mut flatness = 0.0;
    for frame in 0..24 {
        let start = frame * mono.len().saturating_sub(n) / 24;
        for (i, v) in buffer.iter_mut().enumerate() {
            *v = Complex::new(
                mono.get(start + i).copied().unwrap_or(0.0).clamp(-1.0, 1.0)
                    * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos()),
                0.0,
            );
        }
        fft.process(&mut buffer);
        let mut mass = 0.0;
        let mut moment = 0.0;
        let mut logs = 0.0;
        for (i, v) in buffer.iter().take(n / 2).enumerate().skip(1) {
            let magnitude = v.norm().max(1e-8);
            mass += magnitude;
            moment += magnitude * i as f32 * sr as f32 / n as f32;
            logs += magnitude.ln();
        }
        centroid += moment / mass.max(1e-8);
        flatness += (logs / (n / 2 - 1) as f32).exp() / (mass / (n / 2 - 1) as f32).max(1e-8);
    }
    centroid /= 24.0;
    flatness = (flatness / 24.0).clamp(0.0, 1.0);
    let loudness = ((20.0 * energy.max(1e-9).log10() + 50.0) / 40.0).clamp(0.0, 1.0);
    let arousal = (loudness * 0.75 + variation * 1.2 + crossings * 1.5).clamp(0.0, 1.0);
    let valence = if energy < 0.005 {
        -0.7
    } else {
        (centroid / 3000.0 * 0.85 + (1.0 - flatness) * 0.3 - 0.55).clamp(-0.8, 0.8)
    };
    let bytes: Vec<u8> = mono
        .iter()
        .step_by((mono.len() / 4096).max(1))
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let rising = contour[contour.len() / 2..].iter().sum::<f32>()
        > contour[..contour.len() / 2].iter().sum::<f32>();
    Ok(Atmosphere {
        valence,
        energy: arousal,
        complexity: (variation * 4.0 + crossings).clamp(0.0, 1.0),
        hue: 210.0 - crossings * 500.0,
        seed: mood::hash(&bytes),
        contour,
        rising,
    })
}
pub fn audio_to_image(mono: &[f32], sr: u32, opts: &TransformOpts) -> Result<ConvertResponse> {
    image_response(&audio_atmosphere(mono, sr)?, opts)
}
pub fn audio_to_text(mono: &[f32], sr: u32, opts: &TransformOpts) -> Result<ConvertResponse> {
    writing_response(audio_atmosphere(mono, sr)?, opts, "audio")
}

#[cfg(test)]
mod signal_tests {
    use super::*;
    #[test]
    fn silence_and_extreme_float_audio_stay_finite() {
        for samples in [vec![0.0; 2048], vec![f32::MAX; 2048]] {
            let a = audio_atmosphere(&samples, 44100).unwrap();
            assert!(
                a.valence.is_finite()
                    && a.energy.is_finite()
                    && a.contour.iter().all(|x| x.is_finite())
            );
        }
    }
}
