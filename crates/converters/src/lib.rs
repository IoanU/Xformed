//! converters - zero-knobs pipeline: content -> music
//! - Text -> Audio: duration from word count; style from text-features
//! - Image -> Audio: duration from rezolution; parsing without loop; style from image-features
//! - (optional) *-features rute for debug (audio/text/image -> json)

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};

use melody_core::{MonophonicMidi, ScaleKind, degree_to_midi};
use melody_synth::{Osc, StyleParams, render_wav_bytes_styled};

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
    /// Raw audio (WAV) base64 - used only for audio->json features
    AudioBase64 { data_b64: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConvertRequest {
    pub from: String, // "text" | "image" | "audio"
    pub to: String,   // "audio" | "json"
    pub options: TransformOpts,
    pub payload: InputPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutputArtifact {
    /// 16-bit PCM WAV, base64-encoded
    WavBase64 { data_b64: String },
    /// MIDI timeline as JSON, base64 (to preserve binary safety across transports)
    MidiJsonBase64 { data_b64: String },
    /// Generic JSON (features etc.)
    Json { data: serde_json::Value },
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ConvertResponse {
    pub artifacts: Vec<OutputArtifact>,
}

/// Zero-knobs options – only keep the operational controllers (not the creative ones).
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TransformOpts {
    /// (optional) only for text; scaling fallback
    pub text_sec_per_word: Option<f32>,  // default 0.30
    pub text_min_sec: Option<f32>,       // default 10
    pub text_max_sec: Option<f32>,       // default 180
    /// (optional) only for images; if missing, extracting from resolution
    pub target_seconds: Option<f32>,
}

/* ------------------------------------
   Entry point
-------------------------------------*/

pub fn handle_convert(req: ConvertRequest) -> Result<ConvertResponse> {
    match (&*req.from, &*req.to, &req.payload) {
        ("text", "audio", InputPayload::Text { text }) => {
            let (midi_json, wav) = text_to_audio(text, &req.options)?;
            Ok(ConvertResponse {
                artifacts: vec![
                    OutputArtifact::MidiJsonBase64 { data_b64: B64.encode(midi_json) },
                    OutputArtifact::WavBase64 { data_b64: B64.encode(wav) },
                ],
            })
        }
        ("image", "audio", InputPayload::ImageBase64 { data_b64 }) => {
            let bytes = B64.decode(data_b64).context("bad image base64")?;
            let (midi_json, wav) = image_to_audio(&bytes, &req.options)?;
            Ok(ConvertResponse {
                artifacts: vec![
                    OutputArtifact::MidiJsonBase64 { data_b64: B64.encode(midi_json) },
                    OutputArtifact::WavBase64 { data_b64: B64.encode(wav) },
                ],
            })
        }

        // Debug/analytics routes (optional)
        ("audio", "json", InputPayload::AudioBase64 { data_b64 }) => {
            let bytes = B64.decode(data_b64).context("bad audio base64")?;
            let (mono, sr) = audio_features::decode_wav_to_mono_f32(&bytes)?;

            // building the extractor (parameters ok by default)
            let fe = AudioFE::new(44_100, 2048, 512);

            // running analysis on your buffer and its real rate
            let feats = fe.analyze_mono(&mono, sr)?;
            Ok(ConvertResponse {
                artifacts: vec![OutputArtifact::Json { data: serde_json::to_value(feats)? }],
            })
        }
        ("text", "json", InputPayload::Text { text }) => {
            let tf = analyze_text(text)?;
            Ok(ConvertResponse {
                artifacts: vec![OutputArtifact::Json { data: serde_json::to_value(tf)? }],
            })
        }
        ("image", "json", InputPayload::ImageBase64 { data_b64 }) => {
            let bytes = B64.decode(data_b64).context("bad image base64")?;
            let ife = analyze_image_bytes(&bytes)?;
            Ok(ConvertResponse {
                artifacts: vec![OutputArtifact::Json { data: serde_json::to_value(ife)? }],
            })
        }

        _ => Err(anyhow!("unsupported conversion: {} -> {}", req.from, req.to)),
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
    layering: Vec<Osc>,
    polyphony: usize,   // 1..3
    swing: f32,         // 0..0.35
    humanize: f32,      // 0..0.4
    percussion: bool,
    jumpiness: f32,     // 0..1 (melodic leapiness)
}

fn clamp_range(x: f32, lo: f32, hi: f32) -> f32 { x.max(lo).min(hi) }

fn style_from_text(tf: &TextFeatures) -> AutoStyle {
    // tempo ^ with phonetic density
    let tempo = (95.0 + 35.0 * (tf.syllables_per_word - 1.0).clamp(0.0, 1.5)).round() as u32;
    let scale = if tf.sentiment_score < 0.0 { ScaleKind::Minor } else { ScaleKind::Major };
    let root_midi = 60;

    // lexical "richness" -> polyphony & layering
    let richness = ((tf.ttr + tf.word_entropy_bits / 5.0) / 2.0).clamp(0.0, 1.0);
    let polyphony = if richness > 0.7 { 3 } else if richness > 0.4 { 2 } else { 1 };
    let layering = if polyphony >= 3 {
        vec![Osc::Saw, Osc::Sine, Osc::Square]
    } else if polyphony == 2 {
        vec![Osc::Saw, Osc::Sine]
    } else {
        vec![Osc::Saw]
    };

    let swing = (tf.punctuation_ratio * 1.5).clamp(0.0, 0.30);
    let humanize = (0.15 + richness * 0.25).clamp(0.0, 0.4);
    let percussion = richness > 0.5;
    let jumpiness = (0.3 + tf.sentiment_score.abs() * 0.5).clamp(0.0, 1.0);

    AutoStyle { tempo, root_midi, scale, layering, polyphony, swing, humanize, percussion, jumpiness }
}

fn style_from_image(fe: &ImageFeatures) -> AutoStyle {
    let tempo = (80.0 + 60.0 * fe.hsv_mean_v).round() as u32;
    let root_midi = 48 + ((fe.hsv_mean_h / 360.0) * 24.0).round() as i32;
    let scale = if fe.hsv_mean_v < 0.5 || fe.hsv_mean_s < 0.25 { ScaleKind::Minor } else { ScaleKind::Major };

    let layering = if fe.hsv_mean_s < 0.3 {
        vec![Osc::Sine, Osc::Saw]          // soft
    } else if fe.hsv_mean_s < 0.6 {
        vec![Osc::Saw, Osc::Sine]          // med
    } else {
        vec![Osc::Saw, Osc::Square, Osc::Sine] // rich
    };

    let color_var = (fe.hue_variance / 90.0).clamp(0.0, 1.0);
    let polyphony = if color_var > 0.6 { 3 } else if color_var > 0.3 { 2 } else { 1 };

    let swing = (fe.edge_density * 0.4).clamp(0.0, 0.35);
    let humanize = (0.2 + fe.contrast_luma_std * 0.4).clamp(0.0, 0.4);
    let percussion = fe.edge_density > 0.12 || fe.contrast_luma_std > 0.15;
    let jumpiness = (0.25 + fe.hsv_mean_s * 0.6).clamp(0.0, 1.0);

    AutoStyle { tempo, root_midi, scale, layering, polyphony, swing, humanize, percussion, jumpiness }
}

/* ------------------------------------
   Text -> Audio (zero-knobs)
-------------------------------------*/

fn text_to_audio(text: &str, opts: &TransformOpts) -> Result<(Vec<u8>, Vec<u8>)> {
    let tf = analyze_text(text)?;
    let sty = style_from_text(&tf);

    // 1) target duration from text (zero-knobs)
    let spw = opts.text_sec_per_word.unwrap_or(0.50);
    let min_s = opts.text_min_sec.unwrap_or(10.0);
    let max_s = opts.text_max_sec.unwrap_or(180.0);
    let desired_seconds = clamp_range(6.0 + tf.n_words as f32 * spw, min_s, max_s);

    // 2) number of musical "events" (estimated)
    //    (keeping the random-walk idea, but using variations)
    let total_beats = desired_seconds * (sty.tempo as f32) / 60.0;
    let approx_note_len_beats = (4.0 / (tf.syllables_total as f32 / 12.0 + 1.0)).clamp(0.25, 1.0);
    let n_base = (total_beats / approx_note_len_beats).ceil().max(12.0) as usize;

    // 3) unit curve: random walk with "jumpiness" + small octave hops
    let step_span = (1.0 + 6.0 * sty.jumpiness).round() as i32; // 1..7
    let mut degs: Vec<i32> = Vec::with_capacity((n_base as f32 * 1.2) as usize);
    let mut cur = 0;
    for i in 0..n_base {
        let dir = if i % 4 == 0 { 0 } else if (i & 1) == 0 { 1 } else { -1 };
        let step = dir * ((1 + (i as i32 % step_span)).min(step_span));
        cur = (cur + step).clamp(-12, 12);

        // Ocasionally: octave jumps (up if the sentiment is positive and down if sentiment is negative)
        if i % 23 == 0 && sty.humanize > 0.1 {
            let oct = if tf.sentiment_score >= 0.0 { 12 } else { -12 };
            cur = (cur + oct).clamp(-12, 12);
        }
        degs.push(cur);

        // small motive turn in the beginning of the phrase (about every ~20 units)
        if i % 20 == 0 && i > 0 && sty.jumpiness > 0.35 {
            let a = (cur - 2).clamp(-12, 12);
            let b = cur;
            degs.push(a);
            degs.push(b);
        }
    }

    // 4) variable rhythms (small pauses and patterns) - like for the image
    //    choosing the pattern by the "punctuation_ratio" (more punctuation => more syncope)
    let rhythms: &[&[f32]] = &[
        &[0.5, 0.5, 0.5, 0.5],          // "straight" eighths
        &[0.25, 0.75, 0.5, 0.5],        // syncope ușoară
        &[0.75, 0.25, 0.5, 0.25, 0.25], // "push-pull"
    ];
    let sync_bias = (tf.punctuation_ratio * 10.0).round() as usize; // 0..~3
    let mut m = MonophonicMidi::new(sty.tempo);
    let mut t = 0.0f32;
    let mut rstep_idx = 0usize;

    // base velocity, influenced by sentiment
    let base_vel = (90.0 + 30.0 * tf.sentiment_score).clamp(40.0, 120.0) as u8;

    for (i, d) in degs.iter().enumerate() {
        let pat_idx = (sync_bias + i / 32) % rhythms.len();
        let pat = rhythms[pat_idx];
        let dur_beats = pat[rstep_idx % pat.len()];
        rstep_idx += 1;

        // small occasional pause (breathing)
        let is_rest = (i % 19 == 0) && (sty.humanize > 0.12);
        if !is_rest {
            let pitch = degree_to_midi(sty.root_midi, *d, sty.scale).clamp(0, 127) as u8;
            // small accents: once every 8 events, hit a little harder
            let vel = if i % 8 == 0 { (base_vel as i32 + 10).clamp(1, 127) as u8 } else { base_vel };
            m.push(pitch, t, t + dur_beats, vel);
        }
        t += dur_beats;

        // finish if we had reached the beat count target (protection for inserted motives)
        if t >= total_beats { break; }
    }

    // 5) serious rendering (layering, poly, swing, humanize, percussion)
    let wav = render_wav_bytes_styled(&m, 44_100, &StyleParams {
        layering: sty.layering,
        swing: sty.swing,
        humanize: sty.humanize,
        polyphony: sty.polyphony,
        percussion: sty.percussion,
        scale: sty.scale,
    })?;
    let midi_json = serde_json::to_vec(&m)?;
    Ok((midi_json, wav))
}

/* ------------------------------------
   Image -> Audio (zero-knobs, no loop)
-------------------------------------*/

fn image_to_audio(img_bytes: &[u8], _opts: &TransformOpts) -> Result<(Vec<u8>, Vec<u8>)> {
    use image::{GenericImageView};
    use palette::{Srgb, IntoColor, Hsv};

    // 1) Load & basic dims
    let img = image::load_from_memory(img_bytes)?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 { return Err(anyhow!("empty image")); }

    // 2) Global features -> style
    let ife = analyze_image_bytes(img_bytes)?;
    let sty = style_from_image(&ife);

    // 3) Rezolution duration: #tiles ~ area/(300x300) clamped 250..1500
    let cells_target = ((w as f32 * h as f32) / (380.0 * 380.0)).clamp(180.0, 950.0);
    let aspect = w as f32 / h.max(1) as f32;
    let cols = (cells_target.sqrt() * aspect.sqrt()).round().clamp(16.0, 96.0) as u32;
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
            let mut sh=0.0; let mut ss=0.0; let mut sv=0.0; let mut cnt=0.0;
            let mut yy=y0; while yy<y1 {
                let mut xx=x0; while xx<x1 {
                    let p = rgb.get_pixel(xx, yy);
                    let (r8,g8,b8) = (p[0], p[1], p[2]);
                    let (r,g,b) = (r8 as f32/255.0, g8 as f32/255.0, b8 as f32/255.0);
                    let hsv: Hsv = Srgb::new(r,g,b).into_color();
                    sh+=hsv.hue.into_degrees(); ss+=hsv.saturation; sv+=hsv.value; cnt+=1.0;
                    xx = xx.saturating_add(4);
                }
                yy = yy.saturating_add(4);
            }
            let (mh, ms, mv) = if cnt>0.0 { (sh/cnt, ss/cnt, sv/cnt) } else { (base_h, base_s, base_v) };

            // mapping: hue diff -> step size, saturation -> extra salt, value -> velocity
            let dh = (mh - base_h).abs();
            let hue_push = ((dh / 180.0) * span as f32).round() as i32;
            let salt = if ms < 0.2 { 0 } else if ms < 0.5 { 1 } else { 2 };
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

    // 6) Serious rendering with everything
    let wav = render_wav_bytes_styled(&m, 44_100, &StyleParams {
        layering: sty.layering,
        swing: sty.swing,
        humanize: sty.humanize,
        polyphony: sty.polyphony,
        percussion: sty.percussion,
        scale: sty.scale,
    })?;
    let midi_json = serde_json::to_vec(&m)?;
    Ok((midi_json, wav))
}
