use crate::{Diagnostic, Severity};
use std::fmt::Write;

/// One GitHub Actions workflow command derived from a structured diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GithubAnnotation(String);

impl GithubAnnotation {
    #[must_use]
    pub fn from_diagnostic(diagnostic: &Diagnostic) -> Self {
        let level = match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note | Severity::Help => "notice",
        };
        let mut metadata = String::new();
        if let Some(location) = &diagnostic.location {
            let _ = write!(
                metadata,
                " file={},line={},col={}",
                escape_property(&location.path.to_string_lossy()),
                location.line,
                location.column
            );
            if let Some(end_line) = location.end_line {
                let _ = write!(metadata, ",endLine={end_line}");
            }
            if let Some(end_column) = location.end_column {
                let _ = write!(metadata, ",endColumn={end_column}");
            }
        }
        if let Some(code) = &diagnostic.code {
            let separator = if metadata.is_empty() { " " } else { "," };
            let _ = write!(metadata, "{separator}title={}", escape_property(&code.0));
        }
        Self(format!(
            "::{level}{metadata}::{}",
            escape_message(&diagnostic.message)
        ))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn escape_message(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(value: &str) -> String {
    escape_message(value)
        .replace(':', "%3A")
        .replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiagnosticCode, DiagnosticSource, SourceLocation};
    use std::path::PathBuf;

    #[test]
    fn renders_annotation_without_location() {
        let diagnostic = Diagnostic {
            severity: Severity::Error,
            source: DiagnosticSource::Build,
            code: Some(DiagnosticCode("BUILD_FAILURE".into())),
            message: "build failed".into(),
            location: None,
            notes: Vec::new(),
            suggestion: None,
            rendered: None,
            relation: None,
        };

        assert_eq!(
            GithubAnnotation::from_diagnostic(&diagnostic).as_str(),
            "::error title=BUILD_FAILURE::build failed"
        );
    }

    #[test]
    fn renders_file_annotation() {
        let diagnostic = Diagnostic {
            severity: Severity::Error,
            source: DiagnosticSource::Rust,
            code: Some(DiagnosticCode("RUST_E0308".into())),
            message: "mismatched types".into(),
            location: Some(SourceLocation {
                path: PathBuf::from("src/lib.rs"),
                line: 7,
                column: 3,
                end_line: Some(7),
                end_column: Some(9),
            }),
            notes: Vec::new(),
            suggestion: None,
            rendered: None,
            relation: None,
        };

        assert_eq!(
            GithubAnnotation::from_diagnostic(&diagnostic).as_str(),
            "::error file=src/lib.rs,line=7,col=3,endLine=7,endColumn=9,title=RUST_E0308::mismatched types"
        );
    }
}
