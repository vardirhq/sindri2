use sindri_core::AssetId;
use thiserror::Error;

/// Resolves logical asset IDs into URLs beneath a base.
///
/// Web hosting is not always at a domain root: a game may be served from
/// `/games/demo/`, from a CDN, or from a directory relative to the page. The
/// base captures that, and every asset URL is built from it the same way.
///
/// A base may be empty (assets sit beside the page), relative (`assets/`),
/// root-relative (`/games/demo/`), or absolute
/// (`https://cdn.example.com/assets/`). It is normalised to end in a single
/// slash so callers need not care whether they supplied one.
///
/// The logic lives here rather than in the browser source so it is testable on
/// any target. A `fetch` implementation compiled only for `wasm32` is a poor
/// place to keep rules nothing can exercise.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UrlRoot {
    base: String,
}

impl UrlRoot {
    /// Creates a root from `base`, appending a trailing slash if absent.
    ///
    /// A base carrying a query string or fragment is rejected: the asset path
    /// is appended to the base, so anything after it would end up in the middle
    /// of the URL and silently request the wrong thing.
    pub fn new(base: impl Into<String>) -> Result<Self, UrlRootError> {
        let mut base = base.into();
        if let Some(delimiter) = ['?', '#'].into_iter().find(|value| base.contains(*value)) {
            return Err(UrlRootError::QueryOrFragment(delimiter));
        }
        if base.chars().any(char::is_control) {
            return Err(UrlRootError::ControlCharacter);
        }
        if !base.is_empty() && !base.ends_with('/') {
            base.push('/');
        }
        Ok(Self { base })
    }

    /// A root resolving against the page's own directory.
    pub fn relative() -> Self {
        Self {
            base: String::new(),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// The URL `id` loads from.
    ///
    /// Each path segment is percent-encoded, so an ID containing spaces or
    /// non-ASCII resolves to a URL that requests the file it names. The
    /// separators between segments are preserved as separators.
    pub fn resolve(&self, id: &AssetId) -> String {
        let mut url = self.base.clone();
        for (index, segment) in id.as_str().split('/').enumerate() {
            if index > 0 {
                url.push('/');
            }
            encode_segment(segment, &mut url);
        }
        url
    }
}

/// Percent-encodes everything outside RFC 3986's unreserved set.
///
/// Encoding more than strictly necessary is safe; encoding less is not.
fn encode_segment(segment: &str, out: &mut String) {
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(hex(byte >> 4));
            out.push(hex(byte & 0x0F));
        }
    }
}

const fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + nibble - 10) as char,
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum UrlRootError {
    #[error(
        "asset URL roots cannot contain '{0}': asset paths are appended to the root, so anything \
         after it would land in the middle of the URL"
    )]
    QueryOrFragment(char),
    #[error("asset URL roots cannot contain control characters")]
    ControlCharacter,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> AssetId {
        AssetId::new(value).expect("a valid asset ID")
    }

    #[test]
    fn a_missing_trailing_slash_is_added() {
        assert_eq!(UrlRoot::new("assets").unwrap().base(), "assets/");
        assert_eq!(UrlRoot::new("assets/").unwrap().base(), "assets/");
        assert_eq!(UrlRoot::new("").unwrap().base(), "");
    }

    #[test]
    fn assets_resolve_under_every_shape_of_base() {
        let texture = id("textures/badge.png");
        for (base, expected) in [
            ("", "textures/badge.png"),
            ("assets", "assets/textures/badge.png"),
            ("/games/demo/", "/games/demo/textures/badge.png"),
            ("/", "/textures/badge.png"),
            (
                "https://cdn.example.com/v2",
                "https://cdn.example.com/v2/textures/badge.png",
            ),
        ] {
            assert_eq!(UrlRoot::new(base).unwrap().resolve(&texture), expected);
        }
    }

    /// Static hosting under a non-root path is the case that breaks when a
    /// resolver assumes it owns the domain.
    #[test]
    fn a_non_root_base_path_is_preserved() {
        let root = UrlRoot::new("/games/demo").unwrap();
        assert_eq!(
            root.resolve(&id("scenes/room.json")),
            "/games/demo/scenes/room.json"
        );
    }

    #[test]
    fn segment_separators_survive_encoding() {
        let root = UrlRoot::new("assets/").unwrap();
        assert_eq!(
            root.resolve(&id("a/b/c/d.png")),
            "assets/a/b/c/d.png",
            "slashes separate segments and must not be encoded"
        );
    }

    #[test]
    fn spaces_and_non_ascii_are_percent_encoded() {
        let root = UrlRoot::relative();
        assert_eq!(
            root.resolve(&id("my textures/hero.png")),
            "my%20textures/hero.png"
        );
        // 'é' is two UTF-8 bytes, each encoded on its own.
        assert_eq!(root.resolve(&id("café.png")), "caf%C3%A9.png");
        assert_eq!(root.resolve(&id("100%.png")), "100%25.png");
        assert_eq!(root.resolve(&id("a+b.png")), "a%2Bb.png");
    }

    #[test]
    fn unreserved_characters_are_left_alone() {
        let root = UrlRoot::relative();
        assert_eq!(
            root.resolve(&id("Tile-set_01.v2~final.png")),
            "Tile-set_01.v2~final.png"
        );
    }

    #[test]
    fn a_base_carrying_a_query_or_fragment_is_rejected() {
        assert_eq!(
            UrlRoot::new("assets?v=2"),
            Err(UrlRootError::QueryOrFragment('?'))
        );
        assert_eq!(
            UrlRoot::new("assets#frag"),
            Err(UrlRootError::QueryOrFragment('#'))
        );
        assert_eq!(
            UrlRoot::new("assets\nx"),
            Err(UrlRootError::ControlCharacter)
        );
    }

    #[test]
    fn resolution_is_stable() {
        let root = UrlRoot::new("assets").unwrap();
        let texture = id("textures/badge.png");
        assert_eq!(root.resolve(&texture), root.resolve(&texture));
    }
}
