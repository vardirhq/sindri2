use std::{
    env,
    fmt::Write,
    fs,
    io::{self, Read},
    process::ExitCode,
};

use sindri_diagnostics::{
    DiagnosticReport, GithubAnnotation, parse_decay_report, parse_file_size_violations,
    fingerprint_failure, parse_rustfmt_diff, render_terminal_report,
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("sindri-diagnostics: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return Err("expected command: rustfmt, decay, file-size, or fingerprint".into());
    };
    if command != "rustfmt"
        && command != "decay"
        && command != "file-size"
        && command != "fingerprint"
    {
        return Err(format!("unknown command: {command}").into());
    }

    let mut github = false;
    let mut json = false;
    for argument in args {
        match argument.as_str() {
            "--github" => github = true,
            "--json" => json = true,
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    if github && json {
        return Err("--github and --json are mutually exclusive".into());
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    if command == "fingerprint" {
        if github || json {
            return Err("fingerprint does not accept renderer flags".into());
        }
        if let Some(fingerprint) = fingerprint_failure(&input) {
            println!("{}", fingerprint.value);
            return Ok(ExitCode::SUCCESS);
        }
        return Ok(ExitCode::FAILURE);
    }

    let report = match command.as_str() {
        "decay" => parse_decay_report(&input)?,
        "file-size" => DiagnosticReport::new(parse_file_size_violations(&input)),
        _ => DiagnosticReport::new(parse_rustfmt_diff(&input)),
    };

    if github {
        for diagnostic in &report.diagnostics {
            println!("{}", GithubAnnotation::from_diagnostic(diagnostic).as_str());
        }
        if let Ok(summary_path) = env::var("GITHUB_STEP_SUMMARY") {
            fs::write(summary_path, github_summary(&report, &command))?;
        }
    } else if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", render_terminal_report(&report));
    }

    Ok(if report.success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn github_summary(report: &DiagnosticReport, command: &str) -> String {
    let errors = report.error_count();
    let warnings = report.warning_count();
    let mut output = format!(
        "## Sindri diagnostics\n\n**{errors} {command} error(s), {warnings} warning(s)**\n\n"
    );
    for diagnostic in &report.diagnostics {
        let location = diagnostic.location.as_ref().map_or_else(
            || "unknown file".into(),
            |location| location.path.display().to_string(),
        );
        let _ = writeln!(output, "- `{location}`: {}", diagnostic.message);
    }
    if command == "rustfmt" && errors > 0 {
        output.push_str("\nRun `cargo fmt --all` locally before pushing.\n");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_names_every_unformatted_file() {
        let report = DiagnosticReport::new(parse_rustfmt_diff(
            "Diff in crates/a/src/lib.rs:1:\nDiff in crates/b/src/lib.rs:2:\n",
        ));
        let summary = github_summary(&report, "rustfmt");
        assert!(summary.contains("crates/a/src/lib.rs"));
        assert!(summary.contains("crates/b/src/lib.rs"));
        assert!(summary.contains("2 rustfmt error(s)"));
    }
}
