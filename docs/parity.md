# Parity

Last audited against `main`: **2026-09-09**.

What a game engine is expected to do, what Sindri actually does, and the
distance between them. This file replaces `function-matrix.md` and
`feature-integration-matrix.md`, which are deleted: both graded Sindri against
Sindri, so a feature nobody had thought of had no row and could not show as
missing.

That failure was not hypothetical. The integration matrix recorded 2D physics as
**Ready** — masked runtime, body kinds, sensors, validation, velocity, impulse,
all true — while a character could not be given a capsule with a circle at each
side, because a collider was singular from the scene file down to
`BodyRecord2d`. "Ready" meant *complete against what we built*, not *sufficient
for a game*. A table with an outside edge would have carried a `Compound
colliders` row reading ❌ from the first day it existed.

So this table is written from the outside in. The baseline column is what a
person arriving from Unity or Godot expects to find. Rows exist for things we
have never built and have no plan to build, because a gap with no row is a gap
nobody schedules.

## How to read it

Four surface columns, then the outward judgement:

| Column | Question |
| --- | --- |
| **Engine** | Does the runtime do it? |
| **Editor** | Can somebody author it without editing JSON by hand? |
| **Decay** | Can a script reach it? |
| **Proof** | Does a real game in this repository use it? |

**Surface legend:** ✅ works and is exercised · 🟡 a useful slice, with the gap
named · ❌ absent · — genuinely not applicable to that surface.

**Parity legend:**

- **Ahead** — we do this better than the baseline, on purpose. Protect it.
- **Par** — a competent equivalent exists.
- **Behind** — the same idea, less of it.
- **Absent** — the baseline has it and we have nothing.
- **Won't** — a deliberate decision not to have it. See the anti-goals.

A ✅ still means what it always meant: implemented *and* exercised, not
represented by a type, a schema, or an editor control. For a gameplay
capability, exercised means a game uses it.

**Ranking is by "does this stop somebody shipping a game", not by feature
count.** Unity has thousands of features and most of them do not matter. The
ranked queue at the end of this file is the actionable output; the tables are
the evidence behind it.

## Where Sindri is already ahead

Written down first, and deliberately, because parity work is the most likely
way to lose these. Every row below is somewhere copying the baseline would be a
regression.

| We do | They do | Why ours is better |
| --- | --- | --- |
| Components self-describe through `ComponentSchemaRegistry`; templates are checked against serde at startup | Unity's inspector is hand-written per component; drift is found by a user | A template that has drifted from its struct is a startup error, not a row quietly missing from a panel a release later |
| Capability rule: docs and a real game use, in the same change | Features ship, then rot unproven | `capabilities.md` describes what a game actually did, not what an API allows |
| Scenes are readable single files; no sidecars, no GUIDs | `.meta` files, GUID churn, unmergeable scenes | Two people can edit a scene and resolve it in a normal diff |
| Determinism by default: a written-out PCG stream, replayable from a seed on every host, no platform entropy | `UnityEngine.Random` is not contractually stable across versions or platforms | A seed reproduces a run, which is what a roguelike and a bug report both need |
| Decay and Weave designed alongside the engine | C# bolted to a C++ core; UI Toolkit retrofitted after two prior UI systems | One scripting surface and one presentation model, both generated from the same registry |
| Weave: responsive stylesheets for game UI, composed and hot-reloaded | No native equivalent; USS is the nearest and arrived late | Game UI that adapts to a phone and a desktop from one authored source |
| Isometric and grid gameplay in the engine: footprints, occupancy, walls, A\* over whole footprints | Plugins (A\* Pathfinding Project, tile extensions) | The thing most 2D games need is not a third-party dependency |
| Validation refuses at the boundary and names what failed, including the index of the offending collider piece | Silent clamping, silent nulls | A bad value is a named error, not a mystery at frame 4000 |
| Fixed-step loop with input edges consumed exactly once | Update/FixedUpdate confusion, input polled in the wrong phase | Input cannot be missed or double-counted |

## What we will not copy

The advantage of a foundation this new is that its mistakes are still
avoidable. These are anti-goals; a PR that drifts toward one should be stopped
by this list rather than discovered a year later.

- **No sidecar metadata files or asset GUIDs.** Identity comes from the path
  inside the project. Renames are the price; unmergeable scenes are not.
- **No serialization by field-name reflection.** Renaming a Rust field must not
  silently drop authored data. Schema versions and explicit decoding instead.
- **No second render pipeline.** Unity has three that split the ecosystem in
  half. One path, extended.
- **No second input system.** The action layer already in `sindri-platform`
  becomes *the* input system; it does not grow up beside the old one.
- **No prefab variant hierarchy.** Prefabs plus overrides through the reference.
  If nesting variants ever seems necessary, prove it with a game first.
- **No assembly-definition graph.** Crate boundaries in `dependency-policy.md`
  already do this job.
- **No inspector that needs a plugin to be usable.** Odin exists because
  Unity's is not sufficient. Ours has to be sufficient — see the ranked queue.
- **No editor-only code path.** What the editor plays is what the build runs.
- **No silent defaults for things the engine cannot invent.** A component that
  names a font, a sheet, or a clip has no blank; the registry already
  distinguishes a field template from a fresh default, and that distinction stays.

---

## Stranded capability

A class the old matrices could not express: **built, working, and unreachable.**
Neither of these appeared in either matrix, in any column, because nobody had
written the row.

| Capability | State | Why it is stranded |
| --- | --- | --- |
| **Bloom / post-processing** | `crates/sindri-render/src/bloom/` has `BloomSettings`, a chain, and a WGSL shader | Nothing in `sindri-scene`, the editor, or Decay references it. A game cannot turn it on. |
| **Input actions** | `crates/sindri-platform/src/input/action/` parses an action document, binds named actions to keys, pointer axes and scroll | No scene component, no editor surface, no Decay binding. Games poll raw keys instead. |
| **Scroll input** | `Source::ScrollX/ScrollY` are bound and parsed | Never surfaced to Decay, so no game can use a wheel. |
| **Gamepad bindings** | `"gamepad.South"` parses as a binding *name* | `Source::from_name` returns `None` for it — an explicit, tested hole. A gamepad is unimplemented, not merely unbound. |

Stranded capability is the cheapest work in this file: the engine cost is
already paid and only the reach is missing. It is also the most invisible, which
is why it earns a section rather than a footnote.

---

## Scenes, entities, and prefabs

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Entities and hierarchy | ✅ | ✅ | ✅ | ✅ | **Par** | Undo entries for script writes |
| Tags and queries | ✅ | ✅ | ✅ | ✅ | **Par** | Query by more than one tag |
| Prefabs | ✅ | 🟡 | ✅ | ✅ | **Behind** | Nothing makes a prefab from a selection; editing a prefab does not update its instances |
| Reusable data profiles | ✅ | ✅ | ✅ | ✅ | **Ahead** | Unity has no native equivalent; ScriptableObject is close but needs code per asset. Optional schemas when a second catalog proves the shape |
| Transform | ✅ | ✅ | 🟡 | ✅ | **Par** | No structured vector or rotation value in Decay |
| Scene save / load | ✅ | ✅ | — | ✅ | **Par** | Readable single file, which is **Ahead**; see the advantages table |
| **Multiple scenes / additive loading** | ✅ | ❌ | ✅ | ❌ | **Behind** | `World::add_scene` loads a scene beside the ones a world already holds; `LoadedScenes` keeps which one is played and switches between them, leaving everything a scene holds as the player left it. `Scene.go`/`Scene.current` let a script ask. Stable IDs are namespaced, so two interiors may each author a `door`. No project-level scene list and no editor surface, and a world holding several scenes does not round-trip through `to_scene` |
| Scene streaming / Addressables | ❌ | ❌ | ❌ | ❌ | **Absent** | Not urgent at 2D scale, but named so it is not a surprise |
| Undo / redo | ✅ | ✅ | — | — | **Par** | Command-backed; script writes are outside it |

## Assets and the import pipeline

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Asset IDs, manifests, async load | ✅ | ✅ | 🟡 | ✅ | **Par** | No general typed asset value in Decay |
| Project browser: import, rename, copy, delete, preview | — | ✅ | — | ✅ | **Par** | — |
| Content-hashed static export | ✅ | ❌ | — | ✅ | **Par** | No editor export workflow |
| Hot reload | ✅ | 🟡 | 🟡 | ✅ | **Ahead** | Native reload works; Unity's domain reload is a byword for slow |
| **Per-asset import settings** | ❌ | ❌ | — | — | **Absent** | No filter mode, mip, wrap, or compression choice. Every texture is imported one way |
| **Texture compression** | ❌ | ❌ | — | — | **Absent** | Ships raw. Fine at current scale, a problem for a real download |
| **Build-time atlas packing** | ❌ | 🟡 | — | — | **Behind** | The slicer authors sheets by hand; nothing packs loose sprites automatically |
| `referenced_audio` gather | ❌ | — | — | — | **Behind** | Hosts cannot infer every clip a scene needs, unlike `referenced_textures` |

## 2D rendering

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Sprites, sheets, UVs, layers, blending | ✅ | ✅ | 🟡 | ✅ | **Par** | Decay has sprite asset and visibility paths only |
| Sheet-declared ground anchor | ✅ | ✅ | — | ✅ | **Ahead** | Says where a sprite meets the ground, so sorting is authored rather than guessed |
| Procedural shapes | ✅ | 🟡 | ✅ | ✅ | **Ahead** | Instanced with sprites; Unity needs a plugin or a mesh. No point-handle authoring |
| Tilemaps (ortho + iso, overhang) | ✅ | ✅ | ✅ | ✅ | **Par** | `Grid.tile`/`set_tile` read and write cells; `Grid.columns`/`rows` give the size |
| **Autotiling / rule tiles** | ❌ | ❌ | ❌ | ❌ | **Absent** | Unity and Godot both ship it. Painting a wall run by hand is the daily cost |
| **Tilemap collision** | ❌ | ❌ | ❌ | ❌ | **Absent** | Grid walls serve pathfinding, not physics. A tilemap generates no colliders |
| **2D lights and shadows** | ❌ | ❌ | ❌ | ❌ | **Absent** | URP 2D lights, Godot's CanvasModulate + Light2D. Nothing here |
| **Custom shaders / materials** | ❌ | ❌ | ❌ | ❌ | **Absent** | No material asset, no shader authoring. The single biggest ceiling on visual identity |
| Bloom | 🟡 | ❌ | ❌ | ❌ | **Behind** | Built and stranded — see above |
| **Post-processing stack** | 🟡 | ❌ | ❌ | ❌ | **Behind** | Bloom only, and unreachable |
| **Nine-slice sprites** | ❌ | ❌ | — | — | **Absent** | Every UI panel that resizes needs it |
| **Sprite masking / stencil** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |
| Sorting and draw order | ✅ | ✅ | 🟡 | ✅ | **Par** | Layers plus the ground anchor |
| **Camera follow / confine / shake** | ❌ | ❌ | ❌ | ❌ | **Absent** | Cinemachine is the most-used Unity package there is; Unity bought it. Games hand-roll it today |

## 3D rendering

Honest summary: 3D is a foundation, not a feature. It is listed so the size of
the gap is legible, not because it is scheduled.

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Cameras, depth, cube primitive, textured mesh | 🟡 | 🟡 | ❌ | ❌ | **Behind** | Foundation only |
| **glTF / model import** | ❌ | ❌ | ❌ | ❌ | **Absent** | `tools/isometric-baker` renders models to 2D sprites offline, in three views; that is not runtime 3D |
| **Materials** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |
| **Lighting** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |
| **Skeletal animation** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |
| **3D physics** | 🟡 | ❌ | ❌ | ❌ | **Absent** | A Sindri-owned data model exists; no runtime |

## Animation

Still the weakest major system relative to the baseline, but no longer the one
where nothing connects. Sprite clips exist, play, and are now *chosen by
gameplay*: a script plays, stops, restarts and times a clip, and is told when a
one-shot has ended. What remains absent is everything above a single clip —
events, blending, tweening, and animating anything that is not a sprite frame.

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Sprite frame clips, timing, loop state | ✅ | ✅ | ✅ | ✅ | **Par** | `Animation.play`/`stop`/`restart`/`is_finished`/`frame`/`clip`/`set_speed`. Gather's player runs its walk cycle only while walking; Orbital's mine blast plays once and despawns itself when it ends |
| Clip authoring and preview | — | ✅ | — | ✅ | **Par** | — |
| **Animation events (a frame fires a callback)** | ❌ | ❌ | ❌ | ❌ | **Absent** | Footsteps, hit frames, spawn-on-frame all need it |
| **Property animation (animate any component field)** | ❌ | ❌ | ❌ | ❌ | **Absent** | Unity's Animation window animates any serialized property. We animate sprite frames and nothing else |
| **Tweening / easing** | ❌ | ❌ | ❌ | ❌ | **Absent** | DOTween is the most-installed Unity asset in history. Every menu, pickup pop, and screen transition wants it |
| **State machine / blending** | ❌ | ❌ | ❌ | ❌ | **Absent** | Animator controller, Godot AnimationTree. Transitions are hand-written today |
| **Skeletal / cutout 2D animation** | ❌ | ❌ | ❌ | ❌ | **Absent** | Spine, Unity 2D Animation. Frame sheets only |
| **Timeline / cutscenes** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |

## Physics and collision

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Bodies, fixed-step stepping, velocity, impulse | ✅ | 🟡 | ✅ | ✅ | **Par** | — |
| Compound colliders (several pieces, one body) | ✅ | ✅ | — | ❌ | **Par** | Pieces are added, removed, reordered and fully edited, a piece's shape included. No game authors one yet |
| Masks, sensors, collision events | ✅ | 🟡 | ✅ | ✅ | **Par** | — |
| **Named collision layers** | ❌ | ❌ | ❌ | — | **Behind** | Masks are raw `u32` bit values. Unity and Godot both name layers in project settings. Cheap to fix, daily friction |
| Per-piece validation naming the failing index | ✅ | — | — | ✅ | **Ahead** | Neither baseline tells you *which* collider was wrong |
| **Raycast / overlap / shape queries** | ❌ | ❌ | ❌ | ❌ | **Absent** | Line of sight, ground checks, click-to-select in-game, AI vision. Close to universal, and we have none |
| **Joints and constraints** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |
| **Physics materials as assets** | ❌ | ❌ | ❌ | — | **Behind** | Friction and restitution are per-collider literals |
| **Character controller** | ❌ | ❌ | ❌ | ❌ | **Absent** | Every platformer and top-down game writes one |
| **Continuous collision (CCD)** | ❌ | — | — | — | **Absent** | Fast bullets tunnel |
| **Collider gizmos in the Scene view** | — | ❌ | — | — | **Absent** | Colliders are invisible while authoring. With compound pieces this is now worse, not better |

## Navigation and grids

Sindri's strongest domain relative to the baseline.

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Orthogonal and isometric grids | ✅ | ✅ | ✅ | ✅ | **Ahead** | First-class, not a plugin |
| Footprints, occupancy, walls | ✅ | ✅ | ✅ | ✅ | **Ahead** | — |
| A\* over whole footprints | ✅ | ✅ | ✅ | ✅ | **Ahead** | Unity needs A\* Pathfinding Project for the equivalent |
| **Navmesh / off-grid navigation** | ❌ | ❌ | ❌ | ❌ | **Absent** | Grid only. Fine for the games we build; a wall for a free-movement game |
| **Steering / avoidance** | ❌ | ❌ | ❌ | ❌ | **Absent** | Agents walk through each other |
| Per-path costs and policies | ❌ | ❌ | ❌ | — | **Behind** | One uniform cost |
| **Viewport wall painting, height tools** | — | 🟡 | — | ✅ | **Behind** | Inspector authoring only |

## Audio

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| WAV / Ogg / MP3, native + browser + silent backends | ✅ | 🟡 | ✅ | ✅ | **Par** | Scene-source preview is not integrated |
| Play, loop, pause, resume, stop | ✅ | 🟡 | ✅ | ✅ | **Par** | — |
| Per-play volume | ✅ | — | 🟡 | ✅ | **Behind** | — |
| **Buses / mixer / master volume** | ❌ | ❌ | ❌ | ❌ | **Absent** | **A settings screen cannot offer a music slider.** Near-universal shipping requirement |
| **Spatial audio / panning** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |
| **Per-voice handles** | ❌ | — | ❌ | — | **Behind** | A script cannot stop the specific sound it started |
| **Music transitions / crossfade** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |

## Input

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Keyboard, mouse, touch behind the platform boundary | ✅ | ✅ | ✅ | ✅ | **Par** | — |
| Unified pointer, bounded fingers | ✅ | ✅ | ✅ | ✅ | **Par** | — |
| Touch stick built from a finger | ✅ | — | ✅ | ✅ | **Ahead** | A considered solution to a problem most engines leave to the game |
| **Action mapping (named actions, rebindable)** | 🟡 | ❌ | ❌ | ❌ | **Behind** | Exists and is stranded. This becomes the input system rather than growing beside it |
| **Gamepad** | ❌ | ❌ | ❌ | ❌ | **Absent** | Binding names parse; `Source::from_name` returns `None`. A tested hole |
| **Scroll wheel** | 🟡 | — | ❌ | ❌ | **Behind** | Bound in the action layer, never surfaced |
| **Rebinding UI** | ❌ | ❌ | ❌ | ❌ | **Absent** | Rewired sells on this |

## UI and presentation

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Anchored image and text, fill bars, buttons, row/column, safe area | ✅ | ✅ | ✅ | ✅ | **Par** | — |
| Pointer hit-testing, hover/press/held | ✅ | ✅ | ✅ | ✅ | **Par** | — |
| Weave responsive stylesheets | 🟡 | 🟡 | — | ✅ | **Ahead** | No baseline equivalent. Stylesheet source editing and viewport presets remain external |
| Project fonts | ✅ | ✅ | — | ✅ | **Par** | — |
| **Slider, toggle, dropdown, text input** | ❌ | ❌ | ❌ | ❌ | **Absent** | **No settings screen can be built.** Buttons and images only |
| **Scroll region** | ❌ | ❌ | ❌ | ❌ | **Absent** | No inventory, no credits, no long list |
| **Rich text** | ❌ | ❌ | ❌ | ❌ | **Absent** | No colour or emphasis inside a string. TextMeshPro was absorbed for exactly this |
| **World-space text** | ❌ | ❌ | ❌ | ❌ | **Absent** | Damage numbers, name plates |
| **Keyboard/gamepad focus navigation** | ❌ | ❌ | ❌ | ❌ | **Absent** | A menu cannot be driven without a pointer |
| **Accessibility labels** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |
| **Drag and drop** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |

## Effects

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Pooled fleck bursts, batched with sprites | ✅ | 🟡 | ✅ | ✅ | **Behind** | Measured and fast, but it is one effect shape |
| **Particle system (emitters, curves, shapes)** | ❌ | ❌ | ❌ | ❌ | **Absent** | No emission shape, lifetime curve, colour ramp, sub-emitter, or trail |
| **Effect preview in the editor** | — | ❌ | — | — | **Absent** | Authored blind through the generic inspector |
| **Trails / line renderer** | ❌ | ❌ | ❌ | ❌ | **Absent** | — |
| **Screen-space feedback (shake, flash, hitstop)** | ❌ | ❌ | ❌ | ❌ | **Absent** | The `Feel` package sells on precisely this |

## Scripting (Decay)

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Typed host, compile / run / reload | ✅ | 🟡 | ✅ | ✅ | **Par** | Source editing stays external |
| Safe entity, prefab, profile references | ✅ | ✅ | ✅ | ✅ | **Ahead** | A stale handle is refused, not a null dereference |
| `@export` properties discovered and authored | ✅ | ✅ | ✅ | ✅ | **Par** | — |
| Language server: highlight, complete, hover, definitions, diagnostics | — | 🟡 | ✅ | ✅ | **Par** | VS Code only |
| **Structured vectors and rotations** | ❌ | — | ❌ | — | **Behind** | Scripts do component maths on loose numbers |
| **Coroutines / sequencing** | ❌ | — | ❌ | — | **Absent** | "Wait a second, then do this" is manual state |
| **Debugger (breakpoints, stepping)** | ❌ | ❌ | ❌ | — | **Absent** | `print` debugging only |
| **Formatter** | ❌ | ❌ | ❌ | — | **Absent** | — |
| **In-editor script editing** | — | ❌ | — | — | **Behind** | Scripts are created and previewed read-only; editing is external |
| **Signals / event bus** | ❌ | ❌ | ❌ | ❌ | **Absent** | Godot's signals are a headline feature. Scripts poll |
| **Script unit tests** | ❌ | ❌ | ❌ | ❌ | **Absent** | No way to test a `.decay` file without running a game |

## Editor experience

Where the most day-to-day friction is, and where one architectural change
removes most of it.

### The root cause of hand-typed fields

The editor does infer meaning for some fields, and it is worth being precise
about how, because the mechanism is the defect rather than its absence.

Three lookup tables in the editor guess a field's meaning from its **name**:

```rust
// editor/src/native/inspector_panel/field.rs
match (type_name, key) {
    (_, "texture") => Some(assets.textures),
    (_, "font")    => Some(assets.fonts),
    (_, "clip")    => Some(assets.audio),
    ("sindri.script", "source") => Some(assets.scripts),
    _ => None,
}
```

plus `is_colour`, which requires the key to be literally `tint`, `color` or
`colour`, and `editor/src/inspector/choices.rs`, which maps `(type_name, key)`
to an enum's spellings. The `choices` module is careful — it takes each list
from the engine's own constants rather than repeating them — but it is still a
table in the editor keyed by field name.

Four consequences follow, and they are why more table entries is not the fix:

1. **The bare-key rules are global.** `(_, "texture")`, `(_, "font")` and
   `(_, "clip")` match *any* component, including one a game brings of its own.
   A game component with a `clip` field is offered the audio list whether or not
   it holds audio.
2. **Anything absent from the table is a free-text box, silently.** A field
   named `sheet`, `icon` or `portrait` gets nothing. This is what happens to
   every component added since the table was written, which is why new work
   keeps landing as raw fields.
3. **A colour must be spelled `tint`.** Named anything else it is four number
   boxes; and the check is `Numbers(4)`, which a UV rect and a quaternion also
   satisfy.
4. **It lives in the editor.** `sindri-capabilities` cannot document it, and no
   other tool can use it.

Underneath all four: meaning is *guessed by the consumer* rather than *declared
by the component*. That is a second copy of knowledge about a component living
away from the component — exactly the drift `check_template` was written to stop
for field lists. `ComponentSchemaRegistry` stores a field template, a
`serde_json::Value` exemplar checked against what serde asks the type for, so it
captures each field's **shape** and nothing about its **meaning**. A texture id
and a display label are both `String` to the registry.

A second consequence used to follow, and is now fixed. A value that is an array
of objects fell to `ValueKind::Opaque` and was displayed as stored — the
`pieces` array of a compound collider is exactly that shape, so compounds were
authorable in a scene file and not in the inspector. The template's exemplar
item is what closed it: it says what a piece consists of, what each field means,
and what a fresh one is, so the panel can draw a list and add to it without
inventing anything.

The fix is to move meaning to the registration — asset(texture), asset(script),
asset(clip), entity reference, choice, colour, angle, bounded range, list-of —
and have the editor read it instead of guessing. It extends the registry that
already exists, makes the knowledge checkable against the template the way field
lists already are, travels to every tool rather than only the editor, and fixes
components not yet written. It is the highest-leverage item in this file.

| Feature | Editor | vs. baseline | Gap that matters |
| --- | :-: | --- | --- |
| Scene view, hierarchy, generic inspector, project browser | ✅ | **Par** | — |
| Gizmos: transform, snapping, Z-lock-safe movement | ✅ | **Par** | No collider, camera, or effect gizmos |
| Play / pause / stop / single-step, snapshot restore | ✅ | **Ahead** | Single-step and snapshot restore are better than Unity's play mode |
| Tilemap painting, sheet slicer, texture picker | ✅ | **Par** | — |
| **Asset pickers for schema fields generally** | ✅ | **Ahead** | Declared per component in the schema registry, checked against the field template, and carried in `docs/generated/`. Unity needs a plugin (Odin) for the equivalent |
| **Array-of-object editing** | ✅ | **Par** | A list of objects is added to, removed from and reordered, with each item's fields drawn through its meanings. Decided by the template, so a tilemap's thousand tiles stay a readout |
| **Tagged-enum (variant) fields** | ✅ | **Par** | A field that decides what else its object holds is switched as one edit, at any depth — Godot's equivalent is swapping a Resource subtype. What is ours is that every variant is proved to decode at startup, so an unpickable one fails the build rather than the scene |
| Console / log panel | ✅ | **Par** | Every failure the editor reports, plus script `print` named by the entity that printed it, filtered by level, repeats collapsed to a count, with a jump to the entity a line is about and an error count in the status bar. It can now be put wherever it is wanted — it opens beside the Scene view — so watching the log no longer costs the project browser, and a failure recurring every frame is one counted line wherever it sits in the log rather than a new line per frame |
| **Profiler view** | ❌ | **Absent** | Where a fixed step goes is unmeasurable in-editor |
| **Search / filter in hierarchy or project** | ❌ | **Absent** | Painful past a few dozen entities |
| **Project settings surface** | ❌ | **Absent** | `sindri.toml` is edited by hand |
| **Build / export UI** | ❌ | **Absent** | Export is CLI-only |
| **Prefab creation from a selection** | ❌ | **Absent** | — |
| **Save inspector** | ❌ | **Absent** | Persistence is play-testable but not viewable |
| **Multi-select and bulk edit** | ❌ | **Absent** | — |
| **Customisable layout** | ✅ | **Par** | Every panel is a tab, draggable into any of seven docks or four scene-anchored overlays — edges dock, corners float — with the arrangement and every size persisted. Three presets to start from. Behind Unity and Unreal only in that a panel cannot yet be torn off into a window of its own |
| **Canvas-first workspace** | ✅ | **Ahead** | The scene view is the document and panels overlay its corners, rather than the viewport being the rectangle left over when the docks have taken theirs. Unity and Godot have no equivalent posture; the nearest comparison is a design tool. Still to come: command palette, chrome collapse, and the Game view as an anchored thumbnail — see `docs/editor-direction.md` |
| **Guided local-AI setup** | ✅ | **Ahead** | A panel that takes a machine with nothing installed to a verified local model without asking anyone to type. No comparable engine ships one: Unity and Unreal have no local-model story, and every third-party assistant starts from "install this yourself and paste a key" |
| **Command palette** | ✅ | **Ahead** | Ctrl+K over panels, entities, project files, scenes, arrangements and verbs, ranked by a scored subsequence match with multi-term search. Unity has no equivalent; Unreal's is command-only and Godot's is files-only |
| **Customisable shortcuts** | ❌ | **Behind** | Keys are fixed in `native/shortcuts.rs` |
| **Live edit while playing** | ❌ | **Behind** | Stop restores the world wholesale, so a value tuned during a run is lost. The largest single cost in the change-and-feel loop; see `docs/editor-direction.md` |
| **Record and scrub a run** | ❌ | **Absent** | The step is fixed and deterministic, so this is available rather than aspirational, and nothing offers it |
| **In-editor script editing** | ❌ | **Behind** | — |

## Build, export, and platform

| Feature | Engine | Editor | Decay | Proof | vs. baseline | Gap that matters |
| --- | :-: | :-: | :-: | :-: | --- | --- |
| Static web export, base-path aware, content-hashed | ✅ | ❌ | — | ✅ | **Ahead** | Genuinely simple next to Unity's WebGL output |
| Native desktop | ✅ | ✅ | — | ✅ | **Par** | — |
| Browser / WASM | ✅ | — | — | ✅ | **Par** | — |
| Versioned `sindri.toml`, validation | ✅ | 🟡 | — | ✅ | **Par** | — |
| **Native packaging (installer, icon, splash)** | ❌ | ❌ | — | ❌ | **Absent** | A desktop build cannot be shipped to a player as-is |
| **Mobile targets** | ❌ | ❌ | — | ❌ | **Absent** | Touch input exists; a build does not |
| **Console targets** | ❌ | ❌ | — | ❌ | **Won't** | Not a realistic target and should not pretend to be |

## Diagnostics

| Feature | State | vs. baseline | Gap that matters |
| --- | :-: | --- | --- |
| Named validation errors at the boundary | ✅ | **Ahead** | — |
| Deterministic replay from a seed | ✅ | **Ahead** | Reproducing a bug is a seed, not a video |
| In-editor console | ✅ | **Par** | See the editor section: it exists, and the gap is its placement rather than its content |
| **Profiler / frame timing** | ❌ | **Absent** | `docs/effect-scaling.md` measured by hand, once |
| **Debug draw from scripts** | ❌ | **Absent** | A script cannot draw a line to show what it thinks it is doing |
| **Frame / draw-call debugger** | ❌ | **Absent** | — |
| **Crash and error reporting in a shipped build** | ❌ | **Absent** | — |

## Absent whole systems

Named so they are decisions rather than oversights.

| System | Status | Position |
| --- | --- | --- |
| **Localization** | **Absent** | No string table, no locale switch. Needed before any non-English release |
| **Networking / multiplayer** | **Won't**, for now | A large system that no planned game needs. Revisit only with a game that requires it |
| **Video playback** | **Absent** | Rarely load-bearing for 2D games |
| **Visual scripting** | **Won't** | Decay is the answer. A second authoring path would split the ecosystem |
| **In-game console / cheats** | **Absent** | Cheap, and useful for testing |
| **Object pooling (general)** | **Behind** | Effects pool internally; nothing general exists |
| **Analytics / telemetry** | **Won't** | Not our business to add |

---

## What the Unity Asset Store proves

A plugin that sells for a decade is a feature the engine should have had. This
is a market-validated gap list, and it is more honest than our own judgement
because somebody paid for every row.

### Tier 1 — Unity had to absorb it

The strongest possible evidence: Unity bought or cloned these, conceding they
were native gaps. Treat these rows as close to automatic.

| Package | What it fixed | Sindri |
| --- | --- | --- |
| **TextMeshPro** | Text quality, rich text | **Absent** — no rich text |
| **Cinemachine** | Camera follow, framing, confining, shake | **Absent** — hand-rolled per game |
| **Post Processing Stack** | Bloom, colour grading, vignette | **Behind** — bloom exists and is stranded |
| **Shader Graph** | Shader authoring without code | **Absent** — no materials at all |
| **Input System** | Action mapping, rebinding, gamepad | **Behind** — action layer stranded, gamepad unimplemented |
| **Addressables** | Asset streaming and release | **Absent** — not urgent at our scale |
| **ProBuilder** | In-editor geometry | **Won't** — 3D is not the product |

### Tier 2 — still not native, still selling

| Package | What it fixes | Sindri |
| --- | --- | --- |
| **Odin Inspector** | Unity's inspector is not sufficient for real data | **Behind** — and our anti-goal is that ours must be sufficient without one. See the editor root cause |
| **DOTween** | Tweening and easing | **Absent** — the highest-value small feature in this file |
| **A\* Pathfinding Project** | Real pathfinding | **Ahead** — ours is native |
| **Behavior Designer** | Behaviour trees, AI authoring | **Absent** — AI is hand-written Decay |
| **Rewired** | Input mapping and rebinding | **Behind** — as above |
| **Feel / NiceVibrations** | Game juice: shake, hitstop, flash | **Absent** |
| **Master Audio** | Buses, mixing, music transitions | **Absent** — no mixer |
| **Easy Save** | Save/load that works | **Ahead** — versioned, atomic, damaged-state aware, three backends |
| **Spine / 2D Animation** | Skeletal 2D | **Absent** |
| **Dialogue System** | Branching conversation | **Absent** — a profile catalog could carry the data today |
| **Localization packages** | String tables | **Absent** |
| **Peek / editor productivity** | Search, navigation, bulk edit | **Absent** |

Read together, the two tiers say the same thing three times: **camera, tweening,
and input mapping** are the features people reliably pay to add. Two of the
three we have partially built and stranded.

---

## The ranked queue

Ordered by whether it stops somebody shipping a game, not by size. This is the
output of the file; everything above is evidence.

1. ~~**Field meaning in the schema registry, and array-of-object editing.**~~
   **Done.** A component says what its fields are for — asset kind, choice,
   colour, angle, bounded range, collision mask, entity reference — and the
   registry checks every path against the field template, so a renamed field is
   a startup error rather than a control that quietly stopped appearing. The
   editor's three name-keyed tables are gone, meanings are read at every depth
   rather than only at the top level, and a list of objects is editable: a
   compound collider's pieces are added to, removed from and reordered, each
   piece drawn through the meanings that describe it. A **variant tag** — a
   field that decides what else its object holds — is now switched as one edit
   at any depth: a piece's `shape` moved from `box` to `circle` takes the half
   extents away and puts a radius there, and a camera's `projection` does the
   same thing at the top level through the same code rather than through a rule
   written for cameras. Each variant is proved at startup by building the
   component it would produce and decoding it, so a variant that cannot be
   chosen safely stops the build.
2. ~~**Decay control of animation clips.**~~ **Done.** A script names one of the
   clips the scene authored, stops it, restarts a finished one, reads the frame
   it is on, and is told when a one-shot has ended. The two halves stay where
   they belong — which clip plays is written to the world, where it has got to
   is read from the cursor beside it — so a script driving an animation still
   does not rewrite the scene it came from. Gather's player is the proof: its
   walk cycle had run since the sheet was sliced, including while standing
   still, because the clip was authored and nothing could tell it otherwise.
3. **UI widget set: slider, toggle, text input, scroll region.** Without these
   no settings screen, inventory, or long list can be built at all.
4. **Physics queries: raycast, overlap, shape cast.** Line of sight, ground
   checks, AI vision, click-to-select. Close to universal.
5. **Audio buses and a master volume.** A settings screen needs a music slider;
   today it cannot have one.
6. **Un-strand the input action layer, and implement gamepad.** The engine cost
   is paid. Make it the input system, per the anti-goal.
7. **Named collision layers instead of raw `u32` masks.** Cheap, daily friction.
8. **Tweening and easing.** The most-installed Unity asset in history. Menus,
   pickups, transitions.
9. **Collider gizmos in the Scene view.** Colliders are invisible while
   authoring, and compound pieces made that worse.
10. **Multiple scenes and additive loading.** A menu plus a level is the normal
    shape of a game.
11. **Un-strand bloom.** Built, working, unreachable.
12. **Camera follow, confine, and shake.** Every game re-implements it.
13. **Autotiling.** The daily cost of painting tilemaps by hand.
14. **Profiler view.** Needed before performance work is anything but guessing.

Items 1–6 are the ones that block a game today. Items 7–12 are cheap relative to
their daily cost. Items 13–14 are real but survivable.

---

## Beyond parity — where Sindri could lead

Everything above answers "what is an engine expected to do", and every row can
be checked against Unity or Godot. This section answers a different question:
**what could an engine provide that none of them do?** It is kept separate
deliberately. Mixed into the tables above, a reader could no longer tell "the
baseline has this and we do not" from "nobody has this and we might", and that
distinction is what makes the rest of this file worth reading.

Nothing here is scheduled. These are candidates, and they compete with each
other rather than with the ranked queue.

### The thesis

Sindri already refuses to hand a game raw `dt` and raw key states and wish it
luck. It has a fixed step, input edges consumed exactly once, and a seeded
stream that replays a run on every host. Read together those are one idea:

> The engine gives you primitives for translating imperfect human input and
> time into deterministic gameplay.

That is a stronger position than any single feature below, and it is a
description of what Sindri *is* rather than a direction bolted on. The
candidates worth taking are the ones that follow from it.

### Judged against the games, not against plausibility

A candidate earns a row by replacing something a game in this repository is
doing by hand today. Where a game is *not* asking for it, that is recorded too —
an idea that sounds good and nothing needs is the most expensive kind.

| Candidate | What it would replace | Position |
| --- | --- | --- |
| **Gameplay spatial queries** — nearest, within radius, within cone, within box, over tagged entities and backed by an index | `player.decay` has a literal `fn nearest()` looping `World.with_tag` with a `best_distance`; `arc.decay` needs "the next two nearest targets" | **Strongest.** See below — this also corrects a framing error above |
| **Entity lifecycle policies** — despawn after a duration, off-camera, or on animation end | `World.despawn(this.entity)` and a hand-decremented countdown in `bullet`, `beam`, `arc`, `core`, `charger`, `drifter`, `challenger` | **Take.** Small, and seven scripts want it |
| **Cooldowns and charges** — start, ready, remaining, normalised, recharge | `player.decay` hand-rolls `cooldown` and `mine_cooldown`; `director.decay` hand-rolls `spawn_timer` | **Take.** Small, and generic below gameplay |
| **Buffered actions** — a press remembered for a window and consumed exactly once | Nothing yet; the games are not platformers | **Take, but narrowed.** See below |
| **Named time domains** — gameplay, UI, physics and real clocks, scalable and freezable | **Nothing.** No game here scales time; only pause exists | **Row, not queue.** Best fit with the thesis, no current demand, and cross-cutting: every consumer must declare a clock |
| **Gameplay sensors** — vision cone, aggro radius, interaction range, with enter/stay/leave | Enemy scripts compute their own geometry | **Downstream.** Mostly spatial queries plus a component; thin once those exist |
| **Feedback orchestration** — one asset firing sound, effect, shake, haptics, hit-stop and flash | Real glue, but Sindri has no shake, no haptics, no hit-stop, no flash, and one effect shape | **Capstone.** It would orchestrate five systems that do not exist |
| **Deterministic state history** — a short rolling window of selected properties | **Nothing.** No game here rewinds, trails, or replays | **Low.** Unusually cheap given determinism, and nothing needs it |
| **Spawn regions and patterns** — a random point in a circle, edge, or area | A few lines of Decay per use | **Utility, not a system.** Does not earn a subsystem |

### The one that changes something above

Spatial queries expose a framing error in this file's own physics section, which
lists raycast and overlap as **physics** queries. `fn nearest()` in
`player.decay` is not a physics problem — it is a gameplay query over tagged
entities, and it is the one the games actually hand-roll. The physics casts are
a subset, not the parent.

Determinism also matters here in a way it does not for the baseline: a query
that answers in world order answers the same on every host, and a game built on
"the nearest enemy" then replays from a seed. Neither Unity nor Godot promises
that.

### Buffering, narrowed

The tempting version of this is a general *forgiveness* system — jump buffering,
coyote time, grace periods — authored in one block. Half of it does not belong
in an engine.

"Remember a press for 120ms and let gameplay consume it exactly once" knows
nothing about a game and is genuinely engine-level. "A grace period after being
grounded" requires the engine to know what *grounded* means, which is
game-specific: a top-down shooter has no such state, and an engine that assumed
one would be a system games fight rather than use.

So the half worth taking is the input half, and it already has a home: the
**input action layer** in `sindri-platform`, which is built and stranded.
Buffered press and consume-once are features of a named action, not a new
subsystem — which folds this candidate into an existing queue item instead of
adding an eleventh.

## Maintenance rule

Update this file in the same change that moves any cell, as
`AGENTS.md` requires. Three specific rules keep it from rotting into a
checklist nobody reads:

1. **A surface cell claims only what is implemented and exercised.** For a
   gameplay capability, exercised means a game uses it. `capabilities.md`
   remains the detailed evidence.
2. **A parity cell is a judgement and must survive an argument.** "Ahead" is a
   claim about the baseline, not enthusiasm.
3. **Add the row before the feature.** The value of this file is that absent
   things have rows. A gap discovered during work belongs here in the same
   change, marked ❌, even when nothing is planned.
4. **Keep the two questions apart.** Everything above "Beyond parity" is
   answerable against Unity or Godot. Everything below it is not, and moving a
   candidate up requires the baseline to have grown it, not for us to have
   liked the idea. A candidate earns its place by naming what a game in this
   repository does by hand — and when no game wants it, the row says so.
