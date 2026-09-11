//! Looking at the machine to see how far along setup is.
//!
//! Deliberately written against the standard library alone. Everything here
//! talks to `localhost` or reads a file, so none of it needs TLS, and the
//! engine's rule that an AI-disabled build stays a normal configuration is
//! easier to keep when the editor has not grown an HTTP stack to ask whether a
//! port is open.
//!
//! Every answer is best-effort and every failure is "no" rather than an error:
//! a probe that cannot tell is a probe that reports what it could see, and the
//! state machine is built to treat "could not tell" as its own answer.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use super::Probe;

/// Where the runner listens by default.
const ENDPOINT: &str = "127.0.0.1:11434";

/// How long to wait on a local socket before calling it shut.
///
/// Short, because this runs on a timer while the setup panel is open and a
/// person watching an install finish should see the screen move within a
/// second of it finishing, not five.
const TIMEOUT: Duration = Duration::from_millis(600);

/// Reads the machine as it currently stands.
///
/// Blocking, and meant for a worker thread rather than a frame: a shut port
/// costs a connection timeout, and a frame that waits on one drops.
pub fn probe() -> Probe {
    let installed = runner_path().is_some();
    let Some(version) = ask("GET /api/version HTTP/1.1") else {
        return Probe {
            backend_installed: installed,
            backend_reachable: false,
            available_memory: available_memory(),
            ..Probe::default()
        };
    };
    Probe {
        // Reachable implies installed, whatever the filesystem says: a runner
        // someone installed somewhere this does not look for is still a runner.
        backend_installed: true,
        backend_reachable: true,
        backend_version: field(&version, "version"),
        models: ask("GET /api/tags HTTP/1.1")
            .map(|body| names(&body))
            .unwrap_or_default(),
        available_memory: available_memory(),
    }
}

/// Where the runner's binary is, if it is anywhere this knows to look.
///
/// The user-space location first, because that is where Sindri puts one it
/// installed itself and it needs no privilege to write.
fn runner_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(&home).join(".local/share/sindri/ollama"));
        candidates.push(PathBuf::from(&home).join(".local/bin/ollama"));
    }
    candidates.push(PathBuf::from("/usr/local/bin/ollama"));
    candidates.push(PathBuf::from("/usr/bin/ollama"));
    candidates.extend(
        std::env::var_os("PATH")
            .iter()
            .flat_map(|path| std::env::split_paths(path))
            .map(|dir| dir.join("ollama")),
    );
    candidates.into_iter().find(|path| path.is_file())
}

/// One request to the local runner, returning its body.
///
/// A hand-written request rather than a client, because it is one line of
/// HTTP/1.1 to a loopback address and the alternative is a dependency the
/// engine's crate graph is meant not to grow.
fn ask(request_line: &str) -> Option<String> {
    let address: SocketAddr = ENDPOINT.parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT).ok()?;
    stream.set_read_timeout(Some(TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(TIMEOUT)).ok()?;
    write!(
        stream,
        "{request_line}\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    reader.read_line(&mut status).ok()?;
    if !status.contains(" 200") {
        return None;
    }
    // Skip headers; the body is whatever follows the blank line.
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        if header.trim().is_empty() {
            break;
        }
    }
    let mut body = String::new();
    reader.read_to_string(&mut body).ok()?;
    Some(body)
}

/// One string field out of a small JSON object.
fn field(body: &str, key: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get(key)?
        .as_str()
        .map(str::to_owned)
}

/// The model names out of a `/api/tags` answer.
fn names(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    value
        .get("models")
        .and_then(serde_json::Value::as_array)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| model.get("name")?.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// How much memory a model could have, in gigabytes.
///
/// Video memory where a GPU can be read, otherwise what the system has spare.
/// `None` when neither can be determined, which the state machine treats as its
/// own answer rather than as nothing.
fn available_memory() -> Option<f32> {
    video_memory().or_else(system_memory)
}

/// Video memory, asked of whichever vendor tool is present.
///
/// Both vendors, because asking only NVIDIA means every Radeon machine falls
/// silently through to system memory and gets told it can run less than it can.
/// Best-effort throughout: a machine with neither tool is not a machine without
/// a GPU, so a failure falls through rather than concluding anything.
fn video_memory() -> Option<f32> {
    nvidia_memory().or_else(amd_memory)
}

fn nvidia_memory() -> Option<f32> {
    let text = run(
        "nvidia-smi",
        &["--query-gpu=memory.total", "--format=csv,noheader,nounits"],
    )?;
    let megabytes: f32 = text.lines().next()?.trim().parse().ok()?;
    Some(megabytes / 1024.0)
}

/// The largest AMD card, in gigabytes.
///
/// `rocm-smi` reports in bytes and one line per card; the largest is taken
/// rather than the sum, because a model runs on one card unless it has been
/// deliberately split across several.
fn amd_memory() -> Option<f32> {
    let text = run("rocm-smi", &["--showmeminfo", "vram", "--csv"])?;
    text.lines()
        .filter_map(|line| line.rsplit(',').next())
        // Parsed as `f32` directly: a card's memory in bytes is far inside what
        // a float holds exactly enough for a figure shown to one decimal place.
        .filter_map(|field| field.trim().parse::<f32>().ok())
        .map(|bytes| bytes / 1_073_741_824.0)
        .filter(|gigabytes| *gigabytes > 0.0)
        .max_by(f32::total_cmp)
}

/// One command, or nothing when it is absent or unhappy.
fn run(program: &str, arguments: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
}

/// What the system has, as the fallback for a machine running on its CPU.
fn system_memory() -> Option<f32> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = meminfo
        .lines()
        .find(|line| line.starts_with("MemAvailable:"))?;
    let kilobytes: f32 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kilobytes / 1_048_576.0)
}

/// Asks the runner to download a model.
///
/// Over the same loopback socket everything else here uses, so it needs no TLS
/// and no HTTP client: the runner does the fetching and Sindri asks it to. The
/// request streams progress lines until it finishes, which this drains rather
/// than reports — the probe notices the model appear, which is the same answer
/// arriving by the route the rest of setup already trusts.
pub fn pull(tag: &str) -> Option<()> {
    let address: SocketAddr = ENDPOINT.parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT).ok()?;
    // No read timeout: a multi-gigabyte download is minutes of silence between
    // progress lines and cutting it off at six hundred milliseconds would
    // cancel every pull that mattered.
    stream.set_write_timeout(Some(TIMEOUT)).ok()?;
    let body = format!("{{\"model\":\"{tag}\"}}");
    write!(
        stream,
        "POST /api/pull HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .ok()?;
    let mut sink = String::new();
    BufReader::new(stream).read_to_string(&mut sink).ok()?;
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parsing is the part worth testing without a runner present: the shapes
    /// come from someone else's API and are what breaks when it changes.
    #[test]
    fn model_names_are_read_from_a_tags_answer() {
        let body = r#"{"models":[{"name":"qwen3:8b"},{"name":"gemma3:12b"}]}"#;
        assert_eq!(names(body), ["qwen3:8b", "gemma3:12b"]);
    }

    #[test]
    fn an_answer_with_no_models_reads_as_none_rather_than_failing() {
        assert!(names(r#"{"models":[]}"#).is_empty());
        assert!(names("{}").is_empty());
        assert!(names("not json at all").is_empty());
    }

    #[test]
    fn a_version_is_read_from_a_version_answer() {
        assert_eq!(
            field(r#"{"version":"0.5.0"}"#, "version").as_deref(),
            Some("0.5.0")
        );
        assert_eq!(field("{}", "version"), None);
        assert_eq!(field("not json", "version"), None);
    }

    /// A machine with nothing running must report that rather than hanging or
    /// erroring, because "nothing is there" is the first state setup handles.
    #[test]
    fn probing_a_machine_with_no_runner_answers_rather_than_failing() {
        let seen = probe();
        if !seen.backend_reachable {
            assert!(seen.models.is_empty());
            assert!(seen.backend_version.is_none());
        }
    }
}
