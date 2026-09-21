# sindri-diagnostics

Shared structured diagnostics, renderers, and CI failure correlation for Sindri.

The crate turns producer-specific errors from Rust/Cargo, rustfmt, file-size
checks, and Decay into a common `DiagnosticReport`. CI uses that model for
GitHub annotations, stable failure fingerprints, failing-step identity, and
cross-job correlation. It also retains a prose fingerprint fallback for tools
that do not expose structured diagnostics.

The canonical architecture, command reference, preflight workflow, and CI
integration documentation lives in [`docs/diagnostics.md`](../../docs/diagnostics.md).
Repository validation requirements remain authoritative in
[`AGENTS.md`](../../AGENTS.md).

For ordinary development, start with:

```bash
python3 scripts/preflight.py
```

The `sindri-diagnostics` CLI is mainly infrastructure for adapters and CI; the
full command table and input contracts are documented in the canonical guide.
