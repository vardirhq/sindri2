# Weave language reference

Weave is Sindri's responsive presentation language for screen UI. A stylesheet
selects existing scene entities and resolves ordinary Sindri UI component data
for a viewport. It does not create gameplay entities, replace Decay, or introduce
a browser document model.

This reference describes the currently implemented language. Unsupported CSS
features are listed explicitly so authors do not have to discover them by trial
and error.

## Project setup

Add one or more entry `.weave` files to the project's asset set:

```toml
[assets]
include = ["ui/game.weave"]
```

Imported stylesheets do not also need to be listed. The exporter follows the
entry file's `@use` graph and ships every referenced `.weave` source.

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

## Stylesheet composition

A stylesheet may import focused presentation files with top-level `@use`
directives before the first rule or media query:

```css
@use "shared/theme.weave";
@use "hud/status.weave";

.screen {
    width: 100%;
}
```

Both single and double quoted paths are accepted. Import paths are resolved
relative to the importing stylesheet asset ID, use forward slashes on every
platform, and may contain `.` or `..` so long as they do not escape the
stylesheet root.

Imported rules are inserted before the importing file's own rules. Specificity
and source order therefore behave exactly as though the imported rules had been
written first in the entry stylesheet. A source imported more than once in one
composition graph is emitted once.

Multiple independent entry stylesheets remain supported. The runtime identifies
which fetched `.weave` files are roots, composes each root, and does not apply an
imported file a second time.

Composition errors include malformed `@use` directives, missing source IDs,
paths that escape the stylesheet root, and circular imports. A cycle reports the
chain, for example `ui.weave -> shared.weave -> ui.weave`.

## Rule syntax

```css
selector {
    property: value;
}
```

A declaration ends with a semicolon. A rule may contain multiple declarations.
Selectors are written as in CSS:

| Form | Example | Matches |
| --- | --- | --- |
| Element | `text`, `button`, `shape`, `slider` | An entity with that UI component |
| Entity ID | `#hud-score` | The stable scene entity ID |
| Class | `.hud-value` | An entry in `weave.style.classes` |
| Universal | `*` | Any entity |
| State | `:hover`, `:active` (or `:pressed`), `:focus`, `:disabled`, `:checked` | An entity in that state |
| Compound | `button.primary:hover` | All of its parts at once |
| Descendant | `.menu text` | The right-hand entity anywhere inside the left |
| Child | `.menu > button` | The right-hand entity directly inside the left |
| List | `.title, .subtitle` | Either |

An element name is a UI component's short name: `text` for `sindri.ui.text`,
`image`, `shape`, `button`, `slider`, `layout`, and likewise `sprite` for
`sindri.sprite`. The full name still works, so `sindri.ui.text { … }` means
what it always did.

`:disabled` and `:checked` come from the entity's own data: a component with
`"disabled": true` makes its entity `:disabled`. `:hover`, `:active` and
`:focus` come from input, which a host passes in when it resolves
presentation (`PresentationWorld::resolve_with_states`).

## Cascade

For each property independently, as in CSS:

1. A matching rule with greater specificity wins. Specificity counts IDs, then
   classes and states, then element names: `#play` beats
   `.menu .row button.primary:hover`, which beats `button:hover`, which beats
   `button`.
2. At equal specificity, the declaration appearing later in the stylesheet wins.
3. A declaration inside an active media rule participates in the same cascade.
4. Text properties inherit: `color`, `font-size`, `font-weight`, `font-style`,
   `line-height`, `letter-spacing`, `text-align`, `text-transform` and
   `text-wrap`. Colour a panel and every label inside it takes that colour
   unless it says otherwise. Box properties such as `width` do not inherit.
5. `inherit` takes the parent's value for any property; `initial` returns a
   property to the entity's authored value; `unset` inherits a text property
   and resets any other.

```css
.hud { color: #cbd5e1; font-size: 18px; }   /* every label in the HUD */
.hud .warning { color: #f87171; }            /* except warnings */
#hud-hp-text { color: #f8fafc; }             /* and this one */
```

### Custom properties

A property whose name starts with `--` is a variable: it inherits like text
does, and `var(--name)` stands for its value wherever a value is written. A
fallback follows a comma. Put a theme's tokens on the outermost element and use
them anywhere inside it:

```css
.screen {
    --accent: #f97316;
    --panel: #111827;
    --radius: 12px;
}
.card { background: var(--panel); border-radius: var(--radius); }
.card:hover { border-color: var(--accent); }
.badge { background: var(--badge, var(--accent)); }
```

A `var()` naming nothing, with no fallback, makes its declaration invalid, and
the property keeps the entity's authored value, as in CSS. Variables that
refer to each other in a circle are treated the same way.

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
| `padding`, `padding-top`, `-right`, `-bottom`, `-left` | length, one to four as in CSS | Room inside the element's edge; layout children start inside it |
| `margin`, `margin-top`, `-right`, `-bottom`, `-left` | length, one to four as in CSS | Room a layout keeps free around the element |
| `anchor` | Sindri anchor name | Anchor for text, image, and shape |
| `direction`, `flex-direction` | `row`, `column` | Main layout axis |
| `gap` | length | Empty space between adjacent child edges, and between wrapped lines |
| `justify-content` | `start`, `center`, `end`, `space-between`, `space-around`, `space-evenly` | Main-axis distribution |
| `align-items` | `start`, `center`, `end`, `stretch` | Cross-axis alignment |
| `flex-wrap` | `nowrap`, `wrap` | Whether children that do not fit start a new line |
| `flex` | `none`, `auto`, or `<grow> [<shrink>] [<basis>]` | Shorthand for the three below, as in CSS |
| `flex-grow`, `flex-shrink` | number, 0 or more | Share of a line's spare room or shortfall |
| `flex-basis` | length, or `auto` | Size along the line before growing or shrinking |
| `order` | integer | Position in the layout, lowest first |
| `align-self` | `auto`, `start`, `center`, `end`, `stretch` | One item's cross-axis alignment |
| `width`, `height` | `auto` | On a layout: size to its children, padding and gaps |

Layout is hierarchical. A child must be parented beneath the layout entity in
the scene; visual overlap does not establish layout membership. Explicit child
boxes are important because text metrics alone do not currently provide
intrinsic layout sizing.

Layout is CSS flexbox. Children are put in `order`, broken into lines when
the layout wraps, and each line's spare room is shared by `flex-grow` or its
shortfall taken back by `flex-shrink` (weighted by size, and one by default,
as in CSS), within `min-`/`max-` limits; then the line is justified and each
child aligned across it. Wrapped lines share the layout's leftover height, as
CSS's default `align-content` does. What the layout decides is what is drawn
and what is clicked. `flex-start` and `flex-end` are accepted for `start` and
`end`. Two differences from a browser: limits are applied in one pass rather
than by re-sharing what a clamped child could not take, and a child that is
not itself a layout has no automatic minimum, because text is not measured
yet. A layout's automatic minimum is its children and padding, so a badge
does not shrink into its own label.

`padding` and `margin` take one to four values in CSS order (all; vertical
and horizontal; top, horizontal and bottom; top, right, bottom and left), and
the language expands them into their per-side longhands where they are
written, so a later `padding-top` overrides one side and keeps the rest. A
percentage on any side is of the containing element's content width, as in
CSS. Both are written to the element's `sindri.ui.box`, which the engine's
layout reads, so an authored scene gets the same box model without Weave.
As in flexbox, margins do not collapse: neighbours' margins and the `gap` add
up. Negative margins are accepted and currently count as none.

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
| `box-shadow` | `<x> <y> [<blur> [<spread>]] [<color>]`, or `none` |

These declarations target `sindri.ui.shape`.

`box-shadow` draws the shape's silhouette behind it, grown by the spread,
moved by the offset (positive `y` is down, as in CSS) and blurred over the
blur radius, with corners rounded to match. With no colour it is black at
half opacity. One shadow per element: a comma-separated list and `inset` are
refused rather than half drawn. A button can carry both
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
- `max-width: <length>`, `min-width: <length>`
- `max-height: <length>`, `min-height: <length>`

Combine them as in CSS: `and` requires every condition, a comma accepts any
of the alternatives, and a `@media` block nested in another applies only
where both do.

```css
@media (min-width: 900px) and (orientation: landscape) { … }
@media (max-height: 500px), (max-width: 400px) { … }
```

Put shared declarations in a normal rule and override only what changes inside
the media rule.

## Runtime behavior

The host loads the complete `.weave` source graph, composes each independent
entry stylesheet, computes cascade winners, then resolves a presentation world
for the viewport. Parents settle before descendants so percentages, padding,
and constraints have stable inputs. Rendering, pointer hit-testing, editor
placement, and generic UI layout consume the resulting ordinary Sindri
components.

A viewport change resolves a fresh presentation world from the authored scene.
Weave does not progressively mutate the previous resolved result.

Decay should continue referring to stable entity IDs:

```text
Ui.set_number("hud-score", score)
Ui.set_fill("hud-hp", health_ratio)
```

Presentation changes do not require those bindings to change.

## Transitions

`transition` eases a property from the value it is showing to a new one when
the rule that sets it changes, as in CSS:

```css
button { background: #334155; transition: background 150ms ease-out; }
button:hover { background: #475569; }
button:active { background: #1e293b; transition: background 50ms linear; }
```

The shorthand takes the property (or `all`), a duration, an optional easing
and an optional delay, and a comma-separated list for several properties.
Durations are `s` or `ms`. Easings are `linear`, `ease`, `ease-in`,
`ease-out`, `ease-in-out` and `cubic-bezier(x1, y1, x2, y2)`. Colours, lengths
and plain numbers ease; other values change at once. As in CSS, the
transition that applies is the one on the rule being changed *to*, and an
interrupted transition starts again from the value it had reached.

## Pointer states in the running game

Every host presents the scene through Weave each frame with the pointer's
state: the editor's Play, native games and browser exports. `:hover` matches
the element under the pointer and every container it is inside, and `:active`
matches the pressed element while the pointer stays on it (and a slider for
as long as it is dragged), as in CSS. Hit-testing uses the presented geometry,
so a button a media query moved or resized is clicked where it is drawn.
`:disabled` and `:checked` come from the entity's own data. The authored scene
is never changed by any of this.

## Authoring guidance

The editor recognizes manifest-listed `.weave` roots, previews their source,
authors `weave.style.classes`, and applies the composed result to Scene and Game
views. It hot-reloads saved changes throughout each root's `@use` graph. A
broken reload keeps the last good presentation and reports the source path,
line, and column. Editing stylesheet source and choosing named viewport presets
remain external workflows.

- Keep semantic identity in entity IDs and reusable appearance in classes.
- Keep one small entry stylesheet and split large surfaces with `@use`.
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

- sizing from measured text (`width: auto` on a text element, or text as
  the minimum a flex item shrinks to)
- borders per side, negative margins, and more than one or an `inset` shadow
- `align-content` other than its default, `row-gap`/`column-gap`, and
  reversed directions
- grid, scrolling, and clipping regions
- calculations (`calc()`)
- attribute selectors, sibling combinators, and structural pseudo-classes
  such as `:first-child` or `:not()`
- `:focus` in the running game: nothing takes focus until keyboard and
  gamepad navigation land
- `@keyframes` animation
- accessibility mapping
- integrated stylesheet source editing and named viewport preset controls

Do not silently imitate these features with unexplained fixed coordinates.
Document the required workaround or add the missing primitive at the generic
Sindri UI layer when a real game proves it is needed.

## Diagnostic checklist

When an element is misplaced:

1. Confirm the selector matches the intended entity ID, class, or element,
   and that every compound in a descendant selector has an ancestor to match.
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
