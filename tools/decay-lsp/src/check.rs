use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use serde_json::json;

use crate::diagnostics::StructuredDiagnostic;

const SKIPPED_DIRECTORIES: &[&str] = &[".git", "node_modules", "target"];

struct ContractReminder {
    offset: usize,
    code: &'static str,
    message: &'static str,
}

pub(crate) fn run(paths: &[PathBuf], json_output: bool) -> io::Result<bool> {
    let mut files = BTreeSet::new();
    for path in paths {
        collect_decay_files(path, &mut files)?;
    }
    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Decay preflight found no .decay files in the supplied paths",
        ));
    }

    let environment = sindri_decay::environment();
    let mut errors = 0;
    let mut reminders = 0;
    let mut output_diagnostics = Vec::new();

    for path in &files {
        let source = fs::read_to_string(path)?;
        let analysis = decay_semantic::analyze_with_environment(&source, &environment);
        for diagnostic in analysis.diagnostics {
            let diagnostic = StructuredDiagnostic::from_compiler(diagnostic);
            let (line, column) = line_column(&source, diagnostic.span.start);
            if !json_output {
                eprintln!(
                    "{}",
                    diagnostic.human(&path.display().to_string(), line, column)
                );
            }
            output_diagnostics.push(diagnostic.json(&path.display().to_string(), line, column));
            errors += 1;
        }

        for reminder in contract_reminders(&source) {
            let diagnostic =
                StructuredDiagnostic::reminder(reminder.code, reminder.message, reminder.offset);
            let (line, column) = line_column(&source, diagnostic.span.start);
            if !json_output {
                eprintln!(
                    "{}",
                    diagnostic.human(&path.display().to_string(), line, column)
                );
            }
            output_diagnostics.push(diagnostic.json(&path.display().to_string(), line, column));
            reminders += 1;
        }
    }

    if json_output {
        println!(
            "{}",
            json!({
                "schemaVersion": 1,
                "success": errors == 0,
                "filesChecked": files.len(),
                "errorCount": errors,
                "reminderCount": reminders,
                "diagnostics": output_diagnostics
            })
        );
    } else {
        eprintln!(
            "Decay preflight: {} file(s), {errors} error(s), {reminders} runtime-contract reminder(s)",
            files.len()
        );
    }
    Ok(errors == 0)
}

fn collect_decay_files(path: &Path, files: &mut BTreeSet<PathBuf>) -> io::Result<()> {
    if path.is_file() {
        if path.extension() == Some(OsStr::new("decay")) {
            files.insert(path.to_owned());
        }
        return Ok(());
    }
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("preflight path does not exist: {}", path.display()),
        ));
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir()
            && SKIPPED_DIRECTORIES.contains(&entry.file_name().to_string_lossy().as_ref())
        {
            continue;
        }
        if file_type.is_dir() || file_type.is_file() {
            collect_decay_files(&entry.path(), files)?;
        }
    }
    Ok(())
}

fn contract_reminders(source: &str) -> Vec<ContractReminder> {
    let mut reminders = Vec::new();
    if let Some(offset) = source.find("World.set_property(") {
        reminders.push(ContractReminder {
            offset,
            code: "runtime-property-write",
            message: "World.set_property only authors a newly spawned entity before its script starts; use a signal or another live-state API for an entity that is already running",
        });
    }
    if let Some(offset) = source.find("World.property_number(") {
        reminders.push(ContractReminder {
            offset,
            code: "authored-property-read",
            message: "World.property_number reads the scene-authored property, not the target script's current runtime field",
        });
    }
    reminders
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let before = source.get(..offset).unwrap_or(source);
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rsplit_once('\n')
        .map_or(before, |(_, current_line)| current_line)
        .chars()
        .count()
        + 1;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::{contract_reminders, line_column};

    #[test]
    fn reports_each_runtime_contract_once_per_file() {
        let source = "World.set_property(entity, \"speed\", 2.0);\n\
                      World.set_property(other, \"speed\", 3.0);\n\
                      let speed = World.property_number(entity, \"speed\");";
        let reminders = contract_reminders(source);

        assert_eq!(reminders.len(), 2);
        assert_eq!(reminders[0].code, "runtime-property-write");
        assert_eq!(reminders[1].code, "authored-property-read");
    }

    #[test]
    fn reports_one_based_source_locations() {
        assert_eq!(line_column("first\nsecond", 6), (2, 1));
        assert_eq!(line_column("first\nsecond", 9), (2, 4));
    }
}
