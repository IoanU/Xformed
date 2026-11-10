mod cross_modal;
mod fractal;
mod mood;

use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};

use melody_core::{degree_to_midi, Composition, MonophonicMidi, ScaleKind};
use melody_synth::{arrange_seeded, render_composition};

/// External feature extractors (must be provided by sibling crates)
use audio_features::FeatureExtractor as AudioFE;
use text_features::{analyze_text, TextFeatures};
use visual_features::{analyze_image_bytes, ImageFeatures};

/// Public request/response types used by CLI and any service layer.

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum InputPayload {
    /// Plain text (UTF-8)
    Text { text: String },
    /// Raw image, base64-encoded (PNG/JPEG etc.)
    ImageBase64 { data_b64: String },
    /// Encoded audio (WAV, MP3, FLAC, OGG or supported M4A), base64-encoded.
    AudioBase64 { data_b64: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConvertRequest {
    pub from: String, // "text" | "image" | "audio"
    pub to: String,   // "text" | "image" | "audio" | "json"
    #[serde(default)]
    pub options: TransformOpts,
    pub payload: InputPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutputArtifact {
    CreativeInfo {
        title: String,
        mood: String,
        description: String,
        seed: String,
    },
    PngBase64 {
        data_b64: String,
    },
    Text {
        text: String,
    },
    /// 16-bit PCM WAV, base64-encoded
    WavBase64 {
        data_b64: String,
    },
    MidiBase64 {
        data_b64: String,
    },
    /// MIDI timeline as JSON, base64 (to preserve binary safety across transports)
    MidiJsonBase64 {
        data_b64: String,
    },
    /// Generic JSON (features etc.)
    Json {
        data: serde_json::Value,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ConvertResponse {
    pub artifacts: Vec<OutputArtifact>,
}

/// Reproducible creative choices and duration controls shared by every runtime.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TransformOpts {
    #[serde(default)]
    pub variation: u32,
    pub image_style: Option<String>,
    pub text_style: Option<String>,
    /// (optional) only for text; scaling fallback
    pub text_sec_per_word: Option<f32>, // default 0.50
    pub text_min_sec: Option<f32>, // default 10
    pub text_max_sec: Option<f32>, // default 180
    /// Musical duration override for text/image -> audio
    pub target_seconds: Option<f32>,
}

/* ------------------------------------
   Entry point
-------------------------------------*/

pub fn handle_convert(req: ConvertRequest) -> Result<ConvertResponse> {
    validate_request(&req)?;
    match (&*req.from, &*req.to, &req.payload) {
        ("text", "audio", InputPayload::Text { text }) => {
            let (midi_json, wav) = text_to_audio(text, &req.options)?;
            Ok(ConvertResponse {
                artifacts: vec![
                    OutputArtifact::MidiBase64 {
                        data_b64: B64.encode(
                            serde_json::from_slice::<Composition>(&midi_json)?.to_mid_bytes()?,
                        ),
                    },
                    OutputArtifact::MidiJsonBase64 {
                        data_b64: B64.encode(midi_json),
                    },
                    OutputArtifact::WavBase64 {
                        data_b64: B64.encode(wav),
                    },
                ],
            })
        }
        ("image", "audio", InputPayload::ImageBase64 { data_b64 }) => {
            let bytes = B64.decode(data_b64).context("bad image base64")?;
            let (midi_json, wav) = image_to_audio(&bytes, &req.options)?;
            Ok(ConvertResponse {
                artifacts: vec![
                    OutputArtifact::MidiBase64 {
                        data_b64: B64.encode(
                            serde_json::from_slice::<Composition>(&midi_json)?.to_mid_bytes()?,
                        ),
                    },
                    OutputArtifact::MidiJsonBase64 {
                        data_b64: B64.encode(midi_json),
                    },
                    OutputArtifact::WavBase64 {
                        data_b64: B64.encode(wav),
                    },
                ],
            })
        }

        ("text", "image", InputPayload::Text { text }) => {
            cross_modal::text_to_image(text, &req.options)
        }
        ("image", "text", InputPayload::ImageBase64 { data_b64 }) => cross_modal::image_to_text(
            &B64.decode(data_b64).context("bad image base64")?,
            &req.options,
        ),
        ("audio", "image", InputPayload::AudioBase64 { data_b64 }) => {
            let (mono, sr) = decode_audio(data_b64)?;
            cross_modal::audio_to_image(&mono, sr, &req.options)
        }
        ("audio", "text", InputPayload::AudioBase64 { data_b64 }) => {
            let (mono, sr) = decode_audio(data_b64)?;
            cross_modal::audio_to_text(&mono, sr, &req.options)
        }
        // Debug/analytics routes (optional)
        ("audio", "json", InputPayload::AudioBase64 { data_b64 }) => {
            let (mono, sr) = decode_audio(data_b64)?;

            // building the extractor (parameters ok by default)
            let fe = AudioFE::new(44_100, 2048, 512);

            // running analysis on your buffer and its real rate
            let feats = fe.analyze_mono(&mono, sr)?;
            Ok(ConvertResponse {
                artifacts: vec![OutputArtifact::Json {
                    data: serde_json::to_value(feats)?,
                }],
            })
        }
        ("text", "json", InputPayload::Text { text }) => {
            let tf = analyze_text(text)?;
            Ok(ConvertResponse {
                artifacts: vec![OutputArtifact::Json {
                    data: serde_json::to_value(tf)?,
                }],
            })
        }
        ("image", "json", InputPayload::ImageBase64 { data_b64 }) => {
            let bytes = B64.decode(data_b64).context("bad image base64")?;
            let ife = analyze_image_bytes(&bytes)?;
            Ok(ConvertResponse {
                artifacts: vec![OutputArtifact::Json {
                    data: serde_json::to_value(ife)?,
                }],
            })
        }

        _ => Err(anyhow!(
            "unsupported conversion: {} -> {}",
            req.from,
            req.to
        )),
    }
}

/* ------------------------------------
   Style deduction (auto)
-------------------------------------*/

#[derive(Clone, Debug)]
struct AutoStyle {
    tempo: u32,
    root_midi: i32,
    scale: ScaleKind,
    swing: f32,     // 0..0.35
    humanize: f32,  // 0..0.4
    jumpiness: f32, // 0..1 (melodic leapiness)
}

fn clamp_range(x: f32, lo: f32, hi: f32) -> f32 {
    x.max(lo).min(hi)
}

fn style_from_text(tf: &TextFeatures) -> AutoStyle {
    // tempo ^ with phonetic density
    let tempo = (95.0 + 35.0 * (tf.syllables_per_word - 1.0).clamp(0.0, 1.5)).round() as u32;
    let scale = if tf.sentiment_score < 0.0 {
        ScaleKind::Minor
    } else {
        ScaleKind::Major
    };
    let root_midi = 60;

    // lexical "richness" -> polyphony & layering
    let richness = ((tf.ttr + tf.word_entropy_bits) / 2.0).clamp(0.0, 1.0);
    let swing = (tf.punctuation_ratio * 1.5).clamp(0.0, 0.30);
    let humanize = (0.15 + richness * 0.25).clamp(0.0, 0.4);
    let jumpiness = (0.3 + tf.sentiment_score.abs() * 0.5).clamp(0.0, 1.0);

    AutoStyle {
        tempo,
        root_midi,
        scale,
        swing,
        humanize,
        jumpiness,
    }
}

fn style_from_image(fe: &ImageFeatures) -> AutoStyle {
    let tempo = (80.0 + 60.0 * fe.hsv_mean_v).round() as u32;
    let root_midi = 48 + ((fe.hsv_mean_h / 360.0) * 24.0).round() as i32;
    let scale = if fe.hsv_mean_v < 0.5 || fe.hsv_mean_s < 0.25 {
        ScaleKind::Minor
    } else {
        ScaleKind::Major
    };

    let swing = (fe.edge_density * 0.4).clamp(0.0, 0.35);
    let humanize = (0.2 + fe.contrast_luma_std * 0.4).clamp(0.0, 0.4);
    let jumpiness = (0.25 + fe.hsv_mean_s * 0.6).clamp(0.0, 1.0);

    AutoStyle {
        tempo,
        root_midi,
        scale,
        swing,
        humanize,
        jumpiness,
    }
}

/* ------------------------------------
   Text -> Audio (zero-knobs)
-------------------------------------*/

fn text_to_audio(text: &str, opts: &TransformOpts) -> Result<(Vec<u8>, Vec<u8>)> {
    let tf = analyze_text(text)?;
    let atmosphere = mood::text_atmosphere(text);
    let sty = style_from_text(&tf);
    let seconds = opts.target_seconds.unwrap_or_else(|| {
        clamp_range(
            6.0 + tf.n_words as f32 * opts.text_sec_per_word.unwrap_or(0.5),
            opts.text_min_sec.unwrap_or(10.0),
            opts.text_max_sec.unwrap_or(180.0),
        )
    });
    let source_seed = mood::hash(text.as_bytes());
    let seed = if opts.variation == 0 {
        source_seed
    } else {
        mood::mixed(source_seed ^ opts.variation as u64)
    };
    let scale = if atmosphere.valence < -0.1 {
        ScaleKind::Minor
    } else {
        ScaleKind::Major
    };
    let root = 48 + (source_seed % 12) as i32;
    let words: Vec<_> = text.split_whitespace().collect();
    let count = words.len().clamp(12, 512);
    let mut m = MonophonicMidi::new(sty.tempo);
    let mut t = 0.0;
    for i in 0..count {
        let word = words[(i * words.len() / count).min(words.len() - 1)];
        let h = mood::hash(word.to_lowercase().as_bytes());
        let d = (h % 11) as i32 - 3;
        let length = [0.25, 0.5, 0.75, 1.0][(word.chars().count() + i % 3) % 4];
        let pause = if word.ends_with(['.', '!', '?', ';', ',']) {
            0.25
        } else {
            0.0
        };
        m.push(
            degree_to_midi(root + 12, d, scale).clamp(0, 127) as u8,
            t,
            t + length,
            65 + (h % 41) as u8,
        );
        t += length + pause;
    }
    let end = m.notes.last().unwrap().end;
    let stretch = seconds * sty.tempo as f32 / 60.0 / end;
    for n in &mut m.notes {
        n.start *= stretch;
        n.end *= stretch;
    }
    let score = arrange_seeded(&m, scale, root, atmosphere.energy, sty.swing, seed)?;
    let wav = render_composition(&score, 44100)?;
    Ok((serde_json::to_vec(&score)?, wav))
}

fn image_to_audio(img_bytes: &[u8], opts: &TransformOpts) -> Result<(Vec<u8>, Vec<u8>)> {
    use image::GenericImageView;
    use palette::{Hsv, IntoColor, Srgb};

    // 1) Load & basic dims
    let img = visual_features::load_image_bounded(img_bytes)?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err(anyhow!("empty image"));
    }

    // 2) Global features -> style
    let ife = visual_features::analyze_image(&img)?;
    let sty = style_from_image(&ife);

    // 3) Rezolution duration: #tiles ~ area/(300x300) clamped 250..1500
    let cells_target = ((w as f32 * h as f32) / (380.0 * 380.0)).clamp(180.0, 950.0);
    let aspect = w as f32 / h.max(1) as f32;
    let cols = (cells_target.sqrt() * aspect.sqrt())
        .round()
        .clamp(16.0, 96.0) as u32;
    let rows = ((cells_target / cols as f32).round()).clamp(12.0, 96.0) as u32;
    let tile_w = (w as f32 / cols as f32).ceil().max(1.0) as u32;
    let tile_h = (h as f32 / rows as f32).ceil().max(1.0) as u32;

    // 4) Parsing without loop (boustrophedon) + local mapping HSV -> note
    let rgb = img.to_rgb8();
    let total_notes = (cols * rows) as usize;
    let mut degs = Vec::with_capacity(total_notes);
    let mut vels = Vec::with_capacity(total_notes);

    let base_h = ife.hsv_mean_h;
    let base_s = ife.hsv_mean_s;
    let base_v = ife.hsv_mean_v;
    let span = (1.0 + 6.0 * sty.jumpiness).round() as i32;

    let mut cur_degree = 0i32;

    for r in 0..rows {
        let y0 = (r * tile_h).min(h.saturating_sub(1));
        let y1 = ((r + 1) * tile_h).min(h);

        let (start_c, end_c, step_col) = if r % 2 == 0 {
            (0u32, cols, 1i32)
        } else {
            (cols - 1, u32::MAX, -1i32)
        };

        let mut c = start_c as i32;
        while c != end_c as i32 {
            let cc = c as u32;
            let x0 = (cc * tile_w).min(w.saturating_sub(1));
            let x1 = ((cc + 1) * tile_w).min(w);

            // subsampling 4x4 px - HSV average
            let mut sh = 0.0;
            let mut ss = 0.0;
            let mut sv = 0.0;
            let mut cnt = 0.0;
            let mut yy = y0;
            while yy < y1 {
                let mut xx = x0;
                while xx < x1 {
                    let p = rgb.get_pixel(xx, yy);
                    let (r8, g8, b8) = (p[0], p[1], p[2]);
                    let (r, g, b) = (r8 as f32 / 255.0, g8 as f32 / 255.0, b8 as f32 / 255.0);
                    let hsv: Hsv = Srgb::new(r, g, b).into_color();
                    sh += hsv.hue.into_degrees();
                    ss += hsv.saturation;
                    sv += hsv.value;
                    cnt += 1.0;
                    xx = xx.saturating_add(4);
                }
                yy = yy.saturating_add(4);
            }
            let (mh, ms, mv) = if cnt > 0.0 {
                (sh / cnt, ss / cnt, sv / cnt)
            } else {
                (base_h, base_s, base_v)
            };

            // mapping: hue diff -> step size, saturation -> extra salt, value -> velocity
            let dh = (mh - base_h).abs();
            let hue_push = ((dh / 180.0) * span as f32).round() as i32;
            let salt = if ms < 0.2 {
                0
            } else if ms < 0.5 {
                1
            } else {
                2
            };
            let step_deg = (hue_push.min(span) + salt).max(0);

            let dir = if (r + cc) % 2 == 0 { 1 } else { -1 };
            cur_degree = (cur_degree + dir * step_deg).clamp(-12, 12);

            // small occasional transposition for relief (without exiting the ±12 range)
            if (cc + r) % 37 == 0 && sty.humanize > 0.1 {
                cur_degree = (cur_degree + if base_v > 0.5 { 12 } else { -12 }).clamp(-12, 12);
            }

            // "motivic turn" every corner passing on even rows
            if cc == 0 && (r % 2 == 0) && sty.jumpiness > 0.4 {
                // insert 2 short bonus notes (used later for variable rhythms)
                degs.push((cur_degree - 2).clamp(-12, 12));
                vels.push((vels.last().copied().unwrap_or(80) as i32 + 6).clamp(30, 127) as u8);
                degs.push((cur_degree).clamp(-12, 12));
                vels.push((vels.last().copied().unwrap_or(80) as i32 - 4).clamp(30, 127) as u8);
            }

            let vel = (50.0 + 70.0 * mv).clamp(30.0, 127.0) as u8;

            degs.push(cur_degree);
            vels.push(vel);

            c += step_col;
        }
    }

    // 5) Building MIDI: note per tile, without pattern loop. Duration per note = 0.5 beat (eighth).
    let rhythms: &[&[f32]] = &[
        &[0.5, 0.5, 0.5, 0.5],          // "straight" eighths
        &[0.25, 0.75, 0.5, 0.5],        // light syncope
        &[0.75, 0.25, 0.5, 0.25, 0.25], // "push-pull"
    ];
    let mut m = MonophonicMidi::new(sty.tempo);
    let mut t = 0.0f32;
    let mut rpat_idx;
    let mut rstep_idx = 0usize;

    for (i, d) in degs.iter().enumerate() {
        let pitch = degree_to_midi(sty.root_midi, *d, sty.scale).clamp(0, 127) as u8;
        let vel = vels[i];

        // choose pattern by image "agitation" (edge_density) + progress
        rpat_idx = ((sty.swing * 10.0) as usize + (i / 32)) % rhythms.len();
        let pat = rhythms[rpat_idx];

        let dur_beats = pat[rstep_idx % pat.len()];
        rstep_idx += 1;

        // 5–10% chance of "resting": dropping a note to breathe
        let is_rest = (i % 17 == 0) && (sty.humanize > 0.15);
        if !is_rest {
            m.push(pitch, t, t + dur_beats, vel);
        }
        t += dur_beats;
    }

    // Bound automatic image duration as well as explicit overrides (including panoramic inputs).
    let seconds = opts
        .target_seconds
        .unwrap_or_else(|| (t * 60.0 / sty.tempo as f32).clamp(10.0, 180.0));
    let scale = seconds * sty.tempo as f32 / 60.0 / t;
    for note in &mut m.notes {
        note.start *= scale;
        note.end *= scale;
    }
    // 6) Serious rendering with everything
    let raw_seed = mood::hash(img.thumbnail(64, 64).as_bytes());
    let seed = if opts.variation == 0 {
        raw_seed
    } else {
        mood::mixed(raw_seed ^ opts.variation as u64)
    };
    let score = arrange_seeded(&m, sty.scale, sty.root_midi, sty.jumpiness, sty.swing, seed)?;
    let wav = render_composition(&score, 44_100)?;
    let midi_json = serde_json::to_vec(&score)?;
    Ok((midi_json, wav))
}

fn validate_request(req: &ConvertRequest) -> Result<()> {
    if let Some(style) = &req.options.image_style {
        anyhow::ensure!(
            ["nebula", "contour", "bloom"].contains(&style.as_str()),
            "unknown image style"
        );
    }
    if let Some(style) = &req.options.text_style {
        anyhow::ensure!(
            ["poem", "vignette"].contains(&style.as_str()),
            "unknown writing style"
        );
    }
    for value in [
        req.options.target_seconds,
        req.options.text_min_sec,
        req.options.text_max_sec,
    ]
    .into_iter()
    .flatten()
    {
        anyhow::ensure!(
            value.is_finite() && (1.0..=180.0).contains(&value),
            "duration must be between 1 and 180 seconds"
        );
    }
    if let Some(value) = req.options.text_sec_per_word {
        anyhow::ensure!(
            value.is_finite() && (0.01..=10.0).contains(&value),
            "seconds per word must be between 0.01 and 10"
        );
    }
    anyhow::ensure!(
        req.options.text_min_sec.unwrap_or(10.0) <= req.options.text_max_sec.unwrap_or(180.0),
        "minimum duration exceeds maximum"
    );
    match &req.payload {
        InputPayload::Text { text } => {
            anyhow::ensure!(!text.trim().is_empty(), "enter some text first");
            anyhow::ensure!(text.len() <= 100_000, "text exceeds 100 KB");
        }
        InputPayload::ImageBase64 { data_b64 } | InputPayload::AudioBase64 { data_b64 } => {
            anyhow::ensure!(data_b64.len() <= 16_000_000, "file exceeds upload limit");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(text: &str) -> ConvertRequest {
        ConvertRequest {
            from: "text".into(),
            to: "audio".into(),
            options: TransformOpts::default(),
            payload: InputPayload::Text { text: text.into() },
        }
    }
    #[test]
    fn rejects_empty_and_invalid_durations() {
        assert!(handle_convert(request("  ")).is_err());
        for duration in [0.0, -1.0, f32::NAN, f32::INFINITY, 181.0] {
            let mut req = request("hello");
            req.options.target_seconds = Some(duration);
            assert!(handle_convert(req).is_err());
        }
    }
    #[test]
    fn image_duration_is_honored() {
        let (_, wav) = image_to_audio(
            include_bytes!("../../../inputs/images.png"),
            &TransformOpts {
                target_seconds: Some(3.0),
                ..Default::default()
            },
        )
        .unwrap();
        let seconds = (wav.len() - 44) as f32 / 176400.0;
        assert!((3.0..4.1).contains(&seconds), "{seconds}");
    }
    #[test]
    fn duration_and_midi_export() {
        let mut req = request("A bright happy morning full of hope");
        req.options.target_seconds = Some(2.0);
        let response = handle_convert(req).unwrap();
        assert!(response.artifacts.iter().any(|a| matches!(a, OutputArtifact::MidiBase64 { data_b64 } if B64.decode(data_b64).unwrap().starts_with(b"MThd"))));
        let wav = response
            .artifacts
            .iter()
            .find_map(|a| {
                if let OutputArtifact::WavBase64 { data_b64 } = a {
                    Some(B64.decode(data_b64).unwrap())
                } else {
                    None
                }
            })
            .unwrap();
        let seconds = (wav.len() - 44) as f32 / 176400.0;
        assert!((2.0..3.1).contains(&seconds), "{seconds}");
    }
}

fn decode_audio(encoded: &str) -> Result<(Vec<f32>, u32)> {
    let bytes = B64.decode(encoded).context("bad audio base64")?;
    let (mono, sr) = audio_features::decode::decode_audio_to_mono_f32(&bytes)?;
    anyhow::ensure!(
        mono.len() as f64 / sr as f64 <= 180.0,
        "recording exceeds 180 seconds"
    );
    Ok((mono, sr))
}

#[cfg(test)]
mod cross_modal_tests {
    use super::*;
    fn run(from: &str, to: &str, payload: InputPayload) -> ConvertResponse {
        handle_convert(ConvertRequest {
            from: from.into(),
            to: to.into(),
            payload,
            options: TransformOpts {
                target_seconds: Some(3.0),
                ..Default::default()
            },
        })
        .unwrap()
    }
    fn png(response: &ConvertResponse) -> Vec<u8> {
        response
            .artifacts
            .iter()
            .find_map(|a| {
                if let OutputArtifact::PngBase64 { data_b64 } = a {
                    Some(B64.decode(data_b64).unwrap())
                } else {
                    None
                }
            })
            .unwrap()
    }
    #[test]
    fn visual_poems_are_decodable_deterministic_and_source_sensitive() {
        let text = || InputPayload::Text {
            text: "A bright morning of hope".into(),
        };
        let a = png(&run("text", "image", text()));
        let b = png(&run("text", "image", text()));
        let c = png(&run(
            "text",
            "image",
            InputPayload::Text {
                text: "Dark rain against the glass".into(),
            },
        ));
        assert_eq!(a, b);
        assert_ne!(a, c);
        let img = image::load_from_memory(&a).unwrap();
        assert_eq!((img.width(), img.height()), (1280, 832));
    }
    #[test]
    fn image_text_uses_features_and_discloses_limits() {
        let result = run(
            "image",
            "text",
            InputPayload::ImageBase64 {
                data_b64: B64.encode(include_bytes!("../../../inputs/images.png")),
            },
        );
        assert!(result.artifacts.iter().any(|a|matches!(a,OutputArtifact::Text {text} if text.len()>150&&!text.contains("pixels")&&!text.contains("Hz"))));
    }
    #[test]
    fn audio_can_become_image_and_text_and_rejects_garbage() {
        let source = run(
            "text",
            "audio",
            InputPayload::Text {
                text: "A beautiful bright world".into(),
            },
        );
        let encoded = source
            .artifacts
            .iter()
            .find_map(|a| {
                if let OutputArtifact::WavBase64 { data_b64 } = a {
                    Some(data_b64.clone())
                } else {
                    None
                }
            })
            .unwrap();
        let payload = || InputPayload::AudioBase64 {
            data_b64: encoded.clone(),
        };
        let bytes = png(&run("audio", "image", payload()));
        assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 1280);
        assert!(run("audio", "text", payload()).artifacts.iter().any(
            |a| matches!(a,OutputArtifact::Text {text} if text.len()>150&&!text.contains("RMS"))
        ));
        for to in ["text", "image", "json"] {
            assert!(handle_convert(ConvertRequest {
                from: "audio".into(),
                to: to.into(),
                options: Default::default(),
                payload: InputPayload::AudioBase64 {
                    data_b64: "invalid".into()
                }
            })
            .is_err());
        }
    }
}

#[cfg(test)]
mod creative_regressions {
    use super::*;
    fn response(to: &str, opts: TransformOpts) -> ConvertResponse {
        handle_convert(ConvertRequest {
            from: "text".into(),
            to: to.into(),
            options: opts,
            payload: InputPayload::Text {
                text:
                    "I miss the warmth of a voice that no longer calls. The empty room remembers."
                        .into(),
            },
        })
        .unwrap()
    }
    #[test]
    fn longing_is_not_serene() {
        let a = mood::text_atmosphere(
            "I miss the warmth of a voice that no longer calls. The empty room remembers.",
        );
        assert_eq!(a.mood(), mood::Mood::Wistful);
    }
    #[test]
    fn creative_results_have_no_technical_analysis() {
        let r = response("image", TransformOpts::default());
        assert!(r
            .artifacts
            .iter()
            .any(|a| matches!(a, OutputArtifact::CreativeInfo { .. })));
        assert!(!r
            .artifacts
            .iter()
            .any(|a| matches!(a, OutputArtifact::Json { .. })));
    }
    #[test]
    fn variants_and_styles_are_distinct() {
        let a = response("image", TransformOpts::default());
        let b = response(
            "image",
            TransformOpts {
                variation: 1,
                ..Default::default()
            },
        );
        let c = response(
            "image",
            TransformOpts {
                image_style: Some("bloom".into()),
                ..Default::default()
            },
        );
        let png = |r: ConvertResponse| {
            r.artifacts
                .into_iter()
                .find_map(|a| {
                    if let OutputArtifact::PngBase64 { data_b64 } = a {
                        Some(data_b64)
                    } else {
                        None
                    }
                })
                .unwrap()
        };
        let a = png(a);
        assert_ne!(a, png(b));
        assert_ne!(a, png(c));
    }
    #[test]
    fn invalid_creative_controls_fail_cleanly() {
        assert!(handle_convert(ConvertRequest {
            from: "text".into(),
            to: "image".into(),
            options: TransformOpts {
                image_style: Some("unknown".into()),
                ..Default::default()
            },
            payload: InputPayload::Text {
                text: "hello".into()
            }
        })
        .is_err());
    }
}
