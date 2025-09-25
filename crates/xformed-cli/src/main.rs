use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use clap::{Parser, Subcommand};
use converters::{
    handle_convert, ConvertRequest, InputPayload, OutputArtifact, TransformOpts,
};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

/// Xformed CLI – run conversions without HTTP.
#[derive(Parser, Debug)]
#[command(name="xformed", version, about="Xformed CLI – text/image/audio conversions & feature dumps")]
struct Cli {
    /// Output directory (default: ./outputs)
    #[arg(long, global=true, default_value="outputs")]
    out_dir: PathBuf,

    /// Instrument: sine|saw|square|auto
    #[arg(long, global=true)]
    instrument: Option<String>,

    /// Tempo BPM (auto if omitted)
    #[arg(long, global=true)]
    tempo: Option<u32>,

    /// Mood: major|minor|auto
    #[arg(long, global=true, default_value="auto")]
    mood: String,

    /// Jumpiness 0..1 (interval leaps)
    #[arg(long, global=true)]
    jumpiness: Option<f32>,

    /// Root MIDI note (e.g., 60 = C4). If omitted, auto.
    #[arg(long, global=true)]
    root_midi: Option<i32>,

    /// Bars (approximate length). Default: 2
    #[arg(long, global=true)]
    bars: Option<u32>,

    /// Seed (reserved – for future deterministic variation)
    #[arg(long, global=true)]
    seed: Option<u64>,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Convert free-typed text into audio (melody)
    TextToAudio {
        /// Text to transform. If omitted, read from STDIN.
        #[arg(long)]
        text: Option<String>,
    },

    /// Convert an image file into audio
    ImageToAudio {
        #[arg(long)]
        input: PathBuf,
    },

    /// Analyze audio file and dump JSON features
    AudioFeatures {
        #[arg(long)]
        input: PathBuf,
    },

    /// Analyze text and dump JSON features
    TextFeatures {
        #[arg(long)]
        text: Option<String>,
    },

    /// Analyze image file and dump JSON features
    ImageFeatures {
        #[arg(long)]
        input: PathBuf,
    },
}

fn opts_from_cli(cli: &Cli) -> TransformOpts {
    TransformOpts {
        instrument: cli.instrument.clone(),
        tempo: cli.tempo,
        mood: Some(cli.mood.clone()),
        jumpiness: cli.jumpiness,
        root_midi: cli.root_midi,
        bars: cli.bars,
        seed: cli.seed,
    }
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
                fs::write(&p, serde_json::to_string_pretty(data)?)?;
                eprintln!("✓ wrote {}", p.display());
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let opts = opts_from_cli(&cli);

    match cli.cmd {
        Commands::TextToAudio { text } => {
            let txt = match text {
                Some(t) => t,
                None => {
                    let mut buf = String::new();
                    io::stdin()
                        .read_to_string(&mut buf)
                        .context("failed reading STDIN")?;
                    buf
                }
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
                .with_context(|| format!("failed reading {:?}", input.display()))?;
            let req = ConvertRequest {
                from: "image".into(),
                to: "audio".into(),
                options: opts,
                payload: InputPayload::ImageBase64 {
                    data_b64: B64.encode(bytes),
                },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "out_from_image", &resp.artifacts)?;
        }

        Commands::AudioFeatures { input } => {
            let bytes = fs::read(&input)
                .with_context(|| format!("failed reading {:?}", input.display()))?;
            let req = ConvertRequest {
                from: "audio".into(),
                to: "json".into(),
                options: opts,
                payload: InputPayload::AudioBase64 {
                    data_b64: B64.encode(bytes),
                },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "features_audio", &resp.artifacts)?;
        }

        Commands::TextFeatures { text } => {
            let txt = match text {
                Some(t) => t,
                None => {
                    let mut buf = String::new();
                    io::stdin()
                        .read_to_string(&mut buf)
                        .context("failed reading STDIN")?;
                    buf
                }
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
                .with_context(|| format!("failed reading {:?}", input.display()))?;
            let req = ConvertRequest {
                from: "image".into(),
                to: "json".into(),
                options: opts,
                payload: InputPayload::ImageBase64 {
                    data_b64: B64.encode(bytes),
                },
            };
            let resp = handle_convert(req)?;
            write_artifacts(&cli.out_dir, "features_image", &resp.artifacts)?;
        }
    }

    Ok(())
}
