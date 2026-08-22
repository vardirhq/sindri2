use sindri_core::{AssetId, AssetLoadErrorKind};

use crate::{AssetBytes, AssetDecodeError, AssetDecoder};

/// Encoded project audio after the asset pipeline has identified its container.
///
/// Audio stays encoded here. Native and browser backends already have mature
/// decoders for these containers, while eagerly expanding a music track to PCM
/// would turn a small asset into tens of megabytes before a device asks for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioAsset {
    bytes: Vec<u8>,
    format: AudioFormat,
}

impl AudioAsset {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub const fn format(&self) -> AudioFormat {
        self.format
    }
}

/// Containers every Sindri host promises to understand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioFormat {
    Wav,
    Ogg,
    Mp3,
}

impl AudioFormat {
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Wav => "audio/wav",
            Self::Ogg => "audio/ogg",
            Self::Mp3 => "audio/mpeg",
        }
    }
}

/// Identifies a supported audio container and rejects arbitrary bytes early.
///
/// Codec decoding belongs to the platform backend: Rodio/Symphonia natively
/// and the browser media stack on wasm. This decoder is the asset boundary's
/// validation step, so a `.png` accidentally registered as sound fails while
/// loading rather than much later when gameplay tries to play it.
#[derive(Clone, Copy, Debug, Default)]
pub struct AudioAssetDecoder;

impl AssetDecoder for AudioAssetDecoder {
    type Asset = AudioAsset;

    fn decode(&self, bytes: AssetBytes) -> Result<Self::Asset, AssetDecodeError> {
        let id = bytes.id().clone();
        let data = bytes.as_slice();
        let format = detect_format(data).ok_or_else(|| unsupported(id.clone()))?;
        if format == AudioFormat::Wav
            && let Some(rate) = wav_sample_rate(data)
            && !PLAYABLE_SAMPLE_RATES.contains(&rate)
        {
            return Err(AssetDecodeError::new(
                id,
                "audio",
                AssetLoadErrorKind::UnsupportedFormat,
                format!(
                    "{rate} Hz is outside the {}..={} Hz browsers decode",
                    PLAYABLE_SAMPLE_RATES.start(),
                    PLAYABLE_SAMPLE_RATES.end()
                ),
            ));
        }
        Ok(AudioAsset {
            bytes: data.to_vec(),
            format,
        })
    }
}

/// The sample rates a browser will decode.
///
/// Web Audio accepts 3 kHz through 768 kHz and refuses anything outside it, so
/// a clip below the floor loads and plays natively while a browser answers
/// `NotSupportedError` — asynchronously, from a promise, where it is easy to
/// miss. Gather shipped three 2 kHz clips that did exactly that. Rejecting the
/// rate here makes it one loud failure on every target instead.
const PLAYABLE_SAMPLE_RATES: std::ops::RangeInclusive<u32> = 3_000..=768_000;

fn detect_format(bytes: &[u8]) -> Option<AudioFormat> {
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        return Some(AudioFormat::Wav);
    }
    if bytes.starts_with(b"OggS") {
        return Some(AudioFormat::Ogg);
    }
    if bytes.starts_with(b"ID3")
        || bytes
            .windows(2)
            .next()
            .is_some_and(|header| header[0] == 0xff && header[1] & 0xe0 == 0xe0)
    {
        return Some(AudioFormat::Mp3);
    }
    None
}

/// Reads the sample rate out of a RIFF/WAVE `fmt ` chunk.
///
/// `None` for a header too short or malformed to say, which stays the codec
/// backend's problem to report: this is a bounds check on a rate that is
/// present, not a second WAV parser.
fn wav_sample_rate(bytes: &[u8]) -> Option<u32> {
    let mut offset = 12;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        if id == b"fmt " && size >= 16 && offset + 16 <= bytes.len() {
            return Some(u32::from_le_bytes(
                bytes[offset + 12..offset + 16].try_into().ok()?,
            ));
        }
        // Chunks are word-aligned, so an odd size is followed by a pad byte.
        offset += 8 + size + (size & 1);
    }
    None
}

fn unsupported(id: AssetId) -> AssetDecodeError {
    AssetDecodeError::new(
        id,
        "audio",
        AssetLoadErrorKind::UnsupportedFormat,
        "expected WAV, Ogg, or MP3 audio",
    )
}

#[cfg(test)]
mod tests {
    use super::{AudioAssetDecoder, AudioFormat};
    use crate::{AssetBytes, AssetDecoder};
    use sindri_core::{AssetId, AssetLoadErrorKind};

    fn bytes(id: &str, payload: &[u8]) -> AssetBytes {
        AssetBytes::new(id.parse::<AssetId>().expect("asset id"), payload.to_vec())
    }

    /// A minimal RIFF/WAVE header at one sample rate, with no samples.
    fn wav(rate: u32) -> Vec<u8> {
        let mut out = b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0".to_vec();
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data\0\0\0\0");
        out
    }

    #[test]
    fn recognizes_supported_containers() {
        let wav = AudioAssetDecoder
            .decode(bytes("audio/a.wav", b"RIFF\0\0\0\0WAVEfmt "))
            .expect("wav");
        assert_eq!(wav.format(), AudioFormat::Wav);

        let ogg = AudioAssetDecoder
            .decode(bytes("audio/a.ogg", b"OggS\0\0\0\0"))
            .expect("ogg");
        assert_eq!(ogg.format(), AudioFormat::Ogg);

        let mp3 = AudioAssetDecoder
            .decode(bytes("audio/a.mp3", b"ID3\x04\0\0"))
            .expect("mp3");
        assert_eq!(mp3.format(), AudioFormat::Mp3);
    }

    /// A WAV below the browser floor loads and plays natively while a browser
    /// rejects it from a promise nothing was watching. Gather shipped three at
    /// 2 kHz and every test passed.
    #[test]
    fn wav_outside_the_playable_sample_rate_fails_to_load() {
        let error = AudioAssetDecoder
            .decode(bytes("audio/slow.wav", &wav(2_000)))
            .expect_err("2 kHz is below what a browser decodes");
        assert_eq!(error.kind(), AssetLoadErrorKind::UnsupportedFormat);
        assert!(
            error.to_string().contains("2000 Hz"),
            "the error names the rate that failed: {error}"
        );

        AudioAssetDecoder
            .decode(bytes("audio/fine.wav", &wav(44_100)))
            .expect("44.1 kHz is ordinary");
    }

    #[test]
    fn random_bytes_fail_as_audio() {
        let error = AudioAssetDecoder
            .decode(bytes("audio/nope.wav", b"not audio"))
            .expect_err("must reject arbitrary bytes");
        assert_eq!(error.kind(), AssetLoadErrorKind::UnsupportedFormat);
    }
}
