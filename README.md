# Sindri Next

Sindri Next is the new foundation for a coherent Rust-powered 2D and 3D game engine targeting native applications and modern browsers, with a first-class TypeScript API and an integrated visual editor.

The repository is at its foundation stage. The renderer-independent core currently provides:

- strict engine lifecycle semantics
- capped frame time and fixed-step simulation
- generation-checked runtime entities
- safe entity hierarchies
- versioned, editor-friendly scene documents
- stable serialized IDs kept separate from runtime handles

Read the [project overview](PROJECT_OVERVIEW.md), the [feasibility review](docs/FEASIBILITY.md), and the checkable [roadmap](ROADMAP.md).

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
```

The workspace declares Rust 1.85 as its minimum supported Rust version.

## Status

Sindri Next is pre-alpha. Public APIs and serialized formats may change while the foundation milestones are being proven, but format changes will be explicit and versioned.
