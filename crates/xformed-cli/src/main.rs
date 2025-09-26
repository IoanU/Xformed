use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use clap::{Parser, Subcommand};
use converters::{handle_convert, ConvertRequest, InputPayload, OutputArtifact, TransformOpts};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// xformed-cli – zero-knobs: totul se deduce din conținut.
/// Comenzi:
///   - text-to-audio --text "..."     (sau text pe STDIN)
///   - image-to-audio --input path.png
///   - *-features (debug): audio/text/image → json
#[derive(Parser, Debug)]
#[command(name="xformed", version, about="Zero-knobs content-driven music")]
struct Cli {
    /// Directorul de ieșire (implicit: outputs/)
    #[arg(long, default_value = "outputs")]
    out_dir: PathBuf,

    /// Numele de bază pentru TOATE fișierele generate (fără extensie).
    /// Exemplu: --name matei  -> outputs/matei.wav, outputs/matei.midi.json, outputs/matei.json
    #[arg(long)]
    name: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Text → Audio (WAV + MIDI JSON)
    TextToAudio {
        /// Textul de intrare; dacă lipsește, citește din STDIN
        #[arg(long)]
        text: Option<String>,
    },

    /// Imagine → Audio (WAV + MIDI JSON)
    ImageToAudio {
        /// Calea către imagine (PNG/JPEG)
        #[arg(long)]
        input: PathBuf,
    },

    /// DEBUG: extrage JSON cu feature-uri din audio WAV
    AudioFeatures {
        #[arg(long)]
        input: PathBuf,
    },

    /// DEBUG: extrage JSON cu feature-uri din text
    TextFeatures {
        /// Text de intrare; dacă lipsește, citește din STDIN
        #[arg(long)]
        text: Option<String>,
    },

    /// DEBUG: extrage JSON cu feature-uri din imagine
    ImageFeatures {
        #[arg(long)]
        input: PathBuf,
    },
}

fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path).with_context(|| format!("cannot create dir {}", path.display()))?;
    }
    Ok(())
}

fn read_stdin_string() -> Result<String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

fn sanitize_basename(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

/// Scrie artefactele cu un `base_stem` implicit, dar dacă `name_override` e Some(..),
/// toate fișierele (WAV, .midi.json, .json) vor folosi acel stem.
fn write_artifacts(out_dir: &Path, base_stem: &str, name_override: Option<&str>, artifacts: &[OutputArtifact]) -> Result<()> {
    ensure_dir(out_dir)?;
    let stem = name_override.unwrap_or(base_stem);

    for art in artifacts {
        match art {
            OutputArtifact::WavBase64 { data_b64 } => {
                let bytes = B64.decode(data_b64).context("bad wav base64")?;
                let path = out_dir.join(format!("{stem}.wav"));
                fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
            }
            OutputArtifact::MidiJsonBase64 { data_b64 } => {
                let bytes = B64.decode(data_b64).context("bad midi-json base64")?;
                let path = out_dir.join(format!("{stem}.midi.json"));
                fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
            }
            OutputArtifact::Json { data } => {
                let path = out_dir.join(format!("{stem}.json"));
                let pretty = serde_json::to_vec_pretty(data)?;
                fs::write(&path, pretty).with_context(|| format!("write {}", path.display()))?;
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let name_override_clean = cli.name.as_ref().map(|s| sanitize_basename(s));
    let name_override_ref = name_override_clean.as_deref();

    match &cli.command {
        Commands::TextToAudio { text } => {
            let text_in = match text {
                Some(t) => t.clone(),
                None => read_stdin_string()?,
            };
            let req = ConvertRequest {
                from: "text".into(),
                to: "audio".into(),
                options: TransformOpts {
                    text_sec_per_word: None,
                    text_min_sec: None,
                    text_max_sec: None,
                    target_seconds: None,
                },
                payload: InputPayload::Text { text: text_in },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "out_from_text", name_override_ref, &resp.artifacts)?;
        }

        Commands::ImageToAudio { input } => {
            let bytes = fs::read(input).with_context(|| format!("failed reading image: {}", input.display()))?;
            let req = ConvertRequest {
                from: "image".into(),
                to: "audio".into(),
                options: TransformOpts {
                    text_sec_per_word: None,
                    text_min_sec: None,
                    text_max_sec: None,
                    target_seconds: None,
                },
                payload: InputPayload::ImageBase64 { data_b64: B64.encode(bytes) },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "out_from_image", name_override_ref, &resp.artifacts)?;
        }

        Commands::AudioFeatures { input } => {
            let bytes = fs::read(input).with_context(|| format!("failed reading audio: {}", input.display()))?;
            let req = ConvertRequest {
                from: "audio".into(),
                to: "json".into(),
                options: TransformOpts {
                    text_sec_per_word: None,
                    text_min_sec: None,
                    text_max_sec: None,
                    target_seconds: None,
                },
                payload: InputPayload::AudioBase64 { data_b64: B64.encode(bytes) },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "features_audio", name_override_ref, &resp.artifacts)?;
        }

        Commands::TextFeatures { text } => {
            let text_in = match text {
                Some(t) => t.clone(),
                None => read_stdin_string()?,
            };
            let req = ConvertRequest {
                from: "text".into(),
                to: "json".into(),
                options: TransformOpts {
                    text_sec_per_word: None,
                    text_min_sec: None,
                    text_max_sec: None,
                    target_seconds: None,
                },
                payload: InputPayload::Text { text: text_in },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "features_text", name_override_ref, &resp.artifacts)?;
        }

        Commands::ImageFeatures { input } => {
            let bytes = fs::read(input).with_context(|| format!("failed reading image: {}", input.display()))?;
            let req = ConvertRequest {
                from: "image".into(),
                to: "json".into(),
                options: TransformOpts {
                    text_sec_per_word: None,
                    text_min_sec: None,
                    text_max_sec: None,
                    target_seconds: None,
                },
                payload: InputPayload::ImageBase64 { data_b64: B64.encode(bytes) },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "features_image", name_override_ref, &resp.artifacts)?;
        }
    }

    Ok(())
}
