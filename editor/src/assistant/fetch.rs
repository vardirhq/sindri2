//! Getting a file onto the machine and proving it is the file we meant.
//!
//! Sindri downloads the model runner and the model itself, which is a real
//! thing to do on someone's computer. Three things make it defensible, and all
//! three are here rather than in the panel that triggers it.
//!
//! **The source is pinned, not discovered.** Every asset comes from a committed
//! manifest carrying a URL, a size, a licence and a SHA-256. Nothing consults a
//! service at run time for what to fetch.
//!
//! **Transport is the platform's own downloader.** `curl`, or `wget` where
//! there is no curl — which is how this stays honest about the rule that
//! Sindri's crate graph gains no HTTP stack. There is no TLS client here, no
//! certificate handling, and no networking dependency; the tool every one of
//! these systems already ships does the part it is good at. It also means
//! resumption and proxy configuration are whatever the machine is already set
//! up for, rather than a second implementation of both.
//!
//! **Verification is Sindri's own.** A downloader that reports success has said
//! nothing about *what* it fetched. The bytes are hashed here and compared with
//! the manifest, and a file that does not match is deleted rather than used.

use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// One downloadable thing, exactly as the manifest states it.
///
/// The field names are shared with `vardirhq/local-code`'s manifests on
/// purpose: the two tools fetch the same kinds of asset for the same reasons,
/// and a machine set up by one should be legible to the other. The files stay
/// separate so their release cadences do not have to agree.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Asset {
    pub id: String,
    pub version: String,
    pub platform: String,
    pub architecture: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub display_name: String,
    pub description: String,
    pub license: String,
    pub source: String,
}

impl Asset {
    /// Whether this entry is for the machine described.
    ///
    /// `any` matches anything, which is what a platform-independent asset such
    /// as a model file says of itself.
    pub fn suits(&self, platform: &str, architecture: &str) -> bool {
        (self.platform == platform || self.platform == "any")
            && (self.architecture == architecture || self.architecture == "any")
    }
}

/// What this build calls the platform it is running on.
pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

/// What this build calls the architecture it is running on.
pub fn architecture() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

/// Why an acquisition did not happen.
#[derive(Debug)]
pub enum Trouble {
    /// Neither downloader is on the machine.
    NoDownloader,
    /// The manifest names nothing for this platform and architecture.
    Unsupported {
        platform: String,
        architecture: String,
    },
    /// The downloader ran and failed.
    Transport(String),
    /// It fetched something, and that something was not what we asked for.
    ///
    /// The interesting failure, and the reason the hash is checked at all: a
    /// captive portal, a truncated transfer and a tampered mirror all look like
    /// a successful download to the tool that performed it.
    Corrupt {
        expected: String,
        actual: String,
    },
    Io(std::io::Error),
}

impl fmt::Display for Trouble {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDownloader => write!(
                formatter,
                "no downloader found: Sindri uses curl or wget to fetch files"
            ),
            Self::Unsupported {
                platform,
                architecture,
            } => write!(
                formatter,
                "nothing is published for {platform}-{architecture}"
            ),
            Self::Transport(detail) => write!(formatter, "the download failed: {detail}"),
            Self::Corrupt { expected, actual } => write!(
                formatter,
                "the file did not match what was expected: wanted {expected}, got {actual}"
            ),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<std::io::Error> for Trouble {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// What an acquisition turned out to cost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// Already on disk and already correct. Setup being re-runnable for free is
    /// what makes it safe to re-run at all.
    Reused,
    Fetched,
}

/// The tools that can fetch a file, in the order they are tried.
///
/// Both ask for a resume where the server allows one, so an interrupted
/// download of a multi-gigabyte model costs the part that did not arrive rather
/// than all of it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Downloader {
    Curl,
    Wget,
}

impl Downloader {
    /// Whichever is on this machine.
    pub fn found() -> Option<Self> {
        [Self::Curl, Self::Wget]
            .into_iter()
            .find(|tool| which(tool.program()).is_some())
    }

    pub const fn program(self) -> &'static str {
        match self {
            Self::Curl => "curl",
            Self::Wget => "wget",
        }
    }

    /// How to ask it for `url`, written to `into`.
    pub fn arguments(self, url: &str, into: &Path) -> Vec<String> {
        let destination = into.display().to_string();
        match self {
            Self::Curl => vec![
                "--location".to_owned(),
                "--fail".to_owned(),
                "--continue-at".to_owned(),
                "-".to_owned(),
                "--output".to_owned(),
                destination,
                url.to_owned(),
            ],
            Self::Wget => vec![
                "--continue".to_owned(),
                "--output-document".to_owned(),
                destination,
                url.to_owned(),
            ],
        }
    }
}

/// Whether a program is on the path.
fn which(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?
        .to_str()
        .map(str::to_owned)
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|directory| directory.join(program))
                .find(|candidate| candidate.is_file())
        })
}

/// The SHA-256 of a file, as lowercase hexadecimal.
pub fn digest(path: &Path) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    // Streamed rather than read whole: a model is gigabytes and a verification
    // that needs the file in memory is one that fails on the machines that most
    // need it to work.
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    // Written out by hand: sha2 0.11 returns a byte array without a hex
    // formatter, and a digest is compared as text everywhere it is used.
    Ok(hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            use fmt::Write as _;
            let _ = write!(text, "{byte:02x}");
            text
        }))
}

/// Whether the file on disk is already the one the manifest names.
pub fn verified(path: &Path, sha256: &str) -> bool {
    path.is_file() && digest(path).is_ok_and(|actual| actual.eq_ignore_ascii_case(sha256))
}

/// Fetches `asset` to `into`, unless it is already there and correct.
///
/// `transport` performs the download, and exists so the surrounding
/// discipline — reuse, part files, hashing, the atomic rename — can be tested
/// without a network.
///
/// The part file is the whole reason this is safe to interrupt: the
/// destination only ever comes into existence by a rename, and only after the
/// hash matched. There is no window in which a half-written or wrong file sits
/// at the name everything else looks for.
pub fn fetch(
    asset: &Asset,
    into: &Path,
    transport: impl FnOnce(&str, &Path) -> Result<(), Trouble>,
) -> Result<Outcome, Trouble> {
    if verified(into, &asset.sha256) {
        return Ok(Outcome::Reused);
    }
    if let Some(parent) = into.parent() {
        fs::create_dir_all(parent)?;
    }
    let partial = into.with_extension("part");
    transport(&asset.url, &partial)?;
    let actual = digest(&partial)?;
    if !actual.eq_ignore_ascii_case(&asset.sha256) {
        // Removed rather than left for a resume to build on: a mismatch means
        // the bytes are wrong, and continuing from wrong bytes only ever
        // produces more of them.
        let _ = fs::remove_file(&partial);
        return Err(Trouble::Corrupt {
            expected: asset.sha256.clone(),
            actual,
        });
    }
    fs::rename(&partial, into)?;
    Ok(Outcome::Fetched)
}

/// Downloads through whichever tool the machine has.
pub fn platform_transport(url: &str, into: &Path) -> Result<(), Trouble> {
    let tool = Downloader::found().ok_or(Trouble::NoDownloader)?;
    let output = std::process::Command::new(tool.program())
        .args(tool.arguments(url, into))
        .output()
        .map_err(|error| Trouble::Transport(error.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    Err(Trouble::Transport(
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
}

#[cfg(test)]
mod tests;
