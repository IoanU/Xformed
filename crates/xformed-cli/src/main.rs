use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use clap::{Parser, Subcommand};
use converters::{handle_convert, ConvertRequest, InputPayload, OutputArtifact, TransformOpts};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

/// xformed-cli – zero-knobs: totul se deduce din conținut.
/// Comenzi:
///   - text-to-audio --text "..."     (sau text pe STDIN)
///   - image-to-audio --input path.png
///   - *-features (debug): audio/text/image → json
#[derive(Parser, Debug)]
#[command(name="xformed", version, about="Zero-knobs content-driven music")]
struct Cli {
    /// Directorul în care se scriu fișierele rezultate (wav/midi.json/json)
    #[arg(long, global=true, default_value="outputs")]
    out_dir: PathBuf,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generează audio din text (durata și stilul se deduc automat)
    TextToAudio {
        /// Textul; dacă lipsește, se citește din STDIN
        #[arg(long)]
        text: Option<String>,
    },

    /// Generează audio din imagine (durata ~ dimensiunea imaginii; fără loop)
    ImageToAudio {
        /// Calea către fișierul imagine (png/jpg)
        #[arg(long)]
        input: PathBuf,
    },

    /// (Debug) Extrage feature-uri dintr-un WAV
    AudioFeatures {
        #[arg(long)]
        input: PathBuf,
    },

    /// (Debug) Extrage feature-uri din text
    TextFeatures {
        /// Textul; dacă lipsește, se citește din STDIN
        #[arg(long)]
        text: Option<String>,
    },

    /// (Debug) Extrage feature-uri din imagine
    ImageFeatures {
        #[arg(long)]
        input: PathBuf,
    },
}

fn write_artifacts(out_dir: &PathBuf, prefix: &str, artifacts: &[OutputArtifact]) -> Result<()> {
    fs::create_dir_all(out_dir).ok();
    for (i, art) in artifacts.iter().enumerate() {
        match art {
            OutputArtifact::WavBase64 { data_b64 } => {
                let bytes = B64.decode(data_b64)?;
                let p = out_dir.join(format!("{}_{}.wav", prefix, i));
                fs::write(&p, bytes)?;
                eprintln!("✓ wrote {}", p.display());
            }
            OutputArtifact::MidiJsonBase64 { data_b64 } => {
                let bytes = B64.decode(data_b64)?;
                let p = out_dir.join(format!("{}_{}.midi.json", prefix, i));
                fs::write(&p, bytes)?;
                eprintln!("✓ wrote {}", p.display());
            }
            OutputArtifact::Json { data } => {
                let p = out_dir.join(format!("{}_{}.json", prefix, i));
                let pretty = serde_json::to_string_pretty(data)?;
                fs::write(&p, pretty)?;
                eprintln!("✓ wrote {}", p.display());
            }
        }
    }
    Ok(())
}

fn read_stdin_string() -> Result<String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).context("failed reading STDIN")?;
    Ok(buf)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // zero-knobs: deducem totul în `converters`; aici nu mai trimitem „creative knobs”
    let opts = TransformOpts::default();

    match cli.cmd {
        Commands::TextToAudio { text } => {
            let txt = match text {
                Some(t) => t,
                None => read_stdin_string()?,
            };
            let req = ConvertRequest {
                from: "text".into(),
                to: "audio".into(),
                options: opts,
                payload: InputPayload::Text { text: txt },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "out_from_text", &resp.artifacts)?;
        }

        Commands::ImageToAudio { input } => {
            let bytes = fs::read(&input)
                .with_context(|| format!("failed reading image: {}", input.display()))?;
            let req = ConvertRequest {
                from: "image".into(),
                to: "audio".into(),
                options: opts,
                payload: InputPayload::ImageBase64 { data_b64: B64.encode(bytes) },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "out_from_image", &resp.artifacts)?;
        }

        // ----- DEBUG ROUTES -----
        Commands::AudioFeatures { input } => {
            let bytes = fs::read(&input)
                .with_context(|| format!("failed reading audio: {}", input.display()))?;
            let req = ConvertRequest {
                from: "audio".into(),
                to: "json".into(),
                options: opts,
                payload: InputPayload::AudioBase64 { data_b64: B64.encode(bytes) },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "features_audio", &resp.artifacts)?;
        }

        Commands::TextFeatures { text } => {
            let txt = match text {
                Some(t) => t,
                None => read_stdin_string()?,
            };
            let req = ConvertRequest {
                from: "text".into(),
                to: "json".into(),
                options: opts,
                payload: InputPayload::Text { text: txt },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "features_text", &resp.artifacts)?;
        }

        Commands::ImageFeatures { input } => {
            let bytes = fs::read(&input)
                .with_context(|| format!("failed reading image: {}", input.display()))?;
            let req = ConvertRequest {
                from: "image".into(),
                to: "json".into(),
                options: opts,
                payload: InputPayload::ImageBase64 { data_b64: B64.encode(bytes) },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "features_image", &resp.artifacts)?;
        }
    }

    Ok(())
}
