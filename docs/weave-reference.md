# Weave language reference

Weave is Sindri's responsive presentation language for screen UI. A stylesheet
selects existing scene entities and resolves ordinary Sindri UI component data
for a viewport. It does not create gameplay entities, replace Decay, or introduce
a browser document model.

This reference describes the currently implemented language. Unsupported CSS
features are listed explicitly so authors do not have to discover them by trial
and error.

## Project setup

Add one or more `.weave` files to the project's asset set:

```toml
[assets]
include = ["ui/game.weave"]
```

The scene remains the source of entity identity, hierarchy, components, initial
content, and gameplay bindings. Use `weave.style.classes` for reusable
presentation roles:

```json
"weave.style": {
  "classes": ["hud-card", "danger"]
}
```

Removing the stylesheet restores the authored presentation because Weave
resolves a disposable copy of the world.

## Rule syntax

```css
selector {
    property: value;
}
```

A declaration ends with a semicolon. A rule may contain multiple declarations.
The supported selector forms are:

| Form | Example | Matches | Specificity |
| --- | --- | --- | ---: |
| Entity ID | `#hud-score` | Stable scene entity ID | 2 |
| Weave class | `.hud-value` | Entry in `weave.style.classes` | 1 |
| Component | `sindri.ui.text` | Entity containing that component | 0 |

Selectors currently target one entity at a time. Compound selectors,
descendant selectors, selector lists, combinators, attributes, and pseudo-classes
are not implemented.

## Cascade

For each property independently:

1. A matching rule with greater specificity wins.
2. At equal specificity, the declaration appearing later in the stylesheet wins.
3. A declaration inside an active media rule participates in the same cascade.
4. There is no inheritance.

A class can therefore establish a reusable baseline while an ID makes a local
adjustment:

```css
.hud-value { color: #cbd5e1; }
#hud-hp-text { color: #f8fafc; }
```

## Lengths

| Unit | Meaning |
| --- | --- |
| no unit | Sindri overlay units |
| `px` | CSS-style logical pixels converted for the viewport |
| `vw` | Percentage of viewport width |
| `vh` | Percentage of viewport height |
| `%` | Percentage of the parent's resolved content box |

Sindri's overlay is two units tall. Prefer pixels for stable desktop controls,
viewport units for deliberate screen-relative composition, and percentages for
children that should track a parent.

Percentage descendants resolve after the parent. Uniform `padding` reduces the
parent content box, so `width: 100%` fills the padded interior rather than
covering the outer border.

`min-*` and `max-*` constraints apply to the final resolved size.

## Layout properties

| Property | Accepted values | Effect |
| --- | --- | --- |
| `width`, `height` | length | Resolved UI box size |
| `min-width`, `max-width` | length | Horizontal size constraint |
| `min-height`, `max-height` | length | Vertical size constraint |
| `x`, `y` | length | Authored transform position override |
| `padding` | length | Uniform content inset |
| `anchor` | Sindri anchor name | Anchor for text, image, and shape |
| `direction` | `row`, `column` | Main layout axis |
| `gap` | length | Empty space between adjacent child edges |
| `justify-content` | `start`, `center`, `end`, `space-between` | Main-axis distribution |
| `align-items` | `start`, `center`, `end` | Cross-axis alignment |

Layout is hierarchical. A child must be parented beneath the layout entity in
the scene; visual overlap does not establish layout membership. Explicit child
boxes are important because text metrics alone do not currently provide
intrinsic layout sizing.

`gap` measures edge-to-edge space, not distance between child centres.
`space-between` places the first and last child at the content edges and
distributes remaining space between the interior gaps.

## Shape properties

| Property | Accepted values |
| --- | --- |
| `background` | color |
| `border-color` | color |
| `border-width` | length |
| `border-radius` | length |

These declarations target `sindri.ui.shape`. A button can carry both
`sindri.ui.button` and `sindri.ui.shape`, allowing its hit target and visual
box to remain the same entity.

Colors accept `transparent`, `black`, `white`, and 3-, 4-, 6-, or 8-digit
hex. Hex colors are authored in sRGB and converted to the renderer's linear
space; alpha is unchanged.

## Text properties

| Property | Accepted values | Notes |
| --- | --- | --- |
| `color` | color | Text color |
| `font-size` | length | Glyph size |
| `line-height` | length | Baseline-to-baseline distance |
| `letter-spacing` | length | Additional glyph spacing |
| `font-weight` | `normal`, `400`, `bold`, `700` | Normal or bold |
| `text-align` | `start`, `center`, `end` | Alignment inside the text box |
| `text-wrap` | `none`, `nowrap`, `word`, `wrap`, `glyph` | Line breaking mode |
| `text-transform` | `none`, `uppercase`, `lowercase` | Presentation-only case |

Wrapping requires a finite text width and enough height for the expected lines.
Use `word` for ordinary copy and `glyph` only when a long unbroken token must
break.

## Responsive rules

```css
@media (orientation: portrait) {
    .actions {
        direction: column;
        width: 100%;
    }
}

@media (max-width: 700px) {
    .title { font-size: 28px; }
}
```

Supported conditions are:

- `orientation: portrait`
- `orientation: landscape`
- `max-width: <length>`
- `min-width: <length>`

Conditions cannot currently be combined. Put shared declarations in a normal
rule and override only what changes inside the media rule.

## Runtime behavior

The bridge parses the stylesheet, computes cascade winners, then resolves a
presentation world for the viewport. Parents settle before descendants so
percentages, padding, and constraints have stable inputs. Rendering, pointer
hit-testing, editor placement, and generic UI layout consume the resulting
ordinary Sindri components.

A viewport change resolves a fresh presentation world from the authored scene.
Weave does not progressively mutate the previous resolved result.

Decay should continue referring to stable entity IDs:

```text
Ui.set_number("hud-score", score)
Ui.set_fill("hud-hp", health_ratio)
```

Presentation changes do not require those bindings to change.

## Authoring guidance

- Keep semantic identity in entity IDs and reusable appearance in classes.
- Use scene parenting to describe layout ownership.
- Give layout children explicit boxes.
- Prefer content-driven authored dimensions over filling the viewport merely
  because `vh` is available.
- Test at least one wide, one narrow landscape, and one portrait viewport.
- Preserve touch targets when shrinking visuals; a readable mobile layout is
  more important than fitting a desktop composition onto a phone.
- Treat the authored component values as a safe fallback.

## Errors and unsupported declarations

Invalid values for known properties return an `ApplyError` containing the
entity, property, and value. Unknown properties are currently ignored.

The following CSS concepts are not implemented:

- intrinsic `auto` sizing
- margins or per-side padding
- flex grow and shrink, including cross-axis stretch
- grid, wrapping layout, scrolling, and clipping regions
- variables, calculations, or custom properties
- selector composition and inheritance
- hover, pressed, focus, disabled, or other pseudo-states
- transitions and animation
- accessibility mapping
- editor stylesheet tooling and hot reload

Do not silently imitate these features with unexplained fixed coordinates.
Document the required workaround or add the missing primitive at the generic
Sindri UI layer when a real game proves it is needed.

## Diagnostic checklist

When an element is misplaced:

1. Confirm the selector matches the intended entity ID, class, or component.
2. Confirm the entity is parented under the expected layout entity.
3. Check whether a more specific or later rule overrides the property.
4. Give the child an explicit `width` and `height`.
5. Check the parent's resolved size, padding, direction, gap, and alignment.
6. Verify that the active media condition matches the actual layout viewport.

When text escapes its box:

1. Set a finite width.
2. Enable `text-wrap: word`.
3. Provide height for every expected line.
4. Use a deliberate `line-height`.
5. Test the longest runtime string, not only the authored placeholder.

When a button looks stretched or has malformed corners:

1. Keep the shape and button hit target on the same entity.
2. Use an absolute border radius for a rounded rectangle.
3. Avoid a radius near half the height unless a pill is intentional.
4. Verify that responsive rules change both dimensions consistently.
