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
        let format = detect_format(data).ok_or_else(|| unsupported(id))?;
        Ok(AudioAsset {
            bytes: data.to_vec(),
            format,
        })
    }
}

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

    #[test]
    fn random_bytes_fail_as_audio() {
        let error = AudioAssetDecoder
            .decode(bytes("audio/nope.wav", b"not audio"))
            .expect_err("must reject arbitrary bytes");
        assert_eq!(error.kind(), AssetLoadErrorKind::UnsupportedFormat);
    }
}
