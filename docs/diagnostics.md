# Diagnostics

Sindri diagnostics are structured at the point where a problem is understood and
rendered later for people, CI, editors, coding agents, and automation. The shared
representation lives in `sindri-diagnostics`.

## North-star acceptance criterion

A competent developer or coding agent with **zero prior Sindri knowledge** should
be able to receive a diagnostic report plus repository access and answer, as far
as the available evidence permits:

1. **What** failed?
2. **Where** is the exact failure?
3. **Why** is it a failure; what expectation or contract was violated?
4. **How** should it normally be repaired?
5. **When** was it introduced or which change made it relevant, when that can be
   established rather than guessed?
6. **What else** is affected or likely to fail for the same root cause?
7. **How do I verify** that the repair is complete?

A report that merely repeats compiler prose is incomplete. A report that hides
source location, stable code, related occurrences, or the authoritative
verification command is incomplete when that information is available.

Diagnostics must distinguish **fact**, **derived fact**, and **guidance**. They
must not invent causality. If Sindri cannot establish why a failure happened, it
should say that the cause is unknown while still preserving the evidence needed
to investigate it.

## Two jobs: prevent, then explain

Diagnostics have two equally important jobs.

### Prevent what can be known before execution

Static/preflight diagnostics should catch repository facts that do not require a
successful build: file-size policy, generated-artifact relationships, changed
Decay scripts and typed preflight scope, known documentation obligations, and
other mechanically knowable repository contracts.

This matters especially to remote coding agents that can inspect and edit the
repository but cannot execute the workspace before pushing. Preflight should
reduce avoidable CI iterations, but it must never pretend static inspection proves
runtime, platform, browser, render, or integration behaviour.

### Make the first executable failure sufficient

When CI is the first place a check can actually run, the first failed run should
contain enough evidence to make the **complete repair pass**, not merely expose
the first symptom. A failure class should therefore be collected across the full
structured stream. If three functions violate the same lint, report all three in
one run. If one compiler error breaks native tests, WASM, and browser checks,
report one root cause and the affected checks rather than three mysteries.

The target is not "nicer logs". The target is to minimize diagnose/push/wait
loops while making each recommendation trustworthy.

## Actionable diagnostic contract

The base `Diagnostic` remains producer-neutral: severity, source subsystem,
message, optional stable code, optional source range, notes, suggestion, original
rendered form, and root-cause relation. `DiagnosticReport` wraps diagnostics with
`schema_version` and a derived success flag. JSON is an interface rather than
reformatted terminal prose, matching `docs/cli-conventions.md`.

For actionable CI/agent presentation, renderers and aggregators should derive or
preserve the following fields whenever evidence exists:

| Question | Required evidence |
| --- | --- |
| What? | severity, stable diagnostic/lint/test code, concise message |
| Where? | repository path, line/column/range, symbol/test identity when known |
| Why? | violated compiler rule, repository policy, generated contract, or test expectation |
| How? | concrete remediation or authoritative generator/fix command; do not recommend suppression when repository policy prefers a real fix |
| When? | branch/diff provenance when mechanically established; otherwise explicitly unknown |
| Impact? | checks, targets, packages, or surfaces affected by this root cause |
| Related? | every known occurrence of the same failure class, including near-threshold/static risks when intentionally computed |
| Verify? | exact command/check that proves the repair |
| Confidence? | factual/derived/suggested status for claims that go beyond producer facts |

Human output should lead with an executive summary: number of independent root
causes, downstream manifestations, repair scope, and the best starting location.
Machine-readable output must preserve the same information without requiring an
agent to parse Markdown or ANSI output.

The diagnostics crate deliberately has no dependency on scene, editor, renderer,
Decay, or game crates. Sindri-specific policy/context may be supplied by callers
or lightweight repository-aware adapters; the shared diagnostic model must not
invert engine dependency boundaries.

## Cargo and Clippy

`parse_cargo_messages` consumes Cargo's newline-delimited
`--message-format=json` stream and converts compiler messages into the common
model. Primary spans become source locations, compiler codes remain stable codes,
help/note children remain notes, and machine suggestions are retained.

Cargo diagnostics must preserve **all actionable diagnostics in the stream**.
Presentation must not reduce a run to only its first error. Aggregation should
group repeated stable codes/lints while retaining each source location. This is
particularly important for Clippy policy failures such as `too_many_lines`: one
run should reveal every offending function reported by Clippy rather than force a
push for each occurrence.

The `sindri-diagnostics cargo` renderer consumes that stream directly. Clippy CI
captures Cargo JSON and, on failure, emits GitHub file annotations and a step
summary while preserving Clippy's original exit status. CI therefore does not
need to scrape ANSI-formatted compiler prose to answer which file and line
failed.

## Formatting and CI correlation

`parse_rustfmt_diff` turns rustfmt's repeated diff blocks into one actionable
diagnostic per affected file with the exact `cargo fmt --all` remedy. If rustfmt
cannot parse a Rust file, the adapter instead preserves that compiler-style
location as `RUST_PARSE_ERROR`; a syntax error is not mislabeled as formatting.

`fingerprint_failure` extracts a compact identity from common CI output. It
prefers Rust compiler code plus source location, then failed test identity, then
a normalized actionable error line. Known runner/network failures get explicit
infrastructure fingerprints. When a producer already has a `DiagnosticReport`,
`fingerprint_report` derives identity directly from structured data.

`correlate_checks` groups check failures by failure fingerprint. Correlation must
be conservative: equal evidence may be grouped; merely similar prose must not be
presented as proven common causality. The first manifestation is primary, later
checks with the same fingerprint are downstream, and infrastructure failures stay
independent. `render_ci_summary` turns those relations into Markdown and should
evolve toward the north-star executive summary above.

## Generated artifacts

Generated-file drift is a first-class diagnostic, not a generic failed test. When
a generator succeeds but checked-in output differs, report:

- the stale generated path(s);
- the authoritative generator command;
- the source/schema change when it can be established mechanically;
- whether regeneration itself succeeded;
- the CI artifact containing exact regenerated output, when available;
- that the failure is repository consistency rather than evidence of a runtime
  defect unless another diagnostic proves otherwise.

Generated files are never hand-edited merely to silence the check.

## File sizes

`scripts/check-file-size.py` remains the authority for the 600-line source-file
cap. Failed output is adapted into `FILE_SIZE_LIMIT` diagnostics with the
violating file, current line count, configured limit, `docs/module-layout.md`, and
the verification command. Where inexpensive, preventive reporting may also flag
changed files approaching the cap, clearly as guidance rather than failure.

Function-size/Clippy limits are separate from the repository file-size cap and
must be reported with the function/symbol identity when Cargo provides it.

## GitHub Actions

`GithubAnnotation::from_diagnostic` renders the common model as a GitHub workflow
annotation. Structured reports are the authority; annotations are navigation
helpers. Step summaries should be useful even when read without opening raw logs.
Raw logs remain evidence of last resort, not the normal interface for diagnosis.

## Decay preflight

The Decay checker exposes structured diagnostics directly, so CI can add source
annotations and derive stable fingerprints without scraping human output. Syntax
and semantic errors plus runtime-contract reminders retain their Decay code,
source location, and typed meaning.

The checker remains authoritative. Rendering diagnostics must never turn a
rejected Decay preflight green. Runtime-contract reminders must be clearly
separated from compile failures while still giving unfamiliar contributors
Sindri-specific direction.

## Agent preflight

`scripts/preflight.py` is the one-command cheap gate for coding agents and local
development. It diffs the merge base with `origin/main` through the current
working tree, including untracked files, maps changed files to Cargo workspace
packages, then runs global rustfmt/file-size checks, typed checking for changed
`.decay` scripts, and `cargo check` plus tests for affected packages. Changes
inside the separate Decay workspace run its fmt, Clippy, and test gates too.
Root workspace/toolchain changes expand Rust checking to the whole workspace.
`--list` exposes discovered scope without executing it.

Preflight deliberately does not guess WASM, render, browser, or dependency impact
as proof. It may explain which additional CI surfaces are expected to matter,
but CI remains authoritative for checks the local/agent environment cannot run.

## Command reference

Run the cheap changed-scope preflight before pushing ordinary changes:

```bash
python3 scripts/preflight.py
python3 scripts/preflight.py --list
```

The diagnostics binary is primarily an adapter and renderer used by CI. Each
subcommand reads its producer format from standard input, keeping producers
independent of GitHub Actions.

| Command | Input | Purpose |
| --- | --- | --- |
| `rustfmt` | `cargo fmt --all --check` output | Distinguish formatting diffs from Rust parse errors and report affected files. |
| `file-size` | `scripts/check-file-size.py` output | Convert line-cap failures into actionable diagnostics. |
| `cargo` | Cargo/Clippy `--message-format=json` stream | Preserve compiler codes, all primary spans, notes, and suggestions. |
| `decay` | `decay-lsp --check --json` report | Adapt typed Decay diagnostics without scraping human prose. |
| `fingerprint` | Unstructured failure log | Derive a compact failure identity when no structured report exists. |
| `report-check-result` | Check name followed by a `DiagnosticReport` JSON document | Produce a serialized CI check result directly from structured diagnostics. |
| `correlate` | JSON array of serialized check results | Classify root, downstream, and independent failures and render the CI summary. |

Renderers that support `--github` emit workflow annotations. Structured renderers
also support `--json`. The producer's exit status remains authoritative.

## Development method for diagnostics

Diagnostics work is driven by **real failure transcripts**. Every CI failure that
required unnecessary investigation or an extra push should become a regression
fixture where practical. The fixture's acceptance question is:

> Would this report let a competent newcomer locate the failure and make the
> complete sensible repair without first asking a Sindri maintainer what the
> error means?

PR #373's atmosphere work is the first audit corpus for this standard. It exposed
at least these gaps to cover with fixtures and implementation:

- source/function identity disappearing from the useful summary;
- repeated `too_many_lines` failures surfacing across successive pushes instead
  of as one repair set;
- generated capability drift being presented primarily as a failed test;
- downstream jobs obscuring the number of independent root causes;
- guidance failing to distinguish a repository-preferred refactor from a lint
  suppression escape hatch.

Do not solve these by hard-coding PR numbers or individual files. Capture the
failure shape and make the rule general.

## Current status

The existing foundation already provides structured Cargo/Clippy and Decay
adapters, rustfmt and file-size diagnostics, GitHub annotations, fingerprints,
infrastructure classification, cross-job correlation, failing-step identity, and
cheap changed-scope preflight.

The next diagnostics phase is **actionability and newcomer comprehension**:
preserve richer evidence, aggregate complete repair sets, add repository-aware
direction without inventing causality, and make CI summaries sufficient for both
humans and coding agents.
