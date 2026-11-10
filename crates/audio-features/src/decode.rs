use anyhow::{bail, ensure, Context, Result};
use hound::{SampleFormat, WavReader};
use std::io::Cursor;

/// Decode PCM 8/16/24/32-bit or float32 WAV and downmix channels.
/// Corrupt samples and non-finite floats are errors, never silent replacements.
pub fn decode_wav_to_mono_f32(bytes: &[u8]) -> Result<(Vec<f32>, u32)> {
    let mut reader = WavReader::new(Cursor::new(bytes)).context("not a valid WAV")?;
    let spec = reader.spec();
    ensure!(
        spec.channels > 0 && spec.sample_rate > 0,
        "invalid WAV channels or sample rate"
    );
    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, bits @ (8 | 16 | 24 | 32)) => {
            let scale = (1u64 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|s| s as f32 / scale))
                .collect::<Result<_, _>>()?
        }
        (SampleFormat::Float, 32) => reader.samples::<f32>().collect::<Result<_, _>>()?,
        _ => bail!("unsupported WAV encoding; use PCM 8/16/24/32-bit or float32"),
    };
    ensure!(!samples.is_empty(), "empty WAV");
    ensure!(
        samples.iter().all(|s| s.is_finite()),
        "WAV contains non-finite samples"
    );
    let channels = spec.channels as usize;
    ensure!(samples.len() % channels == 0, "incomplete WAV frame");
    Ok((
        samples
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect(),
        spec.sample_rate,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn downmixes_stereo_and_rejects_truncation() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let spec = hound::WavSpec {
                channels: 2,
                sample_rate: 8000,
                bits_per_sample: 16,
                sample_format: SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
            writer.write_sample(16384i16).unwrap();
            writer.write_sample(-16384i16).unwrap();
            writer.finalize().unwrap();
        }
        let bytes = cursor.into_inner();
        let (mono, sr) = decode_wav_to_mono_f32(&bytes).unwrap();
        assert_eq!(sr, 8000);
        assert_eq!(mono, vec![0.0]);
        assert!(decode_wav_to_mono_f32(&bytes[..bytes.len() - 1]).is_err());
    }
}

/// Decode common local audio containers, bounding decoded duration while streaming.
/// WAV keeps its stricter PCM integrity checks; other formats use Symphonia.
pub fn decode_audio_to_mono_f32(bytes: &[u8]) -> Result<(Vec<f32>, u32)> {
    if bytes.starts_with(b"RIFF") {
        return decode_wav_to_mono_f32(bytes);
    }
    use symphonia::core::{
        audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions,
        io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
    };
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("unsupported audio; choose WAV, MP3, FLAC, OGG or M4A")?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .context("audio has no playable track")?;
    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut rate = 0u32;
    let mut mono = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet).context("corrupt audio packet")?;
        let spec = *decoded.spec();
        let channels = spec.channels.count();
        ensure!(channels > 0 && spec.rate > 0, "invalid audio format");
        if rate == 0 {
            rate = spec.rate;
        }
        ensure!(rate == spec.rate, "audio changes sample rate mid-stream");
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        for frame in buffer.samples().chunks_exact(channels) {
            let value = frame.iter().sum::<f32>() / channels as f32;
            ensure!(value.is_finite(), "non-finite audio sample");
            mono.push(value);
        }
        ensure!(
            mono.len() as u64 <= rate as u64 * 180,
            "recording exceeds 180 seconds"
        );
    }
    ensure!(!mono.is_empty(), "empty audio");
    Ok((mono, rate))
}
