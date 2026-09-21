# Diagnostics

Sindri diagnostics are structured at the point where a problem is understood and
rendered later for people, CI, editors, and automation. The shared representation
lives in `sindri-diagnostics`.

## Contract

A diagnostic has a severity, source subsystem, message, optional stable code,
optional source range, notes, suggestion, and optional original rendered form.
A `DiagnosticReport` wraps diagnostics with `schema_version` and a derived
success flag. JSON is therefore an interface rather than reformatted terminal
prose, matching `docs/cli-conventions.md`.

The diagnostics crate deliberately has no dependency on scene, editor, renderer,
Decay, or game crates. Producers can depend on it when a real integration is
added; the editor and future CLI may consume the same report without either
becoming the owner of another subsystem's errors.

## Cargo and Clippy

`parse_cargo_messages` consumes Cargo's newline-delimited
`--message-format=json` stream and converts compiler messages into the common
model. Primary spans become source locations, compiler codes remain stable codes,
help/note children remain notes, and machine suggestions are retained.

This means CI does not need to scrape ANSI-formatted compiler prose to answer
which file and line failed.

## Formatting and CI correlation

`parse_rustfmt_diff` turns rustfmt's repeated diff blocks into one actionable
diagnostic per affected file with the exact `cargo fmt --all` remedy. If rustfmt
cannot parse a Rust file, the adapter instead preserves that compiler-style
location as `RUST_PARSE_ERROR`; a syntax error is no longer mislabeled as a
formatting request.

`correlate_checks` groups check failures by a caller-supplied failure
fingerprint. The first manifestation is primary, later checks with the same
fingerprint are downstream, and infrastructure failures stay independent. This
models real failures seen while building this crate, where one Rust compiler
error failed several jobs while an artifact-finalization 403 was unrelated.

## File sizes\n\n`scripts/check-file-size.py` remains the authority for the 600-line cap. Failed\noutput is adapted into `FILE_SIZE_LIMIT` diagnostics with the offending Rust\nfile and line count, so the quick CI gate can annotate the file and point to\n`docs/module-layout.md` without changing the script's exit contract.\n\n## GitHub Actions

`GithubAnnotation::from_diagnostic` renders the common model as a GitHub
workflow annotation. The quick CI gate now feeds a failed rustfmt check through
the `sindri-diagnostics rustfmt --github` runner, which annotates every affected
file and writes a concise step summary while preserving rustfmt's original exit
status. This is the first live consumer of the shared diagnostic model; the
producer remains independent of GitHub.

## Decay preflight

The Decay CI gate now preserves the typed checker's full output and, when a
changed script fails, mirrors that output into the GitHub step summary together
with the exact changed-script set and a reproducible local command. This keeps
the checker authoritative while making its failure visible without digging
through the complete job log. A later slice should expose structured Decay
diagnostics directly from the checker so CI can add source annotations without
scraping its human output.

## Next integrations

The first crate establishes the contract and Cargo/Clippy adapter without
changing existing CI while PR #339 owns scene/editor/voxel integration. Follow-up
slices should add:

1. extend the runner beyond the now-integrated rustfmt path so repository checks can emit one summary;
2. richer CI failure fingerprint extraction and cross-job correlation;
3. Decay diagnostics raised directly from the typed checker rather than the current CI summary of checker prose;
4. Cargo/Clippy GitHub summary and annotation emission;
5. editor Problems-panel consumption once the editor work is free to move.

CI remains authoritative until those integrations replace individual workflow
commands and prove equivalent behavior.


### Structured Decay checker output

The Decay checker also has a machine-readable mode:

```bash
cargo run --quiet --package decay-lsp -- --check --json path/to/script.decay
```

It emits one JSON object with `schemaVersion: 1`, overall success, file/error/reminder counts, and a `diagnostics` array. Each diagnostic carries `path`, one-based `line` and `column`, `severity`, stable category `code`, `source`, `message`, and the original Decay byte `span`. The command keeps the same exit contract as human `--check`: compiler errors fail; runtime-contract reminders do not.

The LSP and both checker renderers now adapt from the same structured diagnostic model inside `decay-lsp`. A later slice will move/strengthen the neutral diagnostic contract at the language-tooling seam and feed it into `sindri-diagnostics` and CI annotations without parsing prose.


### Decay CI annotations

The Decay preflight now runs the checker in JSON mode and pipes that structured report into `sindri-diagnostics decay --github`. Syntax and semantic errors, plus runtime-contract reminders, therefore become GitHub file annotations with their Decay diagnostic code instead of requiring a reader to scrape the checker log.

The workflow captures the checker exit status before rendering diagnostics and exits with that original status. The diagnostics renderer is deliberately non-authoritative in CI, so a renderer failure cannot turn a rejected Decay preflight green.
