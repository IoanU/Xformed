//! Xformed Converters – cross-modal transformations + feature extraction.
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};

use melody_core::{degree_to_midi, MonophonicMidi, ScaleKind};
use melody_synth::{render_wav_bytes, Osc};

use audio_features::{decode_wav_to_mono_f32, FeatureExtractor};
use text_features::analyze_text;
use visual_features::analyze_image_bytes;

/// Options that steer transformations (shared between routes).
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TransformOpts {
    /// sine | saw | square | auto (image-based)
    pub instrument: Option<String>,
    /// BPM (if None, auto from features)
    pub tempo: Option<u32>,
    /// "major" | "minor" | "auto"
    pub mood: Option<String>,
    /// 0..1 (how big the scale-degree leaps are)
    pub jumpiness: Option<f32>,
    /// Optional fixed root MIDI note (e.g., 60 = C4)
    pub root_midi: Option<i32>,
    /// Number of bars (approximate), default 2
    pub bars: Option<u32>,
    /// Seed for deterministic variation (currently unused hook)
    pub seed: Option<u64>,
}

/// Payload for input modalities.
/// Note: When (de)serializing over HTTP, we want a tagged form.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum InputPayload {
    Text { text: String },
    ImageBase64 { data_b64: String },
    AudioBase64 { data_b64: String },
}

/// Output artifacts we can emit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OutputArtifact {
    /// WAV bytes as Base64
    WavBase64 { data_b64: String },
    /// MonophonicMidi serialized as JSON, Base64-encoded (so it stays a blob)
    MidiJsonBase64 { data_b64: String },
    /// Generic JSON dump
    Json { data: serde_json::Value },
}

/// Request for a conversion or feature dump.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConvertRequest {
    pub from: String,
    pub to: String,
    pub options: TransformOpts,
    pub payload: InputPayload,
}

/// Response containing one or more artifacts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConvertResponse {
    pub artifacts: Vec<OutputArtifact>,
}

/* ---------- helpers ---------- */

fn clamp01(x: f32) -> f32 {
    x.max(0.0).min(1.0)
}

fn osc_from(opts: &TransformOpts, s_hint: Option<f32>) -> Osc {
    // If instrument explicitly set, honor it; otherwise pick from hint (saturation).
    match opts.instrument.as_deref() {
        Some("square") => Osc::Square,
        Some("saw") => Osc::Saw,
        Some("sine") => Osc::Sine,
        Some("auto") => match s_hint {
            Some(s) if s < 0.3 => Osc::Sine,
            Some(s) if s < 0.6 => Osc::Saw,
            _ => Osc::Square,
        },
        _ => match s_hint {
            Some(s) if s < 0.3 => Osc::Sine,
            Some(s) if s < 0.6 => Osc::Saw,
            _ => Osc::Square,
        },
    }
}

fn scale_from(opts: &TransformOpts, auto_minor: bool) -> ScaleKind {
    match opts.mood.as_deref() {
        Some("minor") => ScaleKind::Minor,
        Some("major") => ScaleKind::Major,
        _ => {
            if auto_minor {
                ScaleKind::Minor
            } else {
                ScaleKind::Major
            }
        }
    }
}

fn build_midi_from_degrees(
    degrees: &[i32],
    root: i32,
    scale: ScaleKind,
    tempo: u32,
    vel: u8,
    dur_beats: f32,
) -> MonophonicMidi {
    let mut m = MonophonicMidi::new(tempo);
    let mut t = 0.0f32;
    for &d in degrees {
        let pitch = degree_to_midi(root, d, scale).clamp(0, 127) as u8;
        m.push(pitch, t, t + dur_beats, vel);
        t += dur_beats;
    }
    m
}

/* ---------- text -> audio ---------- */

fn text_to_audio(text: &str, opts: &TransformOpts) -> Result<(Vec<u8>, Vec<u8>)> {
    // Refolosim euristicile din text-features (sentiment & silabe)
    let tf = analyze_text(text)?;
    let s = tf.sentiment_score; // [-1, 1]
    let tempo = opts
        .tempo
        .unwrap_or_else(|| (90.0 + 30.0 * s.abs()).round() as u32);
    let auto_minor = s < 0.0;
    let scale = scale_from(opts, auto_minor);

    let jump = clamp01(opts.jumpiness.unwrap_or(0.35));
    let step_span = (1.0 + 6.0 * jump).round() as i32; // 1..7
    let bars = opts.bars.unwrap_or(2).max(1);
    // densitate: mai multe silabe => note mai scurte
    let dur_beats = (4.0 / (tf.syllables_total as f32 / 12.0 + 1.0)).clamp(0.25, 1.0);
    // ~4 beats per bar -> număr note
    let n = (bars as f32 * 4.0 / dur_beats).ceil() as usize;

    let mut degs = Vec::with_capacity(n);
    let mut cur = 0;
    for i in 0..n {
        let dir = if i % 4 == 0 {
            0
        } else {
            if (i as i32) & 1 == 0 {
                1
            } else {
                -1
            }
        };
        let step = dir * ((1 + (i as i32 % step_span)).min(step_span));
        cur += step;
        if cur > 9 {
            cur -= 7;
        }
        if cur < -9 {
            cur += 7;
        }
        degs.push(cur);
    }

    let root = opts.root_midi.unwrap_or(60); // C4
    let vel = (90.0 + 30.0 * s).clamp(40.0, 120.0) as u8;
    let midi = build_midi_from_degrees(&degs, root, scale, tempo, vel, dur_beats);
    let wav = render_wav_bytes(&midi, 44_100, osc_from(opts, None))?;
    let midi_json = serde_json::to_vec(&midi)?;
    Ok((midi_json, wav))
}

/* ---------- image -> audio ---------- */

fn image_to_audio(img_bytes: &[u8], opts: &TransformOpts) -> Result<(Vec<u8>, Vec<u8>)> {
    let ifeats = analyze_image_bytes(img_bytes)?;
    // hsv_mean_h in [0,360), s,v in [0,1]
    let h = ifeats.hsv_mean_h;
    let s = clamp01(ifeats.hsv_mean_s);
    let v = clamp01(ifeats.hsv_mean_v);

    let root = opts
        .root_midi
        .unwrap_or(48 + ((h / 360.0) * 24.0).round() as i32); // C3..B4 aprox
    let tempo = opts.tempo.unwrap_or((80.0 + 60.0 * v).round() as u32);
    let vel = (60.0 + 65.0 * v).clamp(30.0, 127.0) as u8;
    let auto_minor = v < 0.5 || s < 0.25;
    let scale = scale_from(opts, auto_minor);

    let osc = osc_from(opts, Some(s));
    let jump = clamp01(opts.jumpiness.unwrap_or(0.4));
    let span = (1.0 + 6.0 * jump).round() as i32;
    let bars = opts.bars.unwrap_or(2).max(1);

    // Pattern pe 1 bar (8 optimi), repetat pe nr. de bars.
    let base = [0, 2, 4, 6, 4, 2, 0, -3];
    let mut degs = Vec::with_capacity(base.len() * bars as usize);
    for b in 0..bars {
        for (i, d) in base.iter().enumerate() {
            let extra = if (b + i as u32) % 3 == 0 { span } else { 0 };
            degs.push(d + extra);
        }
    }

    let midi = build_midi_from_degrees(&degs, root, scale, tempo, vel, 0.5); // optimi
    let wav = render_wav_bytes(&midi, 44_100, osc)?;
    let midi_json = serde_json::to_vec(&midi)?;
    Ok((midi_json, wav))
}

/* ---------- feature routes ---------- */

fn audio_to_json(audio_b64: &str) -> Result<serde_json::Value> {
    let bytes = B64.decode(audio_b64)?;
    let (mono, sr) = decode_wav_to_mono_f32(&bytes)?;
    let fx = FeatureExtractor::new(22_050, 2048, 512);
    let feats = fx.analyze_mono(&mono, sr)?;
    Ok(serde_json::to_value(&feats)?)
}

fn text_to_json(text: &str) -> Result<serde_json::Value> {
    let feats = analyze_text(text)?;
    Ok(serde_json::to_value(&feats)?)
}

fn image_to_json(img_b64: &str) -> Result<serde_json::Value> {
    let bytes = B64.decode(img_b64)?;
    let feats = visual_features::analyze_image_bytes(&bytes)?;
    Ok(serde_json::to_value(&feats)?)
}

/* ---------- public entry ---------- */

pub fn handle_convert(req: ConvertRequest) -> Result<ConvertResponse> {
    let mut artifacts = Vec::new();

    match (req.from.as_str(), req.to.as_str(), &req.payload) {
        ("text", "audio", InputPayload::Text { text }) => {
            let (midi_json, wav) = text_to_audio(text, &req.options)?;
            artifacts.push(OutputArtifact::MidiJsonBase64 {
                data_b64: B64.encode(midi_json),
            });
            artifacts.push(OutputArtifact::WavBase64 {
                data_b64: B64.encode(wav),
            });
        }
        ("image", "audio", InputPayload::ImageBase64 { data_b64 }) => {
            let img = B64.decode(data_b64)?;
            let (midi_json, wav) = image_to_audio(&img, &req.options)?;
            artifacts.push(OutputArtifact::MidiJsonBase64 {
                data_b64: B64.encode(midi_json),
            });
            artifacts.push(OutputArtifact::WavBase64 {
                data_b64: B64.encode(wav),
            });
        }

        ("audio", "json", InputPayload::AudioBase64 { data_b64 }) => {
            let j = audio_to_json(data_b64)?;
            artifacts.push(OutputArtifact::Json { data: j });
        }
        ("text", "json", InputPayload::Text { text }) => {
            let j = text_to_json(text)?;
            artifacts.push(OutputArtifact::Json { data: j });
        }
        ("image", "json", InputPayload::ImageBase64 { data_b64 }) => {
            let j = image_to_json(data_b64)?;
            artifacts.push(OutputArtifact::Json { data: j });
        }

        // Unsupported combinations for now → echo a structured error
        _ => {
            artifacts.push(OutputArtifact::Json {
                data: serde_json::json!({
                    "error": "unsupported transformation",
                    "from": req.from,
                    "to": req.to
                }),
            });
        }
    }

    Ok(ConvertResponse { artifacts })
}

/* ---------- tiny handy builders (optional) ---------- */

/// Convenience builder for a text→audio request.
pub fn make_text_to_audio(text: &str, options: TransformOpts) -> ConvertRequest {
    ConvertRequest {
        from: "text".into(),
        to: "audio".into(),
        options,
        payload: InputPayload::Text {
            text: text.to_string(),
        },
    }
}

/// Convenience builder for an image→audio request (input bytes).
pub fn make_image_to_audio(img_bytes: &[u8], options: TransformOpts) -> ConvertRequest {
    ConvertRequest {
        from: "image".into(),
        to: "audio".into(),
        options,
        payload: InputPayload::ImageBase64 {
            data_b64: B64.encode(img_bytes),
        },
    }
}
