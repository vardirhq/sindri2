//! Exports a project into a directory a static host can serve.
//!
//! ```bash
//! cargo run -p sindri-export --bin sindri-export -- game dist --base /sindri/
//! ```
//!
//! The WebAssembly host is built separately, by `wasm-pack`, into the `pkg`
//! directory this creates. That is deliberate: building wasm needs a toolchain
//! this does not want to own or version, and a build step that silently did it
//! would be one nobody could reproduce by hand.

use std::path::PathBuf;
use std::process::ExitCode;

use sindri_export::ProjectExport;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(project) = args.next().map(PathBuf::from) else {
        eprintln!("usage: sindri-export <project> [out] [--base /path/] [--build-id <id>]");
        return ExitCode::FAILURE;
    };
    let mut out = PathBuf::from("dist");
    let mut base = "/".to_owned();
    let mut build = String::new();
    let mut rest = args.collect::<Vec<_>>();
    for (flag, target) in [("--base", &mut base), ("--build-id", &mut build)] {
        if let Some(at) = rest.iter().position(|value| value == flag) {
            if at + 1 >= rest.len() {
                eprintln!("{flag} needs a value");
                return ExitCode::FAILURE;
            }
            *target = rest.remove(at + 1);
            rest.remove(at);
        }
    }
    if let Some(first) = rest.first() {
        out = PathBuf::from(first);
    }

    let gathered = match ProjectExport::gather(&project) {
        Ok(gathered) => gathered,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    match gathered.write_for_build(&out, &base, &build) {
        Ok(written) => {
            println!(
                "{} → {}: {} asset(s), {} bytes, under assets/{}",
                gathered.name,
                written.root.display(),
                written.files,
                written.bytes,
                written.content_root
            );
            println!(
                "Build the host into {}/pkg:  wasm-pack build <host crate> --target web --out-dir pkg",
                out.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
