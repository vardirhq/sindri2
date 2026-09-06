//! Writes the generated capability documents, or checks that they are current.
//!
//! ```bash
//! cargo run -p sindri-capabilities            # check, exit 1 when stale
//! cargo run -p sindri-capabilities -- --write # rewrite the files
//! ```
//!
//! Checking is the default because that is what CI does and what a contributor
//! wants to know; rewriting is the deliberate act.

use std::{fs, path::Path, process::ExitCode};

use sindri_capabilities::{GeneratedDocument, REGENERATE_COMMAND, documents};

fn main() -> ExitCode {
    let write = match mode() {
        Ok(write) => write,
        Err(argument) => {
            eprintln!("sindri-capabilities: unknown argument {argument}");
            eprintln!("usage: sindri-capabilities [--write]");
            return ExitCode::FAILURE;
        }
    };

    let documents = match documents() {
        Ok(documents) => documents,
        Err(error) => {
            eprintln!("sindri-capabilities: {error}");
            return ExitCode::FAILURE;
        }
    };

    let root = repository_root();
    if write {
        for document in &documents {
            if let Err(error) = write_document(root, document) {
                eprintln!(
                    "sindri-capabilities: {} could not be written: {error}",
                    document.path
                );
                return ExitCode::FAILURE;
            }
            println!("wrote {}", document.path);
        }
        return ExitCode::SUCCESS;
    }

    let stale: Vec<&str> = documents
        .iter()
        .filter(|document| !is_current(root, document))
        .map(|document| document.path)
        .collect();

    if stale.is_empty() {
        println!("the generated capability documents are current");
        return ExitCode::SUCCESS;
    }

    for path in &stale {
        eprintln!("sindri-capabilities: {path} is out of date");
    }
    eprintln!("run `{REGENERATE_COMMAND}`");
    ExitCode::FAILURE
}

/// `--write` or nothing. A flag rather than a subcommand, because this tool has
/// one job and giving it a command vocabulary would imply it has more.
fn mode() -> Result<bool, String> {
    let mut write = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--write" => write = true,
            other => return Err(other.to_owned()),
        }
    }
    Ok(write)
}

/// The workspace root, from where Cargo put this crate rather than from the
/// current directory, so the tool writes the right files from anywhere.
fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two directories below the repository root")
}

fn write_document(root: &Path, document: &GeneratedDocument) -> std::io::Result<()> {
    let path = root.join(document.path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, &document.contents)
}

fn is_current(root: &Path, document: &GeneratedDocument) -> bool {
    fs::read_to_string(root.join(document.path)).is_ok_and(|found| found == document.contents)
}
