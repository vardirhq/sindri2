# Migrating a Sindri UI to Weave

This guide uses Orbital Last Stand as the reference migration. It describes how
to move layout and visual styling into Weave without breaking Decay scripts,
button hit targets, dynamic text, fill bars, disabled screen roots, or the
authored scene fallback.

Read [`weave-reference.md`](weave-reference.md) for the exact language surface.

## What moves and what stays

| Concern | Keep in the scene or script | Move to Weave |
| --- | --- | --- |
| Entity identity | Stable scene ID | Never |
| Parent/child ownership | Scene hierarchy | Never |
| Gameplay behavior | Decay script component | Never |
| Button action | Script and `sindri.ui.button` | Never |
| Runtime text and values | `Ui.set_*` calls | Never |
| Runtime meter fill | `Ui.set_fill` | Never |
| Presentation role | `weave.style.classes` metadata | Class name only |
| Size and position | Authored fallback | Responsive override |
| Typography and color | Authored fallback | Responsive override |
| Shape fill, border, radius | Authored fallback | Responsive override |
| Layout direction and gaps | Authored fallback | Responsive override |

Weave should be removable. If deleting the stylesheet also deletes behavior, the
migration crossed the presentation boundary.

## 1. Inventory the real UI

Group entities by screen and behavior before writing styles. Last Stand contains
54 UI entities:

- 32 text entities
- 14 shapes
- 10 buttons
- 7 images, including dynamic fill bars
- 1 authored layout container

Its screens are the gameplay HUD, boss meter, pause overlay, result overlay,
title screen, and upgrade selector. The upgrade selector is the most demanding
responsive surface because its choices change from a horizontal card row to a
vertical phone list.

Record every script-facing ID. Last Stand keeps IDs such as `hud-score`,
`hud-hp`, `pause-resume`, and `card-dmg` unchanged.

## 2. Add semantic presentation roles

Use classes for shared appearance and IDs for unique placement:

```json
"weave.style": {
  "classes": ["overlay-button", "overlay-primary"]
}
```

Useful class categories include:

- structural roles: `upgrade-grid`, `overlay-panel`
- component roles: `hud-stat`, `upgrade-card`
- variants: `overlay-primary`, `overlay-danger`
- element-specific roles only when several rules describe that element

Avoid encoding coordinates or device names into class names.

## 3. Establish the wide layout

Start with the viewport where the existing UI is easiest to inspect. Improve
hierarchy while preserving behavior:

```css
.upgrade-grid {
    width: 980px;
    height: 360px;
    direction: row;
    gap: 20px;
    justify-content: center;
    align-items: center;
}

.upgrade-card {
    width: 300px;
    height: 290px;
    border-radius: 20px;
}
```

Size every layout child explicitly. Weave does not yet have intrinsic `auto`
sizing or flex growth.

For buttons whose text is a separate child entity, move the shape and label
together. The shape remains the hit target.

## 4. Recompose rather than shrink

A phone layout should represent the same information hierarchy, not a miniature
desktop screen:

```css
@media (max-width: 700px) {
    .upgrade-grid {
        width: 80vw;
        height: 126vw;
        direction: column;
        gap: 3vw;
        align-items: stretch;
    }

    .upgrade-card {
        width: 100%;
        height: 34vw;
    }
}
```

Last Stand uses these responsive changes:

| Surface | Wide presentation | Narrow presentation |
| --- | --- | --- |
| HUD | Fixed logical-pixel groups near screen edges | Screen-relative meter and type sizes |
| Pause/results | Compact centered panels | Wider touch controls and `vw` rhythm |
| Title | Large logo panel | Content column sized to phone width |
| Upgrades | Horizontal card choices | Vertical full-width choices |
| Long copy | Bounded text | Bounded word wrapping |

Do not use a tall `vh` panel to manufacture empty space. Author a box that fits
the intended content until intrinsic sizing exists.

## 5. Preserve dynamic behavior

Presentation resolution must retain the component payloads scripts update:

```text
Ui.set_number("hud-score", score)
Ui.set_numbers("hud-hp-text", health, max_health)
Ui.set_fill("hud-hp", health_ratio)
```

The Last Stand regression test resolves the real scene and stylesheet, then
checks that score placeholders and meter fill data survive.

Disabled screen roots remain responsible for visibility. Weave does not replace
screen state management.

## 6. Validate geometry and behavior

Test at minimum:

| Viewport | Purpose |
| --- | --- |
| 1280 × 720 | Baseline landscape composition |
| 844 × 390 | Short landscape pressure |
| 390 × 844 | Common portrait phone |
| 360 × 800 | Narrow phone and long-string pressure |
| 1920 × 1080 | Large-screen bounds |

For each viewport verify:

- text is legible and does not escape its box
- buttons and labels share the same visual centre
- touch targets remain large enough
- active media rules produce the intended direction and sizes
- dynamic labels fit their longest realistic values
- fill bars retain their fill direction and runtime amount
- disabled overlays remain hidden until their script enables them
- pointer hit testing follows the resolved button shape
- scene data remains unchanged after presentation resolution

Automated tests should assert meaningful outcomes, such as `row` becoming
`column`, rather than snapshotting every float.

## 7. Keep the fallback until acceptance

Do not erase authored component values during migration. Merge only after the
resolved presentation has been exercised in the native player and browser build.
The fallback makes the migration reversible and helps distinguish a Weave
problem from a scene or gameplay problem.

## Known migration blockers

Stop and add a generic Sindri UI primitive when a design genuinely requires:

- intrinsic content sizing
- wrapped flex rows or a grid
- scrolling or clipping
- pseudo-state presentation
- per-side spacing
- accessibility metadata
- animation or transitions

Do not hide an engine gap behind duplicated mobile scene trees or Decay viewport
calculations. The point of a real-game migration is to expose missing generic
capabilities.

## Last Stand file map

- Scene and presentation classes:
  `games/orbital-last-stand/assets/orbital.scene.json`
- Stylesheet:
  `games/orbital-last-stand/assets/ui.weave`
- Asset registration:
  `games/orbital-last-stand/sindri.toml`
- Integration tests:
  `crates/sindri-weave/tests/last_stand.rs`

These files form the canonical real-game reference. The smaller
`games/weave-poc` example remains useful for learning individual properties.
