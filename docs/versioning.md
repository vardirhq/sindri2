# Versioning

Sindri versions four separate things, and they do not move together: the Rust
crates, the scene file format, the editor protocol, and the npm SDK. A single
version number across all four would force a scene format bump every time a
crate gained a method.

Two of the four exist today, and their rules are written down here. The other
two are named so that nobody has to guess whether a policy exists — it does not
yet, deliberately. Writing a versioning policy for an artifact that has not been
built means inventing constraints, and an invented policy is worse than an absent
one because people follow it.

## Rust crates

All workspace crates share one version, set in `[workspace.package]`, and are
released together. Splitting them apart is a decision for when someone depends on
one without the others.

The project is pre-1.0 and follows Cargo's semver rules for `0.x`: **the minor
position is the breaking position.** A breaking change goes to `0.2.0`; an
addition or fix goes to `0.1.1`. Until 1.0, breaking changes are expected rather
than exceptional, and `PROJECT_OVERVIEW.md` treats API stability as a first
major release criterion, not a foundation one.

`sindri-editor`, `sindri-cube`, and `sindri-triangle` are `publish = false`. They
carry the workspace version because they live in the workspace, not because
anyone depends on it.

`sindri-decay` is `publish = false` too, for a different reason: it depends on
the Decay crates, and those are not publishable. Every other engine crate
declares its path dependencies with a version — `{ path = "...", version =
"0.1.0" }` — which is what says "this is a real release and the path is only for
local development". `sindri-decay` cannot say that truthfully yet.

## Decay

The crates under `decay/` are a separate workspace on their own version, `0.0.1`,
and are all `publish = false`.

Nothing there is released, and the version is deliberately below the engine's to
say the language is at an earlier stage than what it plugs into. It moves when
someone decides it should, not with the engine.

Being unpublishable is enforced rather than intended: `cargo deny` refuses a
crate that declares itself publishable while depending on a sibling by path,
because crates.io does not accept path dependencies. Marking them private is the
honest form of what was already true.

### MSRV

The minimum supported Rust version is declared in `rust-toolchain.toml` and
`[workspace.package]`, and is currently 1.95 because `wgpu` 30 and `egui` 0.36
require it. Raising it is a deliberate change with a changelog entry, not a side
effect of adding a dependency. Before 1.0, an MSRV increase accompanies a minor
version bump; after 1.0 it needs its own decision, because by then someone is
pinned to it.

## Scene files

Every `SceneDocument` declares an integer `format_version`, currently
`SCENE_FORMAT_VERSION`. A runtime rejects a version it does not recognise rather
than guessing at the meaning of a document from the future.

The version moves when **a document a previous runtime would accept would now be
read differently**. That is a narrower trigger than it first appears, because the
format already carries unknown data forward:

- Adding a component type does not move it. Unregistered component payloads are
  preserved untouched, so an older runtime round-trips a newer scene without
  losing the new component.
- Adding an optional field to an existing component does not move it, for the
  same reason.
- Changing what an existing field means, removing one, or changing how entities
  or hierarchy are encoded does move it.

### Version 2

The first increase, and the one the migrator was built for. Format 2 replaced
the separate `transform_2d` with the single `transform_3d`, so a 2D transform
migrates to the Z = 0 plane: its angle becomes a quaternion about Z and its
two-component scale gains a Z of 1. Nothing is lost.

The one case that is not mechanical is an entity that carried both transforms,
which format 1 allowed. They described positions in different spaces, so no
merge of them is reliably the same scene; the migration refuses it and names the
entity rather than quietly preferring one and moving something.

### Version 3

Transparent sprites sort by how far they are from the camera rather than by a
`depth` number authored beside them, so the field goes and the transform's Z
takes over the job.

A screen-space sprite's Z did nothing in format 2 — the overlay read only X and
Y — so its `depth` becomes a Z, negated, because the overlay camera looks down
the axis from `+Z` and a greater depth meant further away. The stack comes out
in the order it went in, which the offscreen capture holds to byte for byte.

A world-space sprite already had a Z that placed it, and that Z now orders it as
well, so its `depth` is dropped rather than written over the position. That is
the format change rather than a loss: a sort key that disagreed with where the
sprite actually was is what this version stops allowing. Moving a sprite would
be the one thing a migration must never do quietly, so the migration does not.

A version increase requires a registered `SceneMigrator` step **before** the new
version is written anywhere. The migrator enforces the properties that keep a
chain honest — forward-only, one step per source version, no step targeting an
unsupported version — and `docs/scene-serialization.md` describes the format the
steps operate on.

Canonical serialization is part of the contract, not a formatting preference. The
golden fixtures in `crates/sindri-core/tests/fixtures` are stored in canonical
form so that a change to ordering or layout fails the test suite instead of
silently rewriting every scene in every project. Regenerating them is a
deliberate act, and a diff in those files belongs in the pull request that
explains it.

## Editor protocol

**Not yet decided.** The protocol does not exist; `ROADMAP.md` places a versioned
protocol and capability handshake in Milestone 7, and `docs/FEASIBILITY.md`
requires one before stateful editing.

The open question is whether the protocol version tracks the crate version or
moves independently, which cannot be answered before knowing whether the editor
will ever talk to a runtime it was not built alongside. This section gets written
when the handshake does.

## npm SDK

**Not yet decided.** `@sindri/engine` does not exist; it is Milestone 5.

The open question is whether its version tracks the Rust crates — the SDK is a
hand-written wrapper over bindings generated from them, so a crate change can be
invisible to TypeScript or can break it — and how a WASM artifact built from one
crate version is prevented from loading under a package built for another. Both
are decidable once the binding crate exists and neither is decidable now.

## Releases

There is no release process yet, and therefore nothing to validate against.
`CHANGELOG.md` accumulates entries under `## [Unreleased]`; the first release
turns that heading into a version and date. Release and changelog validation is
an open `ROADMAP.md` item deliberately left for when a release exists to check.
