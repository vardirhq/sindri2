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
diagnostic per affected file with the exact `cargo fmt --all` remedy.

`correlate_checks` groups check failures by a caller-supplied failure
fingerprint. The first manifestation is primary, later checks with the same
fingerprint are downstream, and infrastructure failures stay independent. This
models real failures seen while building this crate, where one Rust compiler
error failed several jobs while an artifact-finalization 403 was unrelated.

## GitHub Actions

`GithubAnnotation::from_diagnostic` renders the common model as a GitHub
workflow annotation. The quick CI gate now feeds a failed rustfmt check through
the `sindri-diagnostics rustfmt --github` runner, which annotates every affected
file and writes a concise step summary while preserving rustfmt's original exit
status. This is the first live consumer of the shared diagnostic model; the
producer remains independent of GitHub.

## Next integrations

The first crate establishes the contract and Cargo/Clippy adapter without
changing existing CI while PR #339 owns scene/editor/voxel integration. Follow-up
slices should add:

1. extend the runner beyond the now-integrated rustfmt path so repository checks can emit one summary;
2. file-size adapter and richer CI failure fingerprint extraction;
3. Decay diagnostics raised from the typed checker rather than parsed from prose;
4. Cargo/Clippy GitHub summary and annotation emission;
5. editor Problems-panel consumption once the editor work is free to move.

CI remains authoritative until those integrations replace individual workflow
commands and prove equivalent behavior.
