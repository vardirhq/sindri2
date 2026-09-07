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
| `x`, `y` | entity transform position |
| `anchor` | UI image, shape, and text anchor |
| `direction`, `gap` | `sindri.ui.layout` |
| `background` | `sindri.ui.shape.fill` |
| `border-color`, `border-width`, `border-radius` | shape stroke and corner fields |
| `color` | `sindri.ui.text.color` |
| `font-size`, `line-height`, `letter-spacing` | text metrics |
| `font-weight` | text bold state |
| `text-align` | text line alignment |
| `text-transform` | text case |

Colors accept `transparent`, `black`, `white`, and three-, four-, six-, or
eight-digit sRGB hex notation; the bridge converts color channels to the renderer's
linear space while leaving alpha unchanged. Font weight accepts `normal`, `400`, `bold`, or
`700`. Text transform accepts `none`, `uppercase`, or `lowercase`.

Unknown properties are currently ignored. Invalid values for known properties
return an `ApplyError` naming the entity, property, and value.

## Lengths and responsive rules

Lengths may be unitless overlay values, pixels, `vw`, or `vh`. Percentages
are supported for border width and radius and are relative to the final box.
Sindri's overlay is two units tall, so the bridge resolves viewport-dependent
units before ordinary scene extraction.

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
3. Resolve a `PresentationWorld` for the current viewport.
4. Feed its ordinary transforms and component payloads into the existing
   extraction, rendering, layout, and input paths.
5. Resolve again from the authored world when the viewport changes.

This keeps Weave reversible: removing the bridge leaves the engine and scene
model intact.

## Current limits

This is a deliberately narrow proof, not a complete CSS implementation. It has
no compound or descendant selectors, pseudo-states, variables, padding,
min/max constraints, accessibility mapping, editor inspector, or hot reload.
The demo proves reusable classes, cascade behavior, responsive geometry, font
metrics, fills, strokes, and rounded shapes on the native Sindri UI path.
