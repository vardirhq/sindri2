# UI direction: web-grade, and better than the engines people know

Sindri's UI should be the best game UI system available: better than
Unity's and better than Godot's, and designed the way a web page is designed.
Someone who has styled a website should sit down with Weave and already know
most of it. This page is the contract for getting there: what "better" means,
the principles that decide trade-offs, and the order of the work. It moves
with the implementation, a step at a time.

## Where the others are

| | Unity uGUI | Unity UI Toolkit | Godot | Sindri target |
| --- | --- | --- | --- | --- |
| Structure | GameObjects with RectTransforms | UXML documents | Control nodes | Scene entities (the DOM), markup later |
| Styling | Per object, in the inspector | USS, a subset of CSS | Theme resources, no stylesheets | Weave: CSS as people know it |
| Layout | Anchors and layout groups | Flexbox (Yoga) | Containers | Flexbox and grid, CSS semantics |
| States | Per-component transitions | Some pseudo-classes | Theme overrides per state | `:hover`, `:active`, `:focus`, `:disabled`, `:checked` |
| Animation | Animator or tweens | USS transitions | AnimationPlayer or tweens | `transition` and `@keyframes` |
| Inspecting a live UI | Hierarchy and inspector | UI Toolkit debugger | Remote scene tree | Devtools: box model, computed styles, which rule won |
| Responsive | Canvas scaler | Limited | Stretch modes | Media queries, viewport presets, safe areas |
| Data | Scripts write values | Recent binding system | Scripts write values | Bindings in the stylesheet or scene, no script needed |

Neither engine lets a web designer work the way they already work. UI Toolkit
comes closest and stops at a subset, with tooling that makes iteration slow.
Godot has no stylesheets at all. Sindri already has the right shape: Weave is
a stylesheet language over the scene. The work is to make it complete and to
build the tools around it.

## Principles

- **CSS semantics, not CSS-flavoured.** When Weave has a feature CSS has, it
  behaves as CSS does: the cascade, specificity, inheritance, `var()`, media
  queries, the box model, flexbox. A difference is a bug unless this page
  says why.
- **The scene is the DOM.** UI structure is scene entities, so the hierarchy,
  inspector, prefabs, Decay and the editor all work on UI without learning
  a second model. A markup file comes later as another way to write the same
  tree, not a replacement.
- **Presentation is resolved, not stored.** Weave never mutates the authored
  scene; it resolves a presentation for a viewport and a set of states. What
  is saved is what was authored.
- **Tools are part of the feature.** A property nobody can see the effect of
  is not finished. The editor shows which rule set a value, and lets a value
  be changed where it is shown.
- **Every step is proven.** A UI feature example grows with this plan: a
  settings screen, an inventory and dialogs, responsive and keyboard
  navigable. Nothing is marked done that the example does not use.

## The plan

Each step is its own pull request. A step is checked here when it has landed.

1. **CSS core.**
   - [x] Selectors as in CSS: element names (`text`, `button`), IDs, classes,
     `*`, compounds (`button.primary:hover`), descendant and child
     combinators, lists, and CSS specificity.
   - [x] Inheritance of text properties; `inherit`, `initial`, `unset`.
   - [x] Custom properties and `var()` with fallbacks.
   - [x] Media queries with `and`, commas, nesting, and height conditions.
   - [x] State pseudo-classes: `:disabled` and `:checked` from the entity's
     own data everywhere, and `:hover`, `:active`, `:focus` wherever a host
     passes input state in.
   - [x] A live presentation layer in every host (editor Play, native,
     browser) that resolves each frame with pointer state and hit-tests the
     presented geometry, so `:hover` and `:active` work in the running game.
   - [x] `transition`.
   - [ ] `@keyframes`.
   - [x] The box model's spacing: margin and padding per side, as a
     `sindri.ui.box` component the layout engine itself honours.
   - [x] `box-shadow`: offset, blur, spread and colour.
   - [ ] Borders per side, negative margins, and more than one shadow.
   - [x] Flexbox: wrap, grow, shrink, basis, order, `align-self`, stretch,
     `space-around` and `space-evenly`, min/max limits, and layouts that
     size to their content with `width: auto`. Layout decides sizes as well
     as places, and what is drawn and clicked uses them.
   - [x] Intrinsic sizing from text: a label measured by its font, so text
     can size a button and be the minimum a flex item shrinks to.
   - [ ] Wrapped text's smallest width (its longest word) as a flex item's
     minimum, and text wrapping to the width a layout gave it.
   - [ ] Grid layout.
   - [ ] `calc()`, `:not()`, `:first-child` and the other structural
     pseudo-classes.
2. **Devtools in the editor.** Pick any UI element to see its box model,
   computed style, and every rule that matched with the overridden ones
   struck through; edit a value in place and write it to the stylesheet;
   preview at phone, tablet and desktop sizes with safe areas; hot reload
   into the running game.
3. **Widgets.** Toggle and checkbox, radio, dropdown, text input, scroll view,
   tabs, dialog, tooltip, progress, all styled by Weave and scriptable from
   Decay. Focus navigation by keyboard and gamepad, in an order derived from
   layout.
4. **Data binding.** A label, a fill or a list bound to game data in the
   scene or stylesheet, so Decay is for behaviour rather than for copying
   numbers into labels.
5. **Text.** Rich text inside a string (colour, weight, inline icons) and
   world-space text for damage numbers and name plates.
6. **Components.** A styled piece of UI defined once and reused, with slots,
   like a web component or a prefab.
7. **Markup.** An HTML-like file for UI structure, loading into the same
   entities, for people and tools that would rather write the tree as text.

## Where this lives

- `weave/`: the language. Selectors, the cascade, media queries, variables.
  Independent of Sindri; it decides which declarations win for an element.
- `crates/sindri-weave`: the bridge. Builds the element tree from the scene,
  runs the cascade, and turns declarations into Sindri UI data.
- `crates/sindri-scene/src/screen_ui`: layout, hit-testing and widgets.
- [`weave.md`](weave.md) and [`weave-reference.md`](weave-reference.md): the
  language as it is today.
