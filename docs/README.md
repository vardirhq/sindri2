# Sindri documentation

Last reviewed against `main`: **2026-09-08**.

The repository contains current product contracts, detailed implementation
references, decision records, measured investigations, and historical audits.
They are all useful, but they answer different questions. Start with the current
contracts below; dated evidence should not override them.

## Start here

- [Project README](../README.md) — product identity, current capabilities, setup,
  and the shortest route into the repository
- [Roadmap](../ROADMAP.md) — dependency-ordered work and acceptance criteria
- [Capabilities](capabilities.md) — detailed evidence for what works today
- [Function matrix](function-matrix.md) — terse Engine / Editor / Decay status
- [Feature integration matrix](feature-integration-matrix.md) — gaps between
  runtime, authoring, scripting, and game proof
- [Changelog](../CHANGELOG.md) — user-visible changes
- [Contributing](../CONTRIBUTING.md) — checks and contribution rules

## Projects, scenes, and assets

- [Project format](project-format.md)
- [Asset foundation](asset-foundation.md)
- [Scene extraction](scene-extraction.md)
- [Scene serialization](scene-serialization.md)
- [Component schema registry](component-schema-registry.md)
- [Prefabs](prefabs.md)
- [Reusable profiles](profiles.md)
- [Static web export](export.md)
- [Versioning](versioning.md)

## Engine model

- [How Sindri does 2D](2d-model.md)
- [Camera semantics](cameras.md)
- [Grid coordinates and projection](grid.md)
- [Physics architecture](physics.md)
- [Platform host](platform-host.md)
- [Rendering frame pipeline](rendering-frame-pipeline.md)
- [Presentation surfaces](rendering-surface.md)
- [Colour handling](rendering-color.md)
- [Transparent rendering](rendering-transparency.md)

## Editor

- [Editor architecture](editor-architecture.md)
- [Editor design QA](../design-qa.md)

The editor audits below are dated investigations. Their resolved findings remain
valuable explanations of how bugs escaped code review, but
[capabilities.md](capabilities.md) is the current feature contract.

- [Initial editor audit](editor-audit.md)
- [Editor authoring audit](editor-authoring-audit.md)
- [Taking the editor to Gather](editor-meets-the-game.md)

## Decay

- [Sindri scripting guide](scripting.md)
- [Decay language reference](../decay/LANGUAGE.md)
- [Decay workspace](../decay/README.md)
- [Editor-first language direction](decay-direction.md)
- [Generated Decay host API](generated/decay-api.md)
- [Generated-document contract](generated/README.md)
- [VS Code extension](../editors/vscode-decay/README.md)

The generated directory is produced by `sindri-capabilities` and must not be
edited by hand.

## Weave

- [Weave guide](weave.md)
- [Weave language reference](weave-reference.md)
- [Real-game migration guide](weave-migration.md)
- [Original proof-of-concept decision](weave-poc.md)
- [Standalone language workspace](../weave/README.md)

The focused `games/weave-poc` example demonstrates individual responsive
rules. Orbital Last Stand is the production-oriented reference using composed
stylesheets for every game screen.

## Showcase

- [Gather showcase contract](gather-showcase.md)

This records which mature Sindri capabilities the flagship game demonstrates,
which improvements belong in its refresh, and which forcing-function work must
remain with Orbital Last Stand.

## Orbital Last Stand

Current implementation status is maintained in:

- [Reference parity](last-stand-reference-parity.md)
- [Visual parity](last-stand-visual-parity.md)
- [Game README](../games/orbital-last-stand/README.md)

The following documents preserve the forcing-function plan, measurements, and
handover context:

- [Recreation plan](orbital-last-stand-plan.md)
- [Vertical-slice audit](orbital-last-stand-audit.md)
- [Ten-minute-run evidence](orbital-last-stand-evidence.md)
- [Parity handover](last-stand-handover.md)

## Measurements and historical inputs

These documents describe a specific repository state, experiment, or predecessor
and are not rolling capability lists:

- [Legacy 2D inventory](2d-inventory.md)
- [Effect scaling measurement](effect-scaling.md)
- [Entity scaling measurement](entity-scaling.md)
- [The first browser run](browser.md)
- [Historical Rust + TypeScript proposal](../PROJECT_OVERVIEW.md)

## Repository policy

- [Feasibility and accepted product constraints](FEASIBILITY.md)
- [Dependency policy](dependency-policy.md)
- [CLI conventions](cli-conventions.md)
- [The isometric baker](isometric-baker.md)
- [Module layout and file-size policy](module-layout.md)
- [Agent guidance](../AGENTS.md)
- [Code of Conduct](../CODE_OF_CONDUCT.md)

## Example notes

- [Textured cube and sprite overlay](../examples/cube/README.md)
- [Shared triangle proof](../examples/triangle/README.md)
- [Graphics Lab](../games/graphics-lab/README.md)
- [Graphics Lab tests](../games/graphics-lab/tests/README.md)
- [Browser smoke tooling](../scripts/browser/README.md)

## Documentation maintenance rule

When behavior changes, update the owning subsystem document and the relevant
status matrix in the same change. Date documents that intentionally describe a
snapshot. Generate machine-derived references rather than hand-editing them.
The website is built from these repository sources by `site/build_docs.py`;
it must never silently substitute a different document when a source is missing.
