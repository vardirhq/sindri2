# Versioning

Sindri may version four separate things, and they do not move together: the
Rust crates, the scene file format, the editor protocol, and an optional browser
embedding SDK. A single version number across all four would force a scene
format bump every time a crate gained a method.

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
than exceptional, and `ROADMAP.md` treats API stability as a first-major-release
criterion, not a foundation one.

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

### Version 4

How a sheet is cut moves out of the components that draw it and beside the image
itself. `sindri.sprite` loses `uv_rect` and gains a fragment on its texture
reference — `textures/tiles.png#floor`; `sindri.sprite_animation` loses `sheet`
and its clips list sprite names rather than cell numbers; `sindri.tilemap` loses
`sheet_columns` and `sheet_rows` and gains a `palette` of names its cells index.

The migration recovers a sprite's cell without being told the grid. A rect of
width `w` is one of `1/w` columns and its `x` says which, so a rect that is a
whole cell of a uniform grid becomes `#n` mechanically. Every rect any scene here
carried was such a cell, because a rect was added for sheets in the first place.
One that is *not* a whole cell has no name in format 4 and cannot be given one
without a sheet to name it in, so it **stops the migration** with a message
saying so rather than quietly changing the picture.

What a migration cannot do is write files beside the textures, because it is
handed a document and not a project. So it emits the names a default slice
produces — cell `n` is called `"n"` — and a migrated scene needs a sheet
declaring the grid it used to carry. The grid to declare is the one being
removed, which is why the removal is where it is written down.

### Version 5

Components a subsystem owns are named for that subsystem. `sindri.grid.navigation`
and `sindri.grid.occupant` replace `sindri.grid_navigation` and
`sindri.grid_occupant`, `sindri.animation.sprite` replaces
`sindri.sprite_animation`, and `sindri.audio.source` replaces `sindri.audio`,
matching the `sindri.physics2d.rigid_body` and `sindri.physics2d.collider` names
2D physics was introduced with. A key is the only thing that moves: the payload
under it is carried across untouched, so a format-4 scene holds exactly the
authored data it held before.

Root-level singletons keep their flat names — `sindri.camera`, `sindri.mesh`,
`sindri.sprite`, `sindri.text`, `sindri.tilemap`, and `sindri.script` are not
owned by a subsystem in the same way, and renaming them would churn every scene
for nothing. (`sindri.text` did move later, in format 8, when the UI became a
subsystem in exactly that sense.)

A scene carrying both spellings of one component is the case that is not
mechanical. The two payloads are different authored data and no choice between
them is reliably the same scene, so the migration **stops** and names the entity
and both keys rather than overwriting one of them.

### Version 6

Perspective camera orientation moved into the entity's ordinary `Transform3D`.
Format 5 stored camera direction separately as `target` and `up` fields inside
`sindri.camera`; format 6 removes both. The camera position is
`Transform3D.position`, its rotation is `Transform3D.rotation`, local `-Z` is
forward, and local `+Y` is up.

The migration reconstructs the old look-at basis and writes the equivalent
quaternion onto the transform. Existing transform scale is preserved. A
malformed or degenerate legacy look-at falls back safely rather than producing a
non-finite camera matrix.

Orthographic cameras were intentionally left unchanged by 5 → 6. At that point
in the format they still meant the old authored screen-overlay camera, so
reinterpreting them as transform-driven world cameras would have changed the
picture rather than migrated it.

### Version 7

Screen-space rendering stops being owned by a scene camera. Sprites in `screen`
space and `sindri.text` use a stable projection derived directly from the
viewport, so UI needs no authored `sindri.camera` entity. This is an ownership
change, not a coordinate change: the screen extent remains the same, which keeps
existing HUD placement intact.

Every format-7 `sindri.camera` is therefore a **world/game camera**. Perspective
and orthographic are projection choices of that same role, and both derive
position and orientation from ordinary `Transform3D`. The current runtime
supports one authored world camera and rejects duplicates explicitly rather than
letting entity iteration order choose the winner.

A format-6 orthographic `sindri.camera` is unambiguous historical data: in format
6 orthographic world cameras did not exist, so that component meant the old
screen overlay. The 6 → 7 migration removes that camera component while
preserving the entity itself and every unrelated field/component it carries.
Format-6 perspective cameras survive unchanged. A new orthographic camera
authored in format 7 is a world camera and is never treated as UI.

### Version 8

The screen half of a scene becomes its own family of components. A sprite is a
thing in the world; a thing on the viewport is `sindri.ui.image`, and
`sindri.text` becomes `sindri.ui.text`.

Format 7 said which by a `space` field on `sindri.sprite`, so one component
meant two things. A screen sprite anchored itself to a viewport edge; a world
sprite was placed by its transform and its anchor decided nothing at all — the
editor hid the field to avoid offering a control that did nothing, which is the
clearest possible statement that these were two components sharing a name. They
are now two components, and an entity is a world object or a UI object by which
of them it carries.

The migration moves each sprite to the component it already behaved as. A sprite
in `screen` space — including one that named no space, since screen was the
default — becomes `sindri.ui.image` with its anchor, tint, layer, and texture
carried across. A `world` sprite keeps its name and loses `space` along with the
`anchor` that never applied to it. `sindri.text` was always screen-space, so
only its key moves.

`sindri.tilemap` loses `space` too, and here the default is the awkward case: a
map that named no space was, by that default, on the screen. Nothing can invent
a format-8 spelling for that — a viewport-anchored grid of tiles is a UI element
rather than a tilemap — so a screen-space map **stops the migration** with a
message naming the entity, rather than being shown quietly relocated into the
world. Saying `"space": "world"` in the format-7 file first is the fix, and it
is the answer for every map anyone has actually authored.

Scripts move with the components: `this.ui_image.tint.a` is what a HUD element
writes where it used to write `this.sprite.tint.a`. See `docs/scripting.md`.

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

## Optional browser embedding SDK

**Not yet decided.** No browser embedding package exists, and it is not a
first-release requirement. Decay is the gameplay language on both native and
browser targets.

If a concrete web-application use case earns a TypeScript package later, the
open question is whether its version tracks the Rust crates and how a WASM
artifact built from one crate version is prevented from loading under a package
built for another. Both are decidable once the binding exists and neither is
decidable now.

## Releases

There is no release process yet, and therefore nothing to validate against.
`CHANGELOG.md` accumulates entries under `## [Unreleased]`; the first release
turns that heading into a version and date. Release and changelog validation is
an open `ROADMAP.md` item deliberately left for when a release exists to check.
