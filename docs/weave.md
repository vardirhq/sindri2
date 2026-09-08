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
- `games/orbital-last-stand/assets/ui.weave` is the production-oriented
  migration example as Last Stand screens move onto Weave.

## Quick start

1. Keep UI entities, hierarchy, components, initial content, and stable IDs in
   the scene.
2. Add presentation classes with the opaque `weave.style` component.
3. Add a `.weave` file to the project's `[assets].include` list.
4. Style broad roles with classes and reserve ID rules for true exceptions.
5. Add portrait or width media rules that recompose the hierarchy.
6. Verify rendering and pointer hit testing at wide, narrow, and portrait sizes.

```toml
[assets]
include = ["ui/game.weave"]
```

```css
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
2. Parse the project's `.weave` stylesheet.
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

## Current limits

This is still intentionally smaller than browser CSS. It has no compound or
descendant selectors, pseudo-states, variables, per-side padding, margin,
flexible growth/shrink, accessibility mapping, editor inspector, or hot reload.
The demo proves reusable classes, cascade behavior, responsive geometry,
min/max constraints, uniform content padding, main/cross-axis alignment, font
metrics, fills, strokes, and rounded shapes on the native Sindri UI path.
