use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use clap::{Parser, Subcommand};
use converters::{handle_convert, ConvertRequest, InputPayload, OutputArtifact, TransformOpts};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

#[derive(Parser, Debug)]
#[command(
    name = "xformed",
    version,
    about = "Transform between text, images and audio, locally"
)]
struct Cli {
    #[arg(long, global = true, default_value = "outputs")]
    out_dir: PathBuf,
    #[arg(long, global = true)]
    name: Option<String>,
    /// Musical duration, 1..180 seconds. Automatic when omitted.
    #[arg(long, global = true)]
    duration: Option<f32>,
    #[arg(long, global = true, default_value_t = 0)]
    variation: u32,
    #[arg(long, global = true, value_parser = ["nebula", "contour", "bloom"])]
    visual_style: Option<String>,
    #[arg(long, global = true, value_parser = ["poem", "vignette"])]
    writing_style: Option<String>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand, Debug)]
enum Commands {
    /// Compose a multi-instrument stereo track from text.
    TextToAudio {
        #[arg(long)]
        text: Option<String>,
    },
    /// Draw a procedural visual poem from text features.
    TextToImage {
        #[arg(long)]
        text: Option<String>,
    },
    /// Compose a multi-instrument stereo track from image features.
    ImageToAudio {
        #[arg(long)]
        input: PathBuf,
    },
    /// Write an emotional poem or vignette inspired by an image.
    ImageToText {
        #[arg(long)]
        input: PathBuf,
    },
    /// Create fractal artwork from the atmosphere of audio.
    AudioToImage {
        #[arg(long)]
        input: PathBuf,
    },
    /// Write an emotional poem or vignette inspired by audio.
    AudioToText {
        #[arg(long)]
        input: PathBuf,
    },
    TextFeatures {
        #[arg(long)]
        text: Option<String>,
    },
    ImageFeatures {
        #[arg(long)]
        input: PathBuf,
    },
    AudioFeatures {
        #[arg(long)]
        input: PathBuf,
    },
}
fn text_input(text: &Option<String>) -> Result<InputPayload> {
    let mut value = text.clone().unwrap_or_default();
    if text.is_none() {
        io::stdin().read_to_string(&mut value)?;
    }
    Ok(InputPayload::Text { text: value })
}
fn file_input(input: &Path, image: bool) -> Result<InputPayload> {
    let bytes = fs::read(input).with_context(|| format!("cannot read {}", input.display()))?;
    Ok(if image {
        InputPayload::ImageBase64 {
            data_b64: B64.encode(bytes),
        }
    } else {
        InputPayload::AudioBase64 {
            data_b64: B64.encode(bytes),
        }
    })
}
fn sanitize_basename(value: &str) -> String {
    let result: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let clean = result.trim_matches('_');
    if clean.is_empty() {
        "untitled".into()
    } else {
        clean.into()
    }
}
fn write_artifacts(dir: &Path, stem: &str, artifacts: &[OutputArtifact]) -> Result<()> {
    fs::create_dir_all(dir)?;
    for artifact in artifacts {
        let (extension, bytes) = match artifact {
            OutputArtifact::WavBase64 { data_b64 } => ("wav", B64.decode(data_b64)?),
            OutputArtifact::MidiBase64 { data_b64 } => ("mid", B64.decode(data_b64)?),
            OutputArtifact::MidiJsonBase64 { data_b64 } => ("midi.json", B64.decode(data_b64)?),
            OutputArtifact::PngBase64 { data_b64 } => ("png", B64.decode(data_b64)?),
            OutputArtifact::Text { text } => ("txt", text.as_bytes().to_vec()),
            OutputArtifact::CreativeInfo {
                title,
                mood,
                description,
                seed,
            } => (
                "creative.json",
                serde_json::to_vec_pretty(
                    &serde_json::json!({"title":title,"mood":mood,"description":description,"seed":seed}),
                )?,
            ),
            OutputArtifact::Json { data } => ("json", serde_json::to_vec_pretty(data)?),
        };
        let path = dir.join(format!("{stem}.{extension}"));
        fs::write(&path, bytes)?;
        println!("{}", path.display());
    }
    Ok(())
}
fn main() -> Result<()> {
    let cli = Cli::parse();
    let (from, to, payload) = match &cli.command {
        Commands::TextToAudio { text } => ("text", "audio", text_input(text)?),
        Commands::TextToImage { text } => ("text", "image", text_input(text)?),
        Commands::TextFeatures { text } => ("text", "json", text_input(text)?),
        Commands::ImageToAudio { input } => ("image", "audio", file_input(input, true)?),
        Commands::ImageToText { input } => ("image", "text", file_input(input, true)?),
        Commands::ImageFeatures { input } => ("image", "json", file_input(input, true)?),
        Commands::AudioToImage { input } => ("audio", "image", file_input(input, false)?),
        Commands::AudioToText { input } => ("audio", "text", file_input(input, false)?),
        Commands::AudioFeatures { input } => ("audio", "json", file_input(input, false)?),
    };
    let response = handle_convert(ConvertRequest {
        from: from.into(),
        to: to.into(),
        payload,
        options: TransformOpts {
            target_seconds: cli.duration,
            variation: cli.variation,
            image_style: cli.visual_style,
            text_style: cli.writing_style,
            ..Default::default()
        },
    })?;
    let stem = cli
        .name
        .as_deref()
        .map(sanitize_basename)
        .unwrap_or_else(|| format!("{from}_to_{to}"));
    write_artifacts(&cli.out_dir, &stem, &response.artifacts)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_options_after_subcommand() {
        let cli = Cli::try_parse_from([
            "xformed",
            "text-to-audio",
            "--name",
            "hello",
            "--duration",
            "3",
            "--text",
            "hello",
        ])
        .unwrap();
        assert_eq!(cli.name.as_deref(), Some("hello"));
        assert_eq!(cli.duration, Some(3.0));
    }
    #[test]
    fn all_routes_parse() {
        for command in ["text-to-audio", "text-to-image", "text-features"] {
            assert!(Cli::try_parse_from(["xformed", command, "--text", "hello"]).is_ok());
        }
        for command in [
            "image-to-audio",
            "image-to-text",
            "image-features",
            "audio-to-image",
            "audio-to-text",
            "audio-features",
        ] {
            assert!(Cli::try_parse_from(["xformed", command, "--input", "sample"]).is_ok());
        }
    }
}
