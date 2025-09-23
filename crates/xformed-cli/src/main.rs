use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use converters::{TransformOpts, InputPayload, ConvertRequest, handle_convert};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use base64::Engine;

#[derive(Parser, Debug)]
#[command(name="xformed", version, about="Xformed CLI - run conversions without HTTP")]
struct Cli {
    /// Output directory (default: ./outputs)
    #[arg(long, global=true, default_value="outputs")]
    out_dir: PathBuf,

    /// Instrument for audio (sine|saw|square)
    #[arg(long, default_value="saw")]
    instrument: String,

    /// Mood (auto|major|minor)
    #[arg(long, default_value="auto")]
    mood: String,

    /// Tempo BPM (optional)
    #[arg(long)]
    tempo: Option<u32>,

    /// Jumpiness 0..1
    #[arg(long, default_value_t=0.25)]
    jumpiness: f32,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Convert free-typed text into audio (melody)
    TextToAudio {
        /// Text to transform; if omitted, you'll be prompted to type it
        #[arg(long)]
        text: Option<String>,
    },
    /// Convert an image file (png/jpg) into audio (melody)
    ImageToAudio {
        /// Path to image file
        path: PathBuf,
    },
}

fn ensure_out_dir(p: &Path) -> Result<()> {
    if !p.exists() {
        fs::create_dir_all(p).with_context(|| format!("create out dir {}", p.display()))?;
    }
    Ok(())
}

fn write_b64(path: &Path, b64: &str) -> Result<()> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .context("decode base64")?;
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    ensure_out_dir(&cli.out_dir)?;

    let opts = TransformOpts {
        instrument: Some(cli.instrument.clone()),
        tempo: cli.tempo,
        mood: Some(cli.mood.clone()),
        jumpiness: Some(cli.jumpiness),
    };

    match cli.cmd {
        Commands::TextToAudio { text } => {
            let t = match text {
                Some(s) => s,
                None => {
                    eprintln!("Scrie textul și apasă Enter (Ctrl+Z apoi Enter ca să închei multi-linie pe Windows):");
                    let mut buf = String::new();
                    io::stdin().read_to_string(&mut buf)?;
                    if buf.trim().is_empty() {
                        eprintln!("(hint) Poți folosi și: xformed text-to-audio --text \"fraza ta\"");
                        anyhow::bail!("nu am primit text");
                    }
                    buf
                }
            };

            let req = ConvertRequest {
                from: "text".to_string(),
                to: "audio".to_string(),
                options: opts,
                payload: InputPayload::Text { text: t },
            };
            let resp = handle_convert(req)?;

            let mut saved = 0usize;
            for art in resp.artifacts {
                match art {
                    converters::OutputArtifact::WavBase64 { data_b64 } => {
                        let p = cli.out_dir.join("out.wav");
                        write_b64(&p, &data_b64)?;
                        println!("WAV  -> {}", p.display());
                        saved += 1;
                    }
                    converters::OutputArtifact::MidiBase64 { data_b64 } => {
                        let p = cli.out_dir.join("out.mid");
                        write_b64(&p, &data_b64)?;
                        println!("MIDI -> {}", p.display());
                        saved += 1;
                    }
                    converters::OutputArtifact::Json { data } => {
                        let p = cli.out_dir.join("response.json");
                        fs::write(&p, serde_json::to_string_pretty(&data)?)?;
                        println!("JSON -> {}", p.display());
                    }
                    converters::OutputArtifact::PngBase64 { data_b64 } => {
                        let p = cli.out_dir.join("out.png");
                        write_b64(&p, &data_b64)?;
                        println!("PNG  -> {}", p.display());
                        saved += 1;
                    }
                }
            }
            if saved == 0 {
                anyhow::bail!("nu am primit WAV/MIDI/PNG în răspuns");
            }
        }
        Commands::ImageToAudio { path } => {
            let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);

            let req = ConvertRequest {
                from: "image".to_string(),
                to: "audio".to_string(),
                options: opts,
                payload: InputPayload::ImageBase64 { data_b64: b64 },
            };
            let resp = handle_convert(req)?;

            let mut saved = 0usize;
            for art in resp.artifacts {
                match art {
                    converters::OutputArtifact::WavBase64 { data_b64 } => {
                        let p = cli.out_dir.join("out_from_image.wav");
                        write_b64(&p, &data_b64)?;
                        println!("WAV  -> {}", p.display());
                        saved += 1;
                    }
                    converters::OutputArtifact::MidiBase64 { data_b64 } => {
                        let p = cli.out_dir.join("out_from_image.mid");
                        write_b64(&p, &data_b64)?;
                        println!("MIDI -> {}", p.display());
                        saved += 1;
                    }
                    converters::OutputArtifact::Json { data } => {
                        let p = cli.out_dir.join("response.json");
                        fs::write(&p, serde_json::to_string_pretty(&data)?)?;
                        println!("JSON -> {}", p.display());
                    }
                    _ => {}
                }
            }
            if saved == 0 {
                anyhow::bail!("nu am primit WAV/MIDI în răspuns");
            }
        }
    }

    Ok(())
}
