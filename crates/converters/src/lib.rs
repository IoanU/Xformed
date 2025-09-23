
use anyhow::Result;
use serde::{Deserialize, Serialize};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use melody_core::{MonophonicMidi, degree_to_midi, ScaleKind};
use melody_synth::{render_wav_bytes, Osc};
use text_features::{sentiment_polarity, syllable_guess};
use visual_features::dominant_hsv;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TransformOpts {
    pub instrument: Option<String>,
    pub tempo: Option<u32>,         // bpm
    pub mood: Option<String>,       // "major"|"minor"|"auto"
    pub jumpiness: Option<f32>,     // 0..1
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum InputPayload {
    Text{ text: String },
    AudioBase64{ data_b64: String },
    ImageBase64{ data_b64: String },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum OutputArtifact {
    MidiBase64{ data_b64: String },
    WavBase64{ data_b64: String },
    Json{ data: serde_json::Value },
    PngBase64{ data_b64: String },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ConvertRequest { pub from: String, pub to: String, pub options: TransformOpts, pub payload: InputPayload }

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ConvertResponse { pub artifacts: Vec<OutputArtifact> }

fn osc_from_str(s: Option<&str>) -> Osc {
    match s {
        Some("square") => Osc::Square,
        Some("saw") => Osc::Saw,
        _ => Osc::Sine,
    }
}

/// TEXT -> AUDIO (melody-only)
pub fn text_to_audio(text: &str, opts: &TransformOpts) -> Result<(Vec<u8>, Vec<u8>)> {
    let s = sentiment_polarity(text);
    let minor = match opts.mood.as_deref() {
        Some("minor") => true,
        Some("major") => false,
        _ => s < 0.0,
    };
    let tempo = opts.tempo.unwrap_or(if s >= 0.0 { 110 } else { 85 });
    let scale = if minor { ScaleKind::Minor } else { ScaleKind::Major };
    let root = if minor { 62 } else { 64 }; // D or E

    let mut m = MonophonicMidi::new(tempo);
    let _words = text.split_whitespace().count().max(8).min(64);
    let mut degree = 0i32;
    let jumpiness = opts.jumpiness.unwrap_or(0.2);

    // naive contour from syllables/words
    let n_notes = (syllable_guess(text) as f32 * 0.5).round() as usize;
    let n_notes = n_notes.clamp(8, 64);
    let mut seed = 7u32; // small LCG for deterministic-ish jitter
    for i in 0..n_notes {
        // pseudo-random step with jumpiness
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let r = (seed >> 16) as f32 / 65535.0;
        let step = if r < jumpiness { if r < jumpiness/2.0 { 5 } else { -5 } } else { if r < 0.6 { 1 } else { -1 } };
        degree += step;

        let pitch = degree_to_midi(root as i32, degree, scale).clamp(36, 96) as u8;
        let t0 = i as f32 * 0.32;
        let t1 = t0 + 0.30;
        m.push(pitch, t0, t1, 90);
    }

    let midi = m.to_mid_bytes()?;
    let wav = render_wav_bytes(&m, 44_100, osc_from_str(opts.instrument.as_deref()))?;
    Ok((midi, wav))
}

/// IMAGE -> AUDIO (melody-only)
pub fn image_to_audio(img_b: &[u8], opts: &TransformOpts) -> Result<(Vec<u8>, Vec<u8>)> {
    let hsv = dominant_hsv(img_b)?;
    // map hue to mode, value to tempo, saturation to velocity bias
    let minor = (hsv.h >= 160.0 && hsv.h <= 260.0) || (opts.mood.as_deref() == Some("minor"));
    let tempo = opts.tempo.unwrap_or(if hsv.v > 0.6 { 115 } else { 85 });
    let scale = if minor { ScaleKind::Minor } else { ScaleKind::Major };
    let root = if minor { 62 } else { 64 };

    let mut m = MonophonicMidi::new(tempo);
    let mut degree = 0i32;
    let mut seed = ((hsv.h * 10.0) as u32).max(1);
    for i in 0..32 {
        // interval trend from hue & saturation
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let r = (seed >> 16) as f32 / 65535.0;
        let step_pool = if hsv.s > 0.5 { [2, -2, 5, -5] } else { [1, -1, 2, -2] };
        let step = step_pool[(r * step_pool.len() as f32) as usize % step_pool.len()];
        degree += step;

        let pitch = degree_to_midi(root as i32, degree, scale).clamp(36, 96) as u8;
        let vel_f32 = (70.0 + 50.0 * hsv.s).min(127.0f32);
        let vel = vel_f32 as u8;
        let t0 = i as f32 * 0.30;
        m.push(pitch, t0, t0 + 0.28, vel);
    }

    let midi = m.to_mid_bytes()?;
    let wav = render_wav_bytes(&m, 44_100, osc_from_str(opts.instrument.as_deref()))?;
    Ok((midi, wav))
}

/// Public entry helper to route based on request (MVP supports only text->audio and image->audio)
pub fn handle_convert(req: ConvertRequest) -> Result<ConvertResponse> {
    let mut artifacts = vec![];
    match (req.from.as_str(), req.to.as_str(), req.payload) {
        ("text","audio", InputPayload::Text{ text }) => {
            let (midi, wav) = text_to_audio(&text, &req.options)?;
            artifacts.push(OutputArtifact::MidiBase64{ data_b64: B64.encode(midi) });
            artifacts.push(OutputArtifact::WavBase64{ data_b64: B64.encode(wav) });
        },
        ("image","audio", InputPayload::ImageBase64{ data_b64 }) => {
            let bytes = B64.decode(data_b64).unwrap_or_default();
            let (midi, wav) = image_to_audio(&bytes, &req.options)?;
            artifacts.push(OutputArtifact::MidiBase64{ data_b64: B64.encode(midi) });
            artifacts.push(OutputArtifact::WavBase64{ data_b64: B64.encode(wav) });
        },
        _ => {
            // Unsupported path for now
            artifacts.push(OutputArtifact::Json{ data: serde_json::json!({
                "error": "unsupported transformation in MVP"
            })});
        }
    }
    Ok(ConvertResponse{ artifacts })
}
