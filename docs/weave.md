# Weave presentation styling

Weave is an experimental presentation language for Sindri screen UI. It lets a
project keep responsive layout and visual styling beside its scene instead of
baking every viewport choice into component JSON.

The language remains independent in the `weave/` workspace. The
`sindri-weave` bridge is the only crate that knows both Weave and Sindri. It
resolves a disposable presentation copy of an authored `World`; the source
world is never mutated.

The browser proof lives at `games/weave-poc` and is deployed to
`/sindri2/examples/weave-poc/`.

## Documentation map

- This page is the short conceptual guide.
- [`weave-reference.md`](weave-reference.md) is the complete implemented
  language reference, including diagnostics and unsupported CSS.
- [`weave-poc.md`](weave-poc.md) records the architectural boundary and proof
  strategy.
- [`weave-migration.md`](weave-migration.md) is the step-by-step real-game
  migration guide and acceptance checklist.
- `games/weave-poc/assets/demo.weave` is the smallest responsive example.
- `games/orbital-last-stand/assets/ui.weave` is the production-oriented entry
  stylesheet; it composes focused HUD, overlay, and screen styles with `@use`.

## Quick start

1. Keep UI entities, hierarchy, components, initial content, and stable IDs in
   the scene.
2. Add presentation classes with the opaque `weave.style` component.
3. Add one entry `.weave` file to the project's `[assets].include` list. Imported
   stylesheets are discovered by the exporter.
4. Split larger presentation surfaces with top-level `@use "...";` directives.
5. Style broad roles with classes and reserve ID rules for true exceptions.
6. Add portrait or width media rules that recompose the hierarchy.
7. Verify rendering and pointer hit testing at wide, narrow, and portrait sizes.

```toml
[assets]
include = ["ui/game.weave"]
```

```css
@use "shared/theme.weave";
@use "screens/menu.weave";

.panel {
    width: 560px;
    padding: 24px;
    direction: column;
    gap: 16px;
}

.action {
    width: 240px;
    height: 52px;
    border-radius: 12px;
}

@media (orientation: portrait) {
    .panel { width: 92vw; }
    .action { width: 100%; }
}
```

Weave owns presentation only. Decay should continue updating the same stable
entity IDs with `Ui.set_text`, `Ui.set_number`, `Ui.set_numbers`, and
`Ui.set_fill`.

## Stylesheet composition

`@use` is a deliberately small file-composition primitive, not a second module
system. Directives must be top-level and appear before the first selector or
media query:

```css
@use "shared/theme.weave";
@use "hud/status.weave";
```

Paths are resolved relative to the importing stylesheet's asset ID and always
use forward slashes. `.` and `..` are normalized, but an import may not escape
the stylesheet root. Imported rules are inserted before the importing file's
own rules, so ordinary Weave specificity and source order continue to determine
the winner.

The exporter follows the complete `@use` graph from an entry stylesheet in
`[assets].include`, so imported files do not also need to be listed in
`sindri.toml`. The browser host composes the fetched source graph and does not
apply imported files a second time. Missing sources, malformed directives,
root escapes, and circular imports are errors with the stylesheet path/import
chain in the diagnostic.

Independent `.weave` files may still be listed as separate roots. They are
composed independently and applied in deterministic asset order.

## Selectors

A rule targets one entity at a time:

```css
#menu { width: 620px; }
.menu-button { height: 64px; }
sindri.ui.text { text-align: center; }
```

| Selector | Scene input | Specificity |
| --- | --- | --- |
| `#id` | The entity's stable authored `id` | 2 |
| `.class` | A name in `weave.style.classes` | 1 |
| component type | A component key on the entity | 0 |

Classes are presentation metadata in an opaque scene component:

```json
"weave.style": {
  "classes": ["menu-button", "primary"]
}
```

When matching rules write the same property, higher specificity wins. Rules
with equal specificity use the later source order. There are no compound
selectors or inheritance in this proof of concept.

## Properties

The bridge translates declarations into existing Sindri data. It does not add a
second renderer or a parallel UI object model.

| Weave property | Sindri target |
| --- | --- |
| `width`, `height` | entity transform scale |
| `min-width`, `max-width`, `min-height`, `max-height` | constraints applied to final transform scale |
| `padding` | content-box inset used when percentage-sized children resolve |
| `x`, `y` | entity transform position |
| `anchor` | UI image, shape, and text anchor |
| `direction`, `gap` | `sindri.ui.layout` |
| `justify-content` | main-axis distribution on `sindri.ui.layout` |
| `align-items` | cross-axis alignment on `sindri.ui.layout` |
| `background` | `sindri.ui.shape.fill` |
| `border-color`, `border-width`, `border-radius` | shape stroke and corner fields |
| `color` | `sindri.ui.text.color` |
| `font-size`, `line-height`, `letter-spacing` | text metrics |
| `font-weight` | text bold state |
| `text-align` | text line alignment |
| `text-wrap` | text wrapping (`none`, `word`/`wrap`, `glyph`) |
| `text-transform` | text case |

`justify-content` accepts `start`, `center`, `end`, and `space-between`.
`align-items` accepts `start`, `center`, and `end`. The engine's generic UI
layout owns the actual placement, so rendering, hit testing, and editor handles
all consume the same resolved positions rather than learning Weave-specific
rules.

Colors accept `transparent`, `black`, `white`, and three-, four-, six-, or
eight-digit sRGB hex notation; the bridge converts color channels to the renderer's
linear space while leaving alpha unchanged. Font weight accepts `normal`, `400`, `bold`, or
`700`. Text transform accepts `none`, `uppercase`, or `lowercase`.

Unknown properties are currently ignored. Invalid values for known properties
return an `ApplyError` naming the entity, property, and value.

## Lengths and responsive rules

Lengths may be unitless overlay values, pixels, `vw`, `vh`, or percentages.
Percentage width, height, and min/max constraints resolve against the parent's
final content box rather than the viewport. Uniform `padding` shrinks that
content box for descendants, so `width: 100%` fills the padded interior instead
of the parent's outer box. Percentage border width and radius remain relative to
the final styled box.

Sindri's overlay is two units tall, so the bridge resolves viewport-dependent
units before ordinary scene extraction. Parent boxes settle before descendants,
which keeps percentage sizing deterministic even when a child appears before its
parent in authored scene order.

Supported media conditions are:

```css
@media (orientation: portrait) { /* ... */ }
@media (orientation: landscape) { /* ... */ }
@media (max-width: 700px) { /* ... */ }
@media (min-width: 701px) { /* ... */ }
```

Each condition wraps ordinary rules. Conditions cannot yet be combined.

## Runtime flow

1. Load the authored scene into a normal `World`.
2. Load the project's `.weave` source graph and compose each independent entry
   stylesheet.
3. Resolve selector matches and cascade winners into a computed style for each
   entity.
4. Resolve a `PresentationWorld` for the current viewport, settling ancestors
   before descendants so constraints, padding, and percentages have a stable
   basis.
5. Feed its ordinary transforms and component payloads into the existing
   extraction, rendering, layout, and input paths.
6. Resolve again from the authored world when the viewport changes.

This keeps Weave reversible: removing the bridge leaves the engine and scene
model intact.

## Editor workflow

The editor reads `.weave` roots from `[assets].include`, follows each root's
`@use` graph, and applies the same ordered composition as export to both Scene
and Game views. An entity's stable ID is its `#id` selector; reusable `.class`
selectors are authored in the inspector's **Weave Style** section. Selecting a
stylesheet in the project browser opens a read-only source preview.

Saved changes to the manifest or any stylesheet in the source graph are picked
up automatically. A failed reload reports the source location and preserves
the last good presentation until another file change succeeds. Source editing
and named viewport presets are not integrated into the editor yet.

## Current limits

This is still intentionally smaller than browser CSS. It has no compound or
descendant selectors, pseudo-states, variables, per-side padding, margin,
flexible growth/shrink, accessibility mapping, integrated stylesheet editor, or
named viewport presets.
The demo proves reusable classes, cascade behavior, responsive geometry,
min/max constraints, uniform content padding, main/cross-axis alignment, font
metrics, fills, strokes, rounded shapes, and composable stylesheet sources on
the native Sindri UI path.
