use std::{
    collections::BTreeMap,
    env,
    fmt::Write,
    fs,
    io::{self, Read},
    process::ExitCode,
};

use sindri_diagnostics::{
    CheckOutcome, CheckResult, Diagnostic, DiagnosticReport, GithubAnnotation, fingerprint_failure,
    fingerprint_report, parse_cargo_messages, parse_decay_report, parse_file_size_violations,
    parse_rustfmt_diff, render_ci_summary, render_terminal_report,
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
        return Err(
            "expected command: rustfmt, cargo, decay, file-size, fingerprint, report-check-result, correlate, or check-result"
                .into(),
        );
    };
    if command != "rustfmt"
        && command != "cargo"
        && command != "decay"
        && command != "file-size"
        && command != "fingerprint"
        && command != "report-check-result"
        && command != "correlate"
        && command != "check-result"
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

    if command == "report-check-result" {
        return report_check_result(&input, github, json);
    }

    if command == "correlate" {
        if github || json {
            return Err("correlate does not accept renderer flags".into());
        }
        let checks: Vec<CheckResult> = serde_json::from_str(&input)?;
        print!("{}", render_ci_summary(&checks));
        return Ok(ExitCode::SUCCESS);
    }

    if command == "check-result" {
        if github || json {
            return Err("check-result does not accept renderer flags".into());
        }
        let mut lines = input.lines();
        let name = lines
            .next()
            .ok_or("check-result expects a check name on the first line")?;
        let log = lines.collect::<Vec<_>>().join("\n");
        let fingerprint = fingerprint_failure(&log);
        let result = CheckResult {
            name: name.into(),
            outcome: CheckOutcome::Failure,
            fingerprint: fingerprint.as_ref().map(|value| value.value.clone()),
            infrastructure: fingerprint.is_some_and(|value| value.infrastructure),
        };
        println!("{}", serde_json::to_string(&result)?);
        return Ok(ExitCode::SUCCESS);
    }

    let report = match command.as_str() {
        "cargo" => DiagnosticReport::new(parse_cargo_messages(&input)?),
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

fn report_check_result(
    input: &str,
    github: bool,
    json: bool,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    if github || json {
        return Err("report-check-result does not accept renderer flags".into());
    }
    let mut lines = input.lines();
    let name = lines
        .next()
        .ok_or("report-check-result expects a check name on the first line")?;
    let report_json = lines.collect::<Vec<_>>().join("\n");
    let report: DiagnosticReport = serde_json::from_str(&report_json)?;
    let fingerprint = fingerprint_report(&report);
    let result = CheckResult {
        name: name.into(),
        outcome: CheckOutcome::Failure,
        fingerprint: fingerprint.as_ref().map(|value| value.value.clone()),
        infrastructure: fingerprint.is_some_and(|value| value.infrastructure),
    };
    println!("{}", serde_json::to_string(&result)?);
    Ok(ExitCode::SUCCESS)
}

fn github_summary(report: &DiagnosticReport, command: &str) -> String {
    let errors = report.error_count();
    let warnings = report.warning_count();
    let actionable = report
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, sindri_diagnostics::Severity::Error))
        .collect::<Vec<_>>();
    let groups = group_diagnostics(&actionable);
    let locations = actionable
        .iter()
        .filter(|diagnostic| diagnostic.location.is_some())
        .count();
    let start = actionable
        .iter()
        .find_map(|diagnostic| diagnostic.location.as_ref())
        .map_or_else(
            || "the first diagnostic below".into(),
            |location| {
                format!(
                    "`{}:{}:{}`",
                    location.path.display(),
                    location.line,
                    location.column
                )
            },
        );

    let mut output = format!(
        "## Sindri diagnostics\n\n**{errors} {command} error(s), {warnings} warning(s)**\n\n"
    );
    if errors > 0 {
        let _ = writeln!(
            output,
            "**Repair scope:** {} diagnostic(s) in {} failure class(es); {locations} exact source location(s).",
            actionable.len(),
            groups.len()
        );
        let _ = writeln!(output, "**Start here:** {start}\n");
    }

    for (key, diagnostics) in groups {
        let _ = writeln!(output, "### {} ({})", key, diagnostics.len());
        for diagnostic in diagnostics {
            render_summary_diagnostic(&mut output, diagnostic);
        }
        output.push('\n');
    }

    if command == "rustfmt" && errors > 0 {
        output.push_str("**Why:** committed Rust does not match the repository formatter.\n\n");
        output.push_str(
            "**How:** run `cargo fmt --all`, review the diff, and commit every affected file.\n\n",
        );
        output.push_str("**Verify:** `cargo fmt --all --check`\n");
    }
    if command == "cargo" && errors > 0 {
        output.push_str("**Why:** Cargo/Clippy rejected the code shown above. The stable code and source location come directly from Cargo's structured diagnostic stream.\n\n");
        output.push_str("**How:** fix every occurrence listed above in one pass. Prefer the compiler/Clippy help and repository policy over suppressing the diagnostic.\n\n");
        output.push_str("**Verify:** rerun the same Cargo/Clippy command used by this CI step.\n");
    }
    if command == "file-size" && errors > 0 {
        output.push_str("**Why:** a Rust source file exceeds Sindri's repository size policy.\n\n");
        output.push_str("**How:** split by responsibility; do not compress or reformat code merely to get below the cap. See `docs/module-layout.md`.\n\n");
        output.push_str("**Verify:** `python3 scripts/check-file-size.py`\n");
    }
    output
}

fn group_diagnostics<'a>(diagnostics: &[&'a Diagnostic]) -> BTreeMap<String, Vec<&'a Diagnostic>> {
    let mut groups = BTreeMap::<String, Vec<&Diagnostic>>::new();
    for diagnostic in diagnostics {
        let key = diagnostic
            .code
            .as_ref()
            .map_or_else(|| "uncoded diagnostic".into(), |code| code.0.clone());
        groups.entry(key).or_default().push(*diagnostic);
    }
    groups
}

fn render_summary_diagnostic(output: &mut String, diagnostic: &Diagnostic) {
    let location = diagnostic.location.as_ref().map_or_else(
        || "unknown location".into(),
        |location| {
            format!(
                "{}:{}:{}",
                location.path.display(),
                location.line,
                location.column
            )
        },
    );
    let _ = writeln!(output, "- **Where:** `{location}`");
    let _ = writeln!(output, "  **What:** {}", diagnostic.message);
    if let Some(suggestion) = &diagnostic.suggestion {
        let _ = writeln!(output, "  **Suggested repair:** `{suggestion}`");
    }
    for note in &diagnostic.notes {
        let _ = writeln!(output, "  **Context:** {note}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sindri_diagnostics::{DiagnosticCode, DiagnosticSource, Severity, SourceLocation};

    #[test]
    fn summary_names_every_unformatted_file() {
        let report = DiagnosticReport::new(parse_rustfmt_diff(
            "Diff in crates/a/src/lib.rs:1:\nDiff in crates/b/src/lib.rs:2:\n",
        ));
        let summary = github_summary(&report, "rustfmt");
        assert!(summary.contains("crates/a/src/lib.rs"));
        assert!(summary.contains("crates/b/src/lib.rs"));
        assert!(summary.contains("2 rustfmt error(s)"));
        assert!(!summary.contains("fix every occurrence"));
    }

    #[test]
    fn cargo_summary_groups_all_occurrences_and_keeps_locations() {
        let report = DiagnosticReport::new(vec![
            rust_error(
                "clippy::too_many_lines",
                "first is too long",
                "src/a.rs",
                10,
            ),
            rust_error(
                "clippy::too_many_lines",
                "second is too long",
                "src/b.rs",
                20,
            ),
            rust_error("unused_imports", "unused import", "src/c.rs", 30),
        ]);
        let summary = github_summary(&report, "cargo");
        assert!(summary.contains("3 diagnostic(s) in 2 failure class(es)"));
        assert!(summary.contains("### clippy::too_many_lines (2)"));
        assert!(summary.contains("`src/a.rs:10:1`"));
        assert!(summary.contains("`src/b.rs:20:1`"));
        assert!(summary.contains("fix every occurrence listed above in one pass"));
    }

    fn rust_error(code: &str, message: &str, path: &str, line: u32) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            source: DiagnosticSource::Rust,
            code: Some(DiagnosticCode(code.into())),
            message: message.into(),
            location: Some(SourceLocation {
                path: path.into(),
                line,
                column: 1,
                end_line: None,
                end_column: None,
            }),
            notes: Vec::new(),
            suggestion: None,
            rendered: None,
            relation: None,
        }
    }
}
