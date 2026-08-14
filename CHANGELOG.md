# Changelog

All notable changes to Sindri Next will be documented here.

## [Unreleased]

### Added

- Initial Rust workspace with `sindri-core` and the public `sindri` facade.
- Engine lifecycle and renderer-independent runtime host.
- Capped fixed-step clock with pause and spiral-of-death protection.
- Generation-checked entities, safe hierarchy operations, and recursive destruction.
- Versioned scene documents with stable logical IDs and hierarchy validation.
- Extensive checkable roadmap and architecture feasibility review.
- CI checks for formatting, Clippy, tests, and the declared MSRV.
- Shared `sindri-gpu` adapter/device negotiation and surface configuration policy.
- Target-independent `sindri-render` triangle pipeline.
- A single triangle example that targets native desktops and WebGPU browsers.
- Browser-target compilation in CI.

### Changed

- Increased the MSRV from Rust 1.85 to 1.87 to use the current `wgpu` 30 release.
