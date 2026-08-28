# Changelog

All notable changes to Sindri Next will be documented here.

## [Unreleased]

- Add editor wall/footprint authoring, typed Decay pathfinding, and a Gather Wisp that proves authored A* navigation end to end.

### Added

- **The editor opens a project, and asks which one.** A project is a directory
  holding `sindri.toml` — a format version, a name, and the scene opening it
  opens — and the welcome window is its own window, with the editor's hidden
  behind it until a project is chosen. It lists the projects you have opened,
  marks one that has moved or been deleted as missing rather than dropping it
  silently, and offers the two ways to get another: **New project**, which
  writes a manifest, a scene with a camera in it, and the folders assets resolve
  from; and **Open project**, which takes a folder that already is one. Gather
  ships with a manifest and is offered as a sample when the editor is run from a
  checkout. `docs/project-format.md` is the contract.

  A scene carries its project with it: one opened from the command line, a file
  dialog, or a browser row walks up to the nearest `sindri.toml`, so the project
  browser roots at the whole project and is headed with the project's own name —
  Gather rather than `assets`. A scene in no project still opens, which is what
  the editor did before projects existed.

  A launch honours the command line first, then the last project when the
  welcome window's footer was ticked to skip it, then the window. Reopening a
  project reopens the scene you were last in inside it, rather than its front
  door. `sindri-editor` with no argument now opens the welcome window rather
  than the demo scene compiled into the repository, which is also why
  `scripts/capture-editor.sh` names that scene explicitly.

- Decay has loops: `while`, with `break` and `continue`, bounded by a new
  operation budget so that a script which does not stop cannot take the editor
  with it. One call may run 1,000,000 instructions by default before
  `OperationBudgetExceeded`, and the host may change that.
- Decay has `%` and `%=`, a remainder whose sign follows its left operand.
- Decay accepts a chained `else if`, which previously had to be written
  `else { if ... }`.
- A Decay field initializer that reads a field declared below it, or reads
  itself, is now a compile error naming both fields. It used to compile and fail
  at runtime with a path name.

### Fixed

- **A UI element could not be given the script that drives it.** `space::accepts`
  read the two families as symmetric, so a UI entity accepted `sindri.ui.*` and
  nothing else: selecting Gather's banner — a UI image driven by a script — and
  pressing Add Component offered one entry, UI Text. The banner could be opened
  and not rebuilt. Only the four components that *place* something are exclusive
  to a space, which `declared_space` already knew and `accepts` was not asking;
  the test that existed only checked a UI entity refuses a sprite, so a rule
  stated in two directions was tested in one.

### Added

- **Add Component is grouped.** Thirteen entries is a list you read rather than
  a menu you use, and it only grows. They sit under Rendering, UI, Physics, Grid
  and Behaviour now, from an authored table rather than from splitting the type
  name on its dots — the namespace is a naming scheme, not a taxonomy, and
  splitting it gives two one-entry submenus and five components with no family
  at all. A family holding a single offer is listed at the top level instead,
  because hiding a lone entry behind a heading is a click that buys nothing.
  That table also says which glyph each component draws with, so the two facts
  cannot drift: they already had, and audio sources, rigid bodies and colliders
  were all drawing with the generic entity box. A test asserts every registered
  component has a row, so the next one added to the engine fails the build
  rather than quietly arriving unfamilied — and audio stopped sharing the play
  glyph with sprite animation while the two were side by side in it.
- **An entity can be switched off without being deleted.** It was the last of
  the audit's smaller findings and it was an engine gap, not an editor one:
  nothing in the scene format said an entity could be inactive, so the editor
  had nothing to offer. `EntityData` and `SceneEntity` carry a `disabled` flag
  now, and `World::is_active` is the question anything drawing, stepping,
  scripting or picking asks — it walks ancestors, so switching off a HUD
  switches off its pips. The filter is applied once, in
  `ComponentSchemaRegistry::query`, rather than at each of the six places that
  would otherwise have to remember it. The flag is per entity and is never
  written down through a subtree, so re-enabling a parent brings back exactly
  the children that were on. Omitted from a saved scene when false, so there is
  no format change and every existing file is byte for byte what it was. In the
  editor: an Active switch on the inspector, greyed out and saying so on a child
  that is off because its parent is; Disable and Enable on a hierarchy row's
  menu, taking the whole selection; and a struck-through row for anything
  switched off, because dim already means "nothing here to act on" and this is
  the row you would switch back on.
- **A History dock, showing what Ctrl+Z will do and everything past it.** The
  history was answerable one step at a time, from a label on a menu entry nobody
  opens mid-edit, so "how far back can I go" had no answer and an edit made
  twenty steps ago that turned out to be wrong was undone by pressing a key
  twenty times and watching the viewport to see where you were. The dock is the
  stack drawn: "Scene opened", then every step in the order it happened, the one
  the world is at marked, and the steps already undone still listed under it
  dimmed, because they are still reachable. Clicking one travels there — by
  calling the same undo and redo the keys call, once per step, rather than by a
  jump of its own. `CommandHistory` gained `undo_steps` and `redo_steps`, which
  hand out labels and keep the transactions private: a caller holding one could
  apply it out of order.
- **A string can be clicked in the Scene view.** It was the last drawn thing
  that could not be, and the reason was real: a string is the one drawn thing
  with no size in the scene. What it covers is glyph layout — kerning, fallback,
  the wrap the viewport imposes — decided inside the text renderer, and a box
  guessed from the font size and the character count picks the wrong entity
  along its edges, which is worse than not picking at all. So the box is not
  guessed. `TextRenderer::measure` answers it from the same shaping the frame is
  drawn with, now one function shared by drawing and measuring, and
  `OverlayPlacement::text_origin` answers where the string starts from the same
  place the frame's text pass positions it. The editor measures at the
  resolution the view renders at and hands the boxes to picking, which stays
  free of the GPU and settles a string against an image by the layer rule two
  images already settle it by.
- **Siblings can be reordered by moving them rather than by renaming them.**
  Order was the stable ID sorted, so authoring order was alphabetical by a
  string most authors never look at: five pips made from one arrived as
  `pip-1`, `pip-1-copy`, `pip-1-copy-2`, and putting them in the order the HUD
  reads them meant renaming their IDs. Move up and Move down now sit on a row's
  right-click menu and on Alt+Up and Alt+Down, greyed out at the ends of a list
  rather than offered and refused. Where the order is recorded is the whole of
  the problem: a scene's document order is canonical and deliberately
  meaningless, so that a save stays stable while entities are added and
  reparented, and draw order is expressed by render layers and depths. So
  sibling order goes in the entity's editor-only section of the file, which a
  runtime carries but never interprets — no format change, and a scene nobody
  has reordered still lists exactly as it did. That needed one new command,
  `WorldCommand::SetEditorEntry`: the editor map had no write path, so nothing
  in it could be undone or mark a document unsaved.
- **More than one entity can be selected at a time.** Every bulk verb was
  impossible to express while a selection was one entity: deleting five pips
  meant five deletes and five undo steps, and moving a row of them meant
  dragging each one to the same place by eye. Ctrl-click adds and removes,
  Shift-click takes the range between two rows as the hierarchy is drawing them,
  and Ctrl-click does the same in the Scene view. Delete, Duplicate and a drag
  to a new parent then take the whole selection in one undo step, and so does a
  gizmo drag: each selected entity is moved, turned or scaled by what the one
  under the pointer was, from its own start, so a row stays a row. A selection
  has a primary as well as a set — the last entity pointed at — because a panel
  of fields and one set of handles can only be about one subject; the inspector
  stays on it and says how many the verbs outside it would take, and the rest of
  the selection wears a ring in the Scene view where its own handles would have
  been. Every bulk verb folds the set to its topmost entities first, because all
  of them already take the subtree: a parent and its child both selected would
  otherwise despawn the child's handle twice, land two copies of it, or move it
  by the parent's delta and then again by its own.
- **A clip can be heard and a font can be seen before either is named in a
  component.** They were the last two kinds the project browser could list and
  do nothing with, and both are decisions a filename cannot answer: which of
  four `.wav` files is the pickup, and which of four typefaces suits a score.
  Selecting an audio file offers Play and Stop, played by the editor's own
  audio device rather than by the scene — opened on the first clip rather than
  at startup, so it argues with nothing until asked, and a preview cannot leave
  a voice running in the world someone then presses Play on. Selecting a `.ttf`
  or `.otf` draws a sample in the face itself, at two sizes, over the letters,
  digits and punctuation a HUD actually uses rather than a pangram. A container
  nothing decodes is offered no play button and a `.ttf` that is not a font
  says so, because a preview whose whole job is to reveal what a filename hides
  should not hide it too.
- **A text file the browser lists can be read in the editor, and a script can be
  made there.** The project browser listed the language's own source files and
  could do nothing with any of them, in an engine whose headline capability is
  scripting. Selecting one shows it in the inspector now, the same way selecting
  an image opens the slicer: the file, its line count, and its source in a
  monospace column that scrolls both ways. Read-only, and it says so — an editor
  that opens a script in a text box is promising to be a code editor, and half
  of that is worse than none. It covers every text file the browser lists, not
  only `.decay`. **New script here** writes one that compiles and does nothing,
  because a file that reports an error before anyone has typed a line of it is a
  worse start than an empty one. Audio and fonts still have no preview: one
  needs playback on demand and the other needs the project's font in the
  editor's own text stack, and reading either as text says nothing.
- **The project browser can make, rename, copy, import and delete files.** Every
  asset used to have to arrive from outside the editor — there was no create, no
  folder, no rename, no delete, no duplicate and no import — so building a
  project meant a file manager beside the window and the Refresh button
  afterwards. A row's menu does all five now, and the directory is re-read
  afterwards, so Refresh is for changes made outside the editor rather than for
  its own.

  None of them go through the undo history, and that is not an oversight: the
  history describes a world and these describe a directory, so undoing a delete
  would mean holding the bytes of every removed file for as long as the session
  lasts. What stands in for it is that each operation is checked before it runs
  and refuses rather than overwrites; that nothing can name a path outside the
  project, because a row hands over whatever was typed into it and `../secrets`
  is a perfectly good string but not a file name; and that deleting — the one
  with nothing behind it, and which takes a whole folder — asks first.

  Renaming the open scene follows it, because the editor holds the path it saves
  to and a rename it was not told about would write the scene back under its old
  name and leave two of them on disk. A copy keeps the whole suffix that says
  what kind of asset it is, since `file_stem` stops at the last dot and a
  duplicated scene would otherwise become `level.scene copy.json`.
- **An entity's stable ID is visible and editable**, and the scene has a panel
  of its own. `source_id` is what the file keys an entity by, what a parent link
  names, what sibling order is derived from and what `sindri.grid.occupant`
  points at — and nothing showed it, so the editor could produce
  `game-object-1` and nothing else while Gather's entities are `player`,
  `floor`, `orb-1`. It is a field under the name now, and renaming one takes
  every occupant that points at it along in the same transaction: a stable ID is
  a reference, not a label, and renaming a grid without rewriting its occupants
  leaves a scene that still opens with nothing on the board. An ID that is blank
  or already taken is refused at the field rather than written and rejected.
  With nothing selected, the inspector now shows the scene rather than a shrug:
  its name — a real field that round-trips through a save and was shown nowhere
  — its file, and how many entities it holds. Both are written once the edit is
  finished rather than on every keystroke, and both go through the command
  history, so the editor still knows when the document is unsaved.
- **New Scene and Save As.** A scene file had to exist before the editor could
  do anything with it, so the tool could only continue a project someone else
  had started — and started somewhere the default scene is not, it opened
  detached with Save disabled and no way to make a file to save into. New scene
  (Ctrl+N) asks where the scene goes, writes it, and opens it through the
  ordinary path, so a new scene proves it loads before anyone works in it. It
  holds one world camera: a scene with none is legal and renders a black Game
  view, and "why is the game view empty" is not the first question a new project
  should raise. Save scene as… (Ctrl+Shift+S) is offered whether or not the
  scene has a file, adopts the path only after the write succeeds, and takes the
  project beside the scene, the remembered scene, the textures and the scripts
  with it. A save box takes a name rather than an extension, so the suffix is
  the editor's business: `level` is written as `level.scene.json`, which is what
  the project browser lists as a scene and what reopening it finds.

### Fixed

- **A script failure named a handle nobody could look up.** It printed
  `entity EntityId { index: 4, generation: 0 }`, which is what the runtime has
  and not something anyone can find in a hierarchy. `ScriptFailure` says which
  entity and what went wrong separately now, so the editor — which holds the
  world — writes "Wisp: names script 'NoSuchContainer', which
  'scripts/wisp.decay' does not declare", and the console row ends in that
  entity's name as the way to it. The entity is carried on the entry rather than
  read back out of the message: searching the text for something that looks like
  a name would select the wrong entity the first time a message mentioned a word
  that happened to be one.
- **Add Component could break the scene in one click.** It offered Camera
  whether or not the scene already had one, and a second authored world camera
  is a hard extract error — both viewports go dark with "the scene contains more
  than one authored world camera", and nothing says which two entities are now
  the cameras. Camera is listed and disabled on a scene that already has one.
- **A `.txt` file drew with the glyph the editor uses for an entity**, claiming
  it is an object in the scene, and a font shared the image glyph with a
  texture. Both have their own now.
- **Nothing in the Scene view could be selected by clicking it.** The viewport
  allocated its region with `Sense::drag()`, and egui sets a response's clicked
  flag only for a widget whose sense includes clicks — so the panel's
  `clicked_by` was always false, whatever the picking underneath decided. The
  tile brush was half-dead the same way: it painted on a drag and ignored a
  single click. The coupling between what the response senses and what the panel
  asks it is stated in a test now, because it is invisible from both ends.
- **A fully transparent element swallowed every click over it.** Found by
  clicking Gather's player and selecting its win banner instead: the banner is
  `tint` alpha zero, a third of the viewport wide, sitting in the middle of the
  scene until the game says otherwise. A thing drawn as nothing is not a thing
  to click, in the overlay or in the world.
- **A UI element's gizmo appeared where it is not.** A UI element's position is
  an offset from its anchor in overlay space, and the handle was drawn at the
  entity's transform in world space — so selecting Gather's title and choosing
  Move put one red arm in the bottom-left corner of the Scene view, mostly off
  screen, while the text was at the top. The gizmo is told where its handles
  belong now, separately from the transform it edits, so the pointer maths
  happens against the drawn origin and the answer lands on the authored value. A
  UI element is offered two arms rather than three: its Z orders it within the
  overlay rather than placing it.

### Changed

- **The snap increments can be set, and are remembered.** The snap button's
  tooltip named 0.5 units, 15° and 0.1 scale, and all three were constants
  nothing could change — so a board laid out on quarter units was a board laid
  out by hand. Right-clicking the button sets them. They live there rather than
  in a preferences dialog because that is the control they belong to, and a
  toolbar has no room for three number fields nobody usually touches. Zero is
  allowed and means that one does not round, which is what the gizmo already did
  with a zero step.
- **The console filters by level.** All, Problems, or Errors, remembered across
  launches because it is a reading preference rather than a state: someone
  watching for a failure wants it filtered to failures for as long as they are
  watching. A filter that hides everything says so, since an empty panel
  otherwise reads as a console that stopped working.
- **Delete and F2 act on whichever selection was made last.** The editor holds
  two — an entity and an asset — and these keys always meant the entity, so the
  project browser's menu could not honestly print them beside its own entries.
  Choosing something is what says which the keys mean, which is what a selection
  already communicates.
- **Add Component says what it is not offering, and why.** Sprite Animation
  needs a sliced sheet, Grid Occupant needs a grid, UI Text needs a font in the
  project — and failing any of those, the entry was simply absent, leaving the
  menu quietly shorter than the documentation. Every type the entity's space
  accepts is listed now, disabled with the reason. The rule it replaces stands
  where it belongs: a button that adds a component the engine then rejects is
  worse than no button, and an entry that says why is neither of those.
- **UI elements can be picked in the view that draws them.** Twelve of Gather's
  twenty-two entities are UI, and none of them could be clicked: an anchor picks
  a point on the viewport and the transform is an offset from it, so a world ray
  through a world camera passes nowhere near them. UI images are picked in a
  pass of their own, against the same overlay matrix the frame draws them
  through, and — being drawn over the world — take precedence over it. UI text is
  deliberately left to the hierarchy: what a string covers is decided by glyph
  layout inside the text renderer.
- **Duplicate, rename in place, and Delete, from the row they act on.** The
  three verbs whose absence is felt on every entity after the first. Gather has
  five Orbs, five Pips, and five Pip Sockets, each of which had to be built from
  scratch; renaming meant selecting a row and finding the name field in another
  panel; and Delete was one icon in a header. All three are on the hierarchy
  row's own right-click menu and on a key: Ctrl+D, F2, and Delete or Backspace.
  A rename happens in the row, focused as it appears, Enter to commit and Escape
  to abandon. A duplicate takes the whole subtree, lands beside the original as
  a sibling, earns a stable ID nothing else is using, and undoes in one step —
  which needs the copy rehearsed against a clone of the world first, because
  `WorldCommand::Spawn` names the handle it spawns at and `World::next_handle`
  answers the same thing however many times it is asked.
- **The editor answers a right-click.** There was not one `context_menu` call in
  it, which is half of why the verbs above did not exist: an action that belongs
  to *a specific thing* had nowhere to live. A hierarchy row and a project row
  have menus now, both drawn through one primitive so a menu does not change
  width with the name of whatever is selected and a destructive entry reads as
  destructive. The project row's menu offers what the browser can already do —
  open a scene, look inside a folder, slice an image — and the asset path a
  component field wants, which until now had to be read off the row and typed
  back in by hand.
- **The unmodified shortcut keys belong to whatever is being typed into.** With
  a text field now in the hierarchy, F while renaming would have framed the
  camera and Backspace would have deleted the entity being named rather than a
  letter of its name.
- **The Project dock is a browser rather than a listing.** Its folders fold, so
  a project with four asset directories is no longer a wall of every file in all
  of them. Its folder pane navigates: choosing one lists that folder and nothing
  else, and it used to be labels with no sense that selected nothing and
  filtered nothing. And it has a selection of its own, marked with the band a
  selected row wears — it used to mark only the open scene, so the scene file
  was permanently lit and clicking anything else changed nothing visible. The
  open scene keeps a quieter rule in the margin, because which scene is open and
  which asset is selected are different facts. Every row answers a click now,
  including the ones the editor can do nothing else with: what a row can do is
  said on hover instead of by refusing to respond.


- **A component added in the editor is the component the game uses.** The
  registry recorded one payload per type and asked it two questions — what does
  this component have, and what is a fresh one — so a type with no honest blank
  had no answer to the first either. `sindri.ui.text` inspected as two rows for
  a seven-field component, and a tilemap made in the editor had no `projection`
  and could never be isometric. Those are now separate registrations: a field
  template says what a component consists of, a default payload says what a
  fresh one is, and a type may have the first without the second. Both are
  checked against the field list serde will ask the type for, so a template that
  drifts from its struct is a startup error rather than a missing row noticed a
  release later.
- **A script can be put on an entity.** `sindri.script` and
  `sindri.audio.source` had no default payload and so were never offered by Add
  Component — in an engine whose headline capability is scripting, and a
  companion game that is thirteen script components. Both are offered now,
  completed from the project beside the scene: the first `.decay` source that
  declares a container, the first audio clip. A script arrives with its source,
  its container, its typed `@export` fields, and `enabled`, which had never been
  visible. Behind it, a component that says what an entity *does* no longer
  decides where it *is*: a script on a fresh entity used to mark it a world
  object and stop the menu offering UI Text, which made Gather's script-driven
  HUD unbuildable.
- **Play mode is read-only.** Saving while a scene was playing wrote the running
  world to the file — the authored scene replaced by wherever the scripts had
  pushed everything, marked as saved, with Stop then restoring a world the file
  no longer held. Every other edit made while playing was discarded by Stop
  without being mentioned, leaving undo describing changes the world no longer
  contained. A running scene is not the document: the inspector, the hierarchy's
  create and delete, the gizmo, the tile brush, undo, redo and the File menu all
  stand down until it stops, each saying why. The viewport still orbits and
  selects, and the inspector still shows the values changing.


- **The editor draws itself from one design system instead of eleven opinions.**
  Every panel used to pick its own greys, gaps, and font sizes by copying the
  panel beside it, so the same idea was spelled differently in each one and
  property values were right-aligned — no two rows in a component started at the
  same place. `editor/src/ui/` now holds the tokens, the icon vocabulary, and
  the controls built from both, and the panels are rewired onto it: a scene row
  is a banded row with a selection rule and a guide per level of nesting, a
  component is a heading that folds and carries its own actions, a transform
  component is a well with its axis letter on a spine tinted the colour that
  axis has in the viewport, and a panel with nothing in it says what it is for
  rather than being blank. Scene and Game read as workspaces rather than as two
  more tabs, the scene tools are grouped by what they do and scroll rather than
  clip when the viewport is narrow, and destructive actions — Discard changes,
  Remove component, Delete entity — are drawn as destructive. Every interaction
  is the one it was: selection, drag-and-drop reparenting, the fold
  preferences, the slicer, the tile brush, and the checked-command path behind
  every inspector edit.
- **A procedural texture stops being reported as a texture the project lacks.**
  The inspector's asset picker was built from the scene's directory alone, and
  `procedural:checkerboard` is deliberately not a file, so the fixture's own
  cube was marked as naming something that does not exist. The picker offers
  what the engine can draw, which is the project's images and the ones it
  generates.


- **A scene now has two kinds of entity, and they are spelled apart.** A sprite
  is a thing in the world; a thing on the viewport is `sindri.ui.image`, and
  `sindri.text` becomes `sindri.ui.text`. `sindri.sprite` loses the `space`
  field that used to mean one component was really two — an anchor mattered on a
  screen sprite and decided nothing on a world one — and `sindri.tilemap` loses
  it too, because a map is in the world. Scene format 8 migrates every scene:
  a screen sprite becomes a UI image with everything it drew with, a world
  sprite keeps its name and loses the two fields that decided nothing, and a
  screen-space tilemap stops the migration with a message rather than being
  quietly relocated. Scripts follow: a HUD element writes
  `this.ui_image.tint.a` where it used to write `this.sprite.tint.a`.
- **The Scene view's authored-camera gizmo stops lying about the camera.** Its
  frustum is drawn at the aspect the camera actually renders at — the Game
  viewport's — so resizing the Scene view no longer reshapes it, and every line
  is clipped against the near plane in clip space, so orbiting past a camera no
  longer smears its frustum across the viewport. The frustum is drawn for the
  selected camera only; an unselected one keeps its marker and a short forward
  stub, which is also all a click selects.
- **Play, pause, and stop are two controls and a word instead of four
  controls.** There were a stop icon, a pause icon, a play icon, and an accent
  button — and the accent button said "Stop" while running but paused when
  pressed, as did the play icon beside it. Now one button enters and leaves play
  mode and is labelled with what pressing it does, one icon holds and releases a
  running scene, and the editor says whether it is Editing, Playing, or Paused.
  Ctrl+P plays and stops; Ctrl+Shift+P pauses and resumes.
- **The inspector shows what a component has, and edits it with controls that
  know what it means.** Every field of a component is drawn whether or not this
  instance wrote it down, so two of one component no longer show two different
  sets of rows; a field left alone is still not written to the file. A field
  whose value is one of a few names — a camera's projection, a UI anchor, a
  tilemap's projection, a body's kind — is a menu rather than a text box, and
  switching a camera's projection writes the fields that projection has instead
  of producing a payload the schema refuses. A field naming a project file gets
  a picker beside it, a tint gets a colour swatch, fields are ordered by what
  they say about the component rather than alphabetically, and a row that is
  only a readout says on hover why.
- Rust source files are now capped at 600 lines, with 400 as the target, and
  `scripts/check-file-size.py` enforces it in CI. Twenty-one files were over the
  cap — the largest was the 4,714-line native editor — and each is now a
  directory module named for what its parts do. No public API changed; every
  `use sindri_core::…`, `sindri_scene::…`, and `decay_*::…` path is what it was.
  `docs/module-layout.md` states the rule and how to satisfy it.
- Decay's `&&` and `||` now short-circuit: the right operand is evaluated only
  when the left does not already decide the answer. A guard such as
  `held != null && World.exists(held)` protects the call to its right, which it
  did not before. Both operators still require and produce `bool`.

### Added

- Browser startup failures now reach the player: the engine announces them on
  `window` and the page shows them, instead of a blank canvas and a console line
  nobody opens.
- The browser page asks for a real WebGPU adapter before starting, rather than
  trusting that `navigator.gpu` existing means WebGPU works.

- End-to-end audio across assets, scene authoring, native/browser platform backends, typed Decay calls, editor discovery/component authoring, and Gather background/pickup/victory playback, with a silent backend for device-free tests.
- Command-backed Scene-view translate, rotate, and scale gizmos with local/world
  orientation, optional movement/angle/scale snapping, Z-lock-safe movement,
  Q/W/E/R tool shortcuts, and one undo step per drag; transform rotation is now
  editable as Euler degrees in the inspector while remaining quaternion-backed.
- Authored `sindri.grid_navigation` walls and `sindri.grid_occupant`
  footprints, with a world adapter that resolves stable grid references,
  derives occupancy from transforms, validates complete placement, and runs
  wall-aware whole-footprint paths.
- Normalized renderer-independent wall edges in `sindri-grid`, with bounded
  symmetric block/unblock queries and A* integration for cardinal, diagonal,
  occupancy, and multi-cell footprint paths.
- Deterministic renderer-independent A* pathfinding in `sindri-grid`, with
  cardinal/eight-way movement, explicit corner-cutting policy, integer costs,
  memoized passability, and whole-footprint occupancy paths.
- Renderer-independent multi-cell footprints and bounded occupancy in
  `sindri-grid`, with deterministic cells, atomic moves, and explicit placement
  errors for conflicts, bounds, and coordinate overflow.
- Typed Decay grid positioning through `Grid.position_x`, `position_y`, and
  `place`, with Gather gameplay migrated from top-down world coordinates to the
  exact logical space of its transformed isometric tilemap.
- Shared tilemap/grid coordinates: `sindri.tilemap` now adapts `sindri-grid`
  into upward world Y for both rendering and editor picking, while full map
  transforms rotate and scale the grid itself instead of only its tile quads.
- A dependency-free `sindri-grid` foundation with typed logical coordinates,
  finite bounds, stable neighbour queries, validated orthogonal/isometric
  projection, and exhaustive round-trip coverage across negative and positive
  space.
- A feature integration matrix tracking runtime, editor, Decay, and Gather
  counterparts together so a feature added on one surface leaves its remaining
  integrations visible.
- Drag-and-drop hierarchy reparenting onto another GameObject or the World root, with legal-target feedback, cycle prevention through the world's existing checks, and one-step undo.
- Unity-style editor hierarchy authoring: every GameObject can contain children, child-bearing rows collapse with restart-persistent state, filtered results retain their ancestor paths, and the create menu can add an empty root or child with a stable scene ID.
- Scene-viewport entity selection for world sprites, filled tilemap cells, and meshes, using the rendered camera and geometry with layer-, depth-, and occlusion-aware overlap resolution.
- Sprite-animation clip authoring in the native editor: add a valid animation from a sprite sheet, create, rename, and remove clips, arrange named frames, edit timing and looping, choose the runtime clip, and preview playback against the project texture without changing scene state.
- Text authoring in the native editor: add `sindri.text` when a project font exists, edit multiline content, and choose among project-owned font assets without hand-editing scene JSON.
- Screen-space `sindri.text` rendering through Glyphon, with anchored/layered frame extraction, validated project font assets shared by native and browser hosts, editor loading and hot reload, and a real Inter-rendered title in Gather.
- GitHub Pages delivery for the WebAssembly/WebGPU Gather build from `main`.
- World-space tilemap authoring in the native editor: a visual palette from the texture's sprite-sheet sidecar, overlap-preserving grid resizing, Scene-view paint and erase with projected cell feedback, editable render layers, and undoable drag strokes.
- Initial Rust workspace with `sindri-core` and the public `sindri` facade.
- Engine lifecycle and renderer-independent runtime host.
- Capped fixed-step clock with pause and spiral-of-death protection.
- Generation-checked entities, safe hierarchy operations, and recursive destruction.
- Versioned scene documents with stable logical IDs and hierarchy validation.
- Extensive checkable roadmap and architecture feasibility review.
- CI checks for formatting, Clippy, tests, and the declared MSRV.
- Shared `sindri-gpu` adapter/device negotiation and surface configuration policy.
- Target-independent `sindri-render` triangle pipeline.
- A single triangle example that targets native desktops and WebGPU browsers.
- Browser-target compilation in CI.
- Perspective camera matrices with projection math tests.
- Resizable depth targets and reusable indexed colored-mesh buffers.
- Depth-tested colored cube renderer with uniform-buffer transforms.
- A shared native/WebGPU cube example with frame-time-based keyboard rotation.
- Validated RGBA texture uploads with reusable texture views and filtering samplers.
- UV vertex layouts and a depth-tested textured cube pipeline.
- Procedural checkerboard texture proof shared by native and WebGPU targets.
- Offscreen color targets with aligned GPU readback and row-padding removal.
- Deterministic 512×512 headless cube capture, rendered through Mesa software Vulkan and uploaded as a PNG CI artifact.
- Aspect-correct orthographic camera with projection tests.
- Alpha-blended textured sprite renderer and a native/WebGPU cube-plus-overlay proof.
- Configurable opaque, straight-alpha, premultiplied-alpha, and additive sprite blending.
- Validated transparent draw keys with deterministic layer, back-to-front depth, and stable tie ordering.
- Dynamically growing instanced sprite batches with per-instance transforms/tints and draw-call statistics.
- Explicit frame extraction and preparation with validated viewports, clear operations, layers, and deterministic pass ordering.
- A versioned JSON scene that drives the shared cube and sprite-batch example through the same native, browser, and offscreen frame pipeline.
- Typed scene-component registration with metadata, schema validation, configurable unknown-component handling, and typed world queries.
- Native Rust editor shell with a styled hierarchy, interactive viewport composition, transform inspector, toolbar, and runtime status surfaces driven by the real demo scene document.
- Portable logical asset IDs, typed strong and weak handles, explicit load states and failures, duplicate-request coalescing, and reference-counted collection semantics.
- A cross-platform `sindri-assets` source contract with deterministic in-memory storage, root-confined native filesystem reads, and asynchronous browser Fetch API loading.
- A bounded asynchronous asset-load queue with native I/O workers, non-blocking browser future polling, generation-safe completions, duplicate rejection, and backpressure.
- Cross-platform PNG/JPEG texture decoding into upload-ready RGBA8 data, validated scene JSON decoding, and generation-checked completion application to typed asset stores.
- Shared-device editor viewport that renders the real prepared cube-and-sprite runtime frame into an egui texture, with drag orbit, zoom, resize handling, and a full-window CI screenshot artifact.
- Viewport-first editor workspace with a compact hierarchy, scene tool rail, real asset browser, inspector sections, Inter typography, and Material Symbols icons.
- Switchable perspective and orbit-matched orthographic projection for the editor's real WGPU scene viewport.
- Lossless world-to-scene saving that preserves authored stable IDs and reports runtime entities that have none.
- Deterministic ID assignment for runtime-spawned entities that skips identities already in use.
- Canonical scene serialization with sorted entities and keys, omitted empty sections, single-line scalar arrays, and fixed-point output.
- Golden scene fixtures with load, save, re-serialize, and idempotence coverage, regenerable through `SINDRI_UPDATE_SCENE_FIXTURES`.
- Namespaced editor-only metadata on scene documents and entities, carried through the runtime untouched and strippable for export.
- A scene migration API with forward-only, non-overlapping steps defined before format version 2 exists.
- Scene validation for non-finite transform values that JSON cannot represent.
- A deferred world command buffer whose commands each produce their own inverse, giving the core reversible edits rather than leaving undo to tools.
- All-or-nothing transactions that roll back applied commands when a later one is rejected.
- Bounded undo and redo history with labelled transaction grouping and an unrecorded zero-limit mode.
- Merge runs that collapse a continuous drag into a single undo step.
- Editor hierarchy, selection, and inspector driven by the live runtime world instead of a second copy of the scene document.
- Editor name and transform edits applied through world commands, with undo/redo on the toolbar and Ctrl+Z/Ctrl+Shift+Z.
- Editor play, pause, stop, and reset-to-authored-state driven by the real engine lifecycle state machine.

- A `sindri-platform` boundary crate defining what a host supplies: a clock trait, platform-independent input, and the loop that turns them into gameplay calls.
- Physical-key and mouse input with both held and per-frame edge state, ignoring operating-system key repeat and releasing everything on focus loss.
- A `Game` trait with fallible start, fixed-update, update, and stop hooks, and an `EngineHost` that reports which phase a failure came from.
- A manual clock and frame timer that make the whole loop testable with no window, GPU, or sleeping, including a frame-rate independence proof.
- Rational time scale that carries its division remainder between frames, so scaled time never drifts from the exact ratio of real time.
- Separate scaled and real frame deltas, so interface animation keeps running while the simulation is slowed or frozen.
- A `sindri-desktop` adapter translating `winit` keyboard, mouse, pointer, wheel, and focus events into platform input.
- A `sindri-scene` crate owning the built-in `sindri.camera`, `sindri.mesh`, and `sindri.sprite` schemas and deriving frames from a world, so no scene needs hand-written extraction code.
- Sprite batching per render layer instead of requiring every sprite in a scene to share one.
- Sprite anchors resolved against the overlay camera's extent, covering all nine corner, edge, and centre positions.
- Camera views that orbit, scale distance, and switch projection without touching the scene.
- A single shared colour target format, so offscreen and in-editor targets cannot disagree about colour space.
- A required sRGB swapchain format, reported as an error rather than silently accepted as a fallback.
- Verification that the headless capture actually contains the colours the scene authored, reporting the frame's dominant colours when it does not.
- Renderer texture handles and a registry, so one renderer draws every mesh and sprite in a scene instead of owning a single baked-in texture.
- Scene texture references resolved through bindings, making the `texture` field on mesh and sprite components load-bearing for the first time.
- Sprite batching split per texture as well as per layer, so a batch remains one draw call.
- A missing-texture fallback drawn in place of an unbound reference, and a report naming every unresolved reference in a world.
- The demo's badge as a real PNG decoded through the asset pipeline, connecting `sindri-assets` to `sindri-render` for the first time.
- Platform-independent asset URL resolution supporting relative, root-relative, and absolute bases, including static hosting under a non-root path.
- Percent-encoded URL path segments, so asset IDs containing spaces or non-ASCII resolve to the file they name.
- Rejection of URL roots carrying a query string or fragment, which would otherwise land in the middle of every asset URL.
- A single presentation surface policy deciding what a ready, suboptimal, outdated, timed-out, occluded, lost, or validation-failed acquisition each means, checked case by case without a GPU.
- A `WindowSurface` owning a surface, its configuration, and that recovery, so hosts acquire a frame or skip one and never write the decision themselves.
- An error for a surface that was lost and could not be built again, which is the one acquisition failure retrying does not fix.
- Apache 2.0 and MIT licence files, which every crate manifest has claimed since the workspace was created.
- Contributing guide and code of conduct, recording the conventions the codebase already follows.
- Dependency policy enforced by `cargo deny`, covering licences, sources, wildcard requirements, and security advisories across all four supported targets.
- A weekly scheduled advisory check and Dependabot updates, so a dependency problem surfaces without waiting for someone to open a pull request.
- A versioning policy for crate versions and the scene format, naming the editor protocol and npm SDK as deliberately undecided until they exist.
- A windowed host in `sindri-desktop` owning the window, event loop, device request, frame timing, and input, so an application supplies only how to build itself, what to do with a frame of time, and how to draw.
- A clock the host reads on both native and browser targets, replacing hand-measured frame deltas.
- Fallible application hooks, so a failure during a frame stops the host and is reported rather than logged and drawn over.
- A `verify` binary that reads back a captured PNG and holds it to the colours the demo scene authors, decoded through the engine's own texture decoder.
- Colour verification of the editor screenshot in CI, which was captured and uploaded but never examined.
- Gameplay running through the engine's fixed-step loop, so the demo turns at the same rate at any frame rate, proved at 15, 60, and 144 frames per second.
- An editor that opens a scene file from disk, saves the world back to it in canonical form, and reloads it, with the open file and its unsaved state shown in the status bar.
- A working File menu and Ctrl+S, replacing menu labels that did nothing.
- An entity-scaling benchmark covering spawn, iteration, typed queries, teardown, and both directions of save and load at 1k, 10k, and 100k entities.
- A `ViewportTarget` owning a colour target, the two views a Sindri target is drawn into and sampled through, and the depth buffer rebuilt with it.
- Camera panning in the editor viewport, measured in fractions of the framed height so a drag moves the picture the same distance at any zoom and under either projection.
- A reset control returning the editor viewport to the camera the scene authored.
- Editor settings that survive a launch — the project browser's presentation, the viewport projection, and the open bottom dock — alongside the window geometry and panel sizes egui persists itself.
- A project browser list view showing each asset's name and kind, now the default.
- A game view rendering the real scene through the authored camera, so what the player would see sits beside what is being edited.
- Decay, an experimental gameplay language in the isolated `decay/` workspace, with a lexer, parser, semantic analyzer, portable symbolic IR, and interpreter that depends on no engine crate.
- Decay is the decided scripting direction for Sindri Next; no embedded language is adopted.
- Decay `let` and `var` bindings execute, and a binding declares its value rather than storing it.
- Decay blocks carry their scope into the IR and the runtime, so a shadowing declaration no longer replaces the name it shadows.
- A Decay call-depth limit, so runaway recursion returns an error instead of aborting the process.
- The Decay example the documentation leads with is executed by a test, against a recording host.
- Decay's CI checks that the language compiles for `wasm32-unknown-unknown`.
- `sindri-decay`, binding the Decay language to a world: a `sindri.script` component, a host that gives Decay's symbolic paths a meaning on one entity's transform, and a driver that runs every script once a frame.
- Authored `@export` properties, carried in the scene and applied to a script instance before it starts, with a property naming an unexported or undeclared field refused rather than ignored.
- Decay scripts loaded through `sindri-assets` and hot-reloaded from the same file watch textures use.
- The editor snapshots the world when Play is pressed and restores it on Stop, so a script writing to the world cannot cost unsaved work.
- `TextAssetDecoder`, for assets that are text.
- `decay/LANGUAGE.md`, a language reference stating the grammar, what does not exist, and which behaviours will surprise you, with its claims enforced by a test.
- `docs/scripting.md`, the contract for how a script reaches a world.
- `.decay` files listed as scripts in the project browser rather than as plain files.
- Typed host members in Decay: a host describes a named type's members, and a path through a described type is checked when the script compiles rather than failing on its first frame.
- Decay host types may have methods, checked for arity and argument types like any other call.
- Reaching for `this.helper()` on a script's own function is refused with the bare-name form to write instead, rather than failing at runtime as an unknown host path.
- Sindri's script surface is described once and read by both the analyzer and the host, with a test asserting the host answers every path the analyzer accepts.
- Decay scripts can read the keyboard: `Input.axis`, `is_down`, `just_pressed`, and `just_released`, with physical key names that ignore case and a name nothing answers to refused rather than read as never-held.
- Decay scripts can ask the frame its `Time.delta` and their own `Time.elapsed`.
- Decay scripts can read and write their entity's sprite tint and layer, reaching the stored payload so a scene still round-trips byte for byte.
- `print` from a Decay script, reaching the editor console tagged with the entity that said it.
- Physical key names on `sindri_platform::Key`, with `ALL`, `name`, and a case-insensitive `from_name`.
- The editor translates its own keyboard into engine input for running scripts, and lets go of every key when play stops or a text field takes focus.
- Editing any component's fields in the inspector, driven by the stored payload rather than hand-written rows, so a component the engine has never heard of is editable too.
- Component edits checked against the component's own schema before they become commands, so an edit that would stop it decoding is refused rather than written into a scene that then will not open.
- Adding and removing components from the inspector, both undoable, with Add Component offering only types the registry can create.
- Component schema registrations may carry the payload a fresh component starts as, validated when it is registered rather than when someone clicks Add.
- A script's `@export` properties in the inspector, drawn from what the script declared — name, type, and default — with an unset field showing its default and saying so.
- Decay sources compile when a scene names them rather than when it is played, so a broken script reports at the scene it was opened with.
- Creating and deleting entities from the editor's hierarchy, both undoable, with a delete taking the whole subtree.
- `World::spawn_at`, which creates an entity at an exact handle, so undoing a delete gives back the same `EntityId` rather than a new one that leaves the selection and the rest of the undo history pointing at nothing.
- `World::next_handle`, so a caller can know an entity's handle before creating it and a spawn command can be redone onto the same one.
- `WorldCommand::Spawn`, `Despawn`, and `Restore`, each producing its own inverse, with a restored subtree returning to its place among its siblings.
- `Game.get` and `Game.set`, a board of named numbers every script on a world shares, because Decay has no value that can hold an entity and a script that needs a fact from another one has nowhere else to leave it.
- Gather, the companion game, in `game/`: five orbs on a floor, a thing you drive with the arrow keys, and a row of lamps that fills as you collect them, with all four of its rules written as Decay scripts and none of them in Rust.
- A second deterministic capture, of the companion game part-way through a scripted run rather than at rest, uploaded by CI beside the cube proof.
- `sindri.tilemap`, a grid of tiles cut from one sheet and drawn from one entity, laid out orthogonally or isometrically, whose cells join the same sprite batches loose sprites use so a prop sorts among the floor rather than behind it.
- A check that every built-in component naming a texture is one hosts actually load, since a component missing from that list draws the magenta checker while everything else about it works.
- A Decay value that can hold an entity: opaque to the language, holdable and comparable, with `World.find`, `World.exists` and `World.despawn` beside it, so one script can read and write another entity's transform and sprite instead of leaving numbers on a shared board for it.
- `EntityId::to_bits` and `from_bits`, for handing a runtime handle across a boundary that carries numbers and nothing else — never for writing to a file.

- `SpriteSheetDocument`, a sidecar naming the parts of one texture at an ID derived from the texture's own, so a sliced image says how it is cut in one place rather than in each component that draws it.
- `SpriteRef`, which splits `textures/tiles.png#floor` into a path and a sprite name, keeping `AssetId` a pure path — `#` was already reserved so a fragment could not leak into a URL.
- A check that an animated sprite asks for the sheet its clips read, whose own reference names no part of one and so is invisible to anything looking for fragments.

- A sheet slicer in the editor: selecting a texture outlines every cell on the picture, columns, rows, margin and spacing are drags, a cell is named by clicking it, and saving writes the sidecar beside the image.
- Margin and spacing on a sheet grid, so a sheet packed with gutters — which is how sheets are exported, to stop filtering bleeding one frame into the next — can be cut as it actually is.
- Sprite rows under a sliced image in the project browser, collapsed until asked for, so a sixty-four frame sheet does not flood the listing.

- `scripts/browser/smoke.mjs`, which loads a wasm-pack build in a real browser and fails when the page does not start the engine.
- `docs/browser.md`, recording what the engine's first run in a browser found.
- A browser build of the companion game, which is playable there: the keyboard drives the player, an orb is collected and a lamp lights, so Decay, entity references, the blackboard and input all run on the browser target for the first time.

### Changed

- Renamed the components a subsystem owns to hierarchical, subsystem-owned IDs
  as scene format 5: `sindri.grid.navigation`, `sindri.grid.occupant`,
  `sindri.animation.sprite`, and `sindri.audio.source`, matching the
  `sindri.physics2d.*` names 2D physics arrived with. Format-4 scenes migrate
  their keys on load with their payloads untouched, and a scene carrying both
  spellings of one component stops the migration by name rather than having one
  of them silently overwritten. Root-level singletons — camera, mesh, sprite,
  text, tilemap, and script — keep their flat names.

- Reconciled the README, roadmap, capability inventory, scripting contract, and contributor guidance with the implemented Decay entity references, tilemaps, editor authoring, companion-game browser build, and Editor + Decay product direction.

- Made a surface that offers no sRGB format draw through an sRGB view of one it can hold, instead of refusing to start; a browser canvas offers no sRGB format at all, so the engine had never once started in one.
- Made a host log the failure it records, since in a browser `run` has already handed the event loop to the page and there is nobody to return an error to — the engine was stopping at the device request in silence.

- Moved `sindri.sprite`, `sindri.sprite_animation` and `sindri.tilemap` onto named sprites as scene format 4: `uv_rect` and both sheet grids are gone, clips list names, and a tilemap's cells index a palette of names. The migration recovers a rect's cell without being told the grid, and stops rather than guessing when a rect is not a whole cell of one.
- Made a playing clip decide which part of a sheet is drawn on its own, so a frame that resolves to nothing draws the whole image rather than falling back to the clip's first frame, which was a plausible picture of the wrong moment.
- Gave the companion game one function that binds its textures and sheets, instead of one copy per binary; the window had sheets and the capture did not, which drew every sprite as its whole sheet.

- Rebuilt the companion game's orbs to ask the player where it is rather than compare against two numbers it published, which is what the blackboard was standing in for; the board keeps the score, which is a fact about the game rather than about an entity.
- Rebuilt the companion game's floor as one tilemap instead of 49 sprite entities, taking the scene from 68 entities to 20 and from 45KB to 12KB, and its hierarchy from a list you scroll past 49 rows of floor to one that fits on a screen.

- Gave every sprite batch its own uniform and instance buffers, so each draws with its own camera and its own instances; they shared one set, and because `queue.write_buffer` stages a write until the queue is submitted, every batch in a frame drew with the last one's camera — which no proof noticed, since none of them had both a world and an overlay.
- Made an animated sprite that authored no rect of its own draw the first frame of the clip it is playing, rather than the whole sheet squeezed into one quad, so a scene shows a pose before anything has run it.
- Stopped the editor reporting a script that is still loading as a compile error, which put one permanent error per scripted entity in the console on every cold open; a script that will never arrive is still reported.
- Made `scripts/capture-editor.sh` take a scene path, so the editor can be photographed against a scene other than its fixture.

- Increased the MSRV from Rust 1.85 to 1.87 to use the current `wgpu` 30 release.
- Replaced the planned Tauri/React editor architecture with native `egui`, `egui-winit`, and `egui-wgpu` integration.
- Increased the MSRV from Rust 1.87 to 1.95 for the first `egui` release aligned with `wgpu` 30.
- Replaced the editor's painted viewport composition with the actual Sindri render pipeline while retaining the native UI overlays and controls.
- Added a bounded X11 window-capture lifecycle for reliable full-editor WGPU screenshots in Xvfb CI runs.
- Rewrote the shared demo scene asset in canonical form; the extracted frame and draw order are unchanged.
- Replaced the cube example's hand-rolled key bitflags with the shared input state and the `winit` adapter.
- Replaced the demo's procedural badge with `assets/textures/badge.png`, decoded at runtime; the image is byte-identical, so the rendered frame is unchanged.
- Moved browser asset URL building out of the `wasm32`-only fetch source, where no test could reach it, into a tested module shared by every target.
- Removed the editor's duplicate left tool rail; the scene view toolbar already drove the same select, move, rotate, and scale modes.
- Replaced the cube example's bespoke extraction with the shared extractor, and moved its cube spin into the world so gameplay drives rendering through scene state.
- Fixed the editor viewport rendering into a non-sRGB target, which stored linear colour as if it were sRGB and made the scene far darker and more saturated than the offscreen capture of the same content.
- Replaced the editor's decorative transport cluster with working undo, redo, stop, pause, and play controls.
- Replaced the editor's hardcoded console and status text with the live entity count, engine state, and renderer error state.
- Fixed the editor hierarchy being clipped to the height of the adjacent tool rail, which hid most of the scene.
- Replaced editor status bullets and em dashes, which the bundled Inter subset cannot render, with a painted dot and in-subset punctuation.
- Replaced both proof examples' identical copies of the swapchain acquisition and recovery decision with the shared surface policy.
- Replaced the examples' panic on a validation error during acquisition with a skipped frame, leaving the device's error scope to report it.
- Gave the path dependencies of `sindri-platform`, `sindri-desktop`, and `sindri-scene` explicit versions, without which crates.io rejects a publish.
- Moved window creation, the browser canvas lookup, the four-state startup, and the async device request out of both examples and into the windowed host; the triangle example is now a quarter of its previous size.
- Replaced the cube example's hand-measured, hand-capped frame delta with the host's clock and frame timer.
- Fixed the editor viewport sampling its sRGB colour target through an sRGB view, so egui decoded a second time and every authored colour arrived far too dark; the scene render was correct and only the display of it was wrong.
- Moved the authored-colour expectation and its tolerance out of the capture binary so the offscreen capture and the editor screenshot are held to one definition rather than two.
- Moved the demo's cube rotation into a `Game` implementation driven by `EngineHost`, replacing a frame delta integrated by hand in the middle of rendering.
- Gave the world a single owner: a scene now carries its component schemas only, and extraction reads whichever world the engine or the editor holds.
- Moved input accumulation out of the windowed host into the engine host, so one `InputState` answers whether a key is down.
- Made the editor open the demo scene from disk rather than a copy compiled into the binary, falling back to the embedded copy and saying so when the file cannot be read.
- Made the editor screenshot wait for a frame with content, so a window grabbed before it had drawn no longer produces a blank capture.
- Fixed scene validation being quadratic in the number of entities, which every load, save, and canonical serialization paid: a ten thousand entity scene took about 1.4 seconds to validate and now takes ten milliseconds, and a hundred thousand entity scene completes at all.
- Moved the editor's viewport colour and depth targets onto the shared `ViewportTarget`, so the rule that kept them in the right colour space lives in the renderer rather than in one caller.
- Made the project browser's list and grid buttons switch the view; they were drawn but did nothing.
- Replaced the editor's game preview placeholder with a real rendered view, sharing the scene view's renderers and drawing only whichever view is visible.
- Added an editor fixture scene holding one cube, one sprite, and the two cameras they need, opened with `cargo run -p sindri-editor -- editor/assets/fixture.scene.json`.
- Added end-to-end editor tests that open the fixture, edit it through the command history, save, undo, and reload, proving an untouched scene saves byte for byte unchanged.
- Added a `Parent` menu to the editor's inspector, so an entity can be moved under another or out to the root as one undoable step.
- Added `World::check_set_parent`, which answers whether a reparent would be accepted without making it, so an interface can offer only the moves the command layer allows.
- Updated `glam` to 0.33, moving the camera matrices onto its new projection API.
- Added `perspective_projection`, `orthographic_projection`, and `look_at` to `sindri-render`, so the zero-to-one depth range and Y-up convention are chosen once rather than at each call site.
- Updated `pollster` to 1.0.
- Unpinned `png` to share `image`'s version, which stops every build compiling two copies of it.
- Added File → Open scene to the editor, so it can open a scene other than the one it was started on.
- Made the editor start with nothing selected, and let a click on the hierarchy's empty space or Escape clear the selection.
- Collapsed `Transform2D` into `Transform3D` so sprites and meshes share one world, as scene format version 2.
- Added the scene format version 1 to 2 migration, so scenes written by an earlier Sindri still open; the editor now migrates rather than parsing strictly.
- Added workspace layouts to the editor, chosen from the View menu and remembered between launches: `2 by 3` shows the scene above the game view with Hierarchy, Project, and Inspector beside them, and `Wide` keeps the previous single-view arrangement.
- Made the editor render the scene and game views at the same time in the `2 by 3` layout, rather than only whichever tab was showing.
- Fixed action failures being invisible: a failed save, open, or undo was overwritten by the next frame's render result within a frame of happening.
- Moved the frame clear out of the mesh pass and into `encode_clear`, so a scene with several meshes no longer erases all but the last, and a scene with none still starts from a cleared colour and depth buffer.
- Gave sprites a `space`: the default `screen` is the anchored overlay every sprite already was, and `world` places a sprite by its whole transform, draws it through the world camera, and lets opaque geometry in front of it hide it.
- Added the first GPU-backed tests, run in CI on software Vulkan, proving a world-space sprite is occluded by a mesh in front of it and drawn when it is the only thing in the frame.
- Made the editor's inspector show each built-in component's real fields instead of fixed text describing the demo scene.
- Sorted transparent sprites by how far they are from the camera rather than by a `depth` field authored beside them, with the render layer as the explicit override; the field is gone as scene format version 3, and the migration turns a screen sprite's depth into the Z that now orders it.
- Added `PerspectiveCamera::view` and `OrthographicCamera::view`, so where a camera is and what it looks at can be asked for without its projection.
- Added 2D-shaped accessors to `Transform3D` — `position_2d`, `set_position_2d`, `translate_2d`, `scale_2d`, `set_scale_2d`, and the turn about Z — so code thinking in two dimensions has a call that cannot flatten the Z a layered scene depends on.
- Added a Z lock a transform can declare, so an author can say that what layer a thing is on is deliberate: the command layer refuses a write that would move or drop a locked Z, and the inspector shows the lock and takes the Z drag away while it is on.
- Fixed the editor being unable to select an entity: a hierarchy row reported the surrounding layout's response rather than the row's, so no click ever reached it and every edit in the editor — names, transforms, parents, undo — was unreachable. A row now answers across its whole width.
- Fixed the editor refusing, and from the command line panicking on, a scene carrying a component its built-in schemas do not know: such a scene now opens, keeps the payload through an edit and a save, and shows its fields in the inspector.
- Made the editor ask before throwing unsaved work away: opening another scene, reloading from disk, discarding changes, and closing the window each name what they are about to lose and offer to save first, rather than doing it silently.
- Fixed the editor's Stop button discarding every unsaved edit; it now stops the engine lifecycle and nothing else, and is enabled only while something is running.
- Added `CommandHistory::revision`, which numbers the state the world is in, so a tool can tell whether the world still matches what it saved rather than tracking that a write happened.
- Fixed the editor claiming unsaved work after undoing back to the saved state, and claiming none after redoing away from it.
- Fixed Ctrl+Shift+Z performing an undo in the editor: egui ignores an extra modifier when matching, so the redo shortcut was consumed by the undo binding tested before it.
- Added `SceneExtractor::world_camera_view`, which answers where the world camera looks under a given view adjustment, so a viewport can draw its own chrome without extracting a frame or keeping a second copy of the orbit maths.
- Fixed the editor's axis indicator being painted at three fixed offsets: it now turns, foreshortens, and reorders its arms with the camera, which is the first thing in the editor that visibly answers where the viewport is looking from.
- Made the editor's project browser read the directory the open scene lives in, replacing eight hardcoded entries that named files no project contained, with the walk bounded in depth and count so opening a scene in a source tree does not stall the editor.
- Fixed the editor's asset search box accepting typing and filtering nothing: it now filters the browser to matching files, listed flat by their path below the root so a match is never indented under a parent the search removed.
- Added opening a scene by double-clicking its row in the project browser, which asks about unsaved work the same way every other way of leaving a scene does.
- Replaced the project browser's inert filter icon with a refresh that re-reads the directory, which is what a cached listing needs when a file appears beside it.
- Added an Edit menu holding undo and redo, each labelled with what it would undo; "Edit" was a label shaped like a menu that opened nothing.
- Removed the editor controls that were drawn and did nothing: the Select, Move, Rotate, and Scale tool modes and the `EditorMode` they wrote to, the "Scene", "Build", "Tools", and "Help" labels shaped like menus, the top bar's project name, the hierarchy's add-entity button, the inspector's Tag, Layer, and Add Component, the collapse chevrons and overflow menus that collapsed and overflowed nothing, and the settings gear.
- Replaced the editor's three fixed console lines with a real log: every failure, what each scene turned out to be when it opened, and every texture it names that nothing has bound, bounded and with a message repeated back to back collapsed into a count so a per-frame render failure cannot bury what explains it.
- Made the editor's error and warning counts count what the console holds, rather than saying "1 Error" for anything at all and never mentioning a warning.
- Made the editor reopen the scene it was last left in, overridden by a path on the command line, falling back to the demo scene and saying so when the remembered file has moved or been deleted.
- Named the open scene and its unsaved state in the editor's window title, so a task switcher can tell two editors apart.
- Widened the editor viewport's zoom from a factor of under three to a factor of four hundred and made the wheel move it proportionally, so a scene much larger or smaller than the demo can be framed at all.
- Added **Focus selection** to the editor viewport, on the toolbar and on F, which centres the view on the selected entity.
- Stopped an orbit being driven onto the camera's pole, where nothing says which way round the picture goes and dragging through straight down whipped the whole scene round to face the other way.
- Renamed `SceneExtractor::world_camera_view` to `world_camera`, returning a `ViewCamera` carrying the framed half-height as well as the view, which is the unit a pan is measured in and so what turns a distance on screen back into one.
- Added `AssetLoader`, which drives a store, a bounded queue, and a decoder in the one order that works, so loading an asset is a request and a poll rather than six steps that each fail quietly when skipped; requesting is idempotent, a failure is reported once rather than retried forever, and releasing says which assets went.
- Made the editor load the textures a scene names from the directory the scene lives in, through the real asset pipeline, so opening a project's scene shows that project's art; until now the editor bound two textures a demo crate handed it and drew the magenta checker for everything else.
- Added `referenced_textures`, which lists every texture a world draws with — the statement of what a scene needs loading, as against `unresolved_textures`, which is that list narrowed to what nothing has bound.
- Added `PROCEDURAL_TEXTURES`, one table of the textures the engine generates rather than loads, shared by the demo and the editor so two hosts cannot choose different colours for the same reference.
- Moved `encode_prepared_frame` and its target and renderer types out of the cube example and into `sindri-render`, which is where a stage that knows nothing about worlds or scenes belongs; the editor no longer depends on an example in order to draw.
- Made the editor's console wrap long lines, so an asset failure naming a path and an operating system error can be read rather than clipped at the edge of the dock.
- Gave the editor's fixture scene its own copy of the badge texture, so it resolves from its own directory like any other project's, with a test holding it to naming only textures that actually resolve.
- Added hot reload for native development: saving a texture the open scene uses shows the edit in the editor within about a second, without restarting and without blinking through the missing checker.
- Added `AssetWatch`, which notices that the file behind an asset changed by polling its modification time and length, and `AssetLoader::reload`, which loads an asset again because what is held is stale rather than because it failed.
- Made `TextureId` a generation-checked slot handle, so the texture registry can release a texture and reuse its slot while a handle nobody updated still resolves to the missing checker rather than to whatever landed there next.
- Made the editor release the GPU texture a reload or an edit replaced, which hot reload turned from a slow leak across a session into one per keystroke.
- Added image decoding compatibility tests that run on both native and `wasm32-unknown-unknown`, holding a corpus of every PNG colour type, sixteen bits per channel, an interlaced encoding, and a JPEG to the same pixels on both, so a texture cannot decode one way in the editor and another in the browser.
- Added `AssetManifest`, a versioned file recording each asset's length and the SHA-256 of its stored bytes, so a build knows what to publish and a load can check what arrived against what was promised.
- Made `AssetLoader` optionally verify arriving bytes against a manifest, turning a truncated response or a stale cache entry into an error naming the asset rather than a picture from last week; an asset the manifest does not list still loads.
- Made the editor pick up `sindri.manifest.json` from the directory a scene lives in, treating a malformed one as absent rather than refusing to open the scene.
- Added `docs/2d-inventory.md`, recording what each legacy 2D subsystem should become — port, refactor, replace, or defer — read from the legacy engine rather than from memory of it.
- Added a committed manifest for the demo's assets, with a test that regenerates it and compares, so editing an asset without updating the manifest fails there rather than in somebody's browser.
- Gave a sprite a checked `UvRect`, so it draws part of a texture rather than all of one and a sprite sheet becomes expressible; the rect rides on the instance, so every frame of one sheet stays in a single draw call, and a GPU test reads the pixels back to prove the shader honours it.
- Added `sindri.sprite_animation`, which cuts a sprite's texture into a grid, names clips of cells with their timing, and plays one; playback lives beside the world rather than in it, so watching an animation run does not rewrite the scene it came from.
- Made the editor's Play button actually run something: sprite animations advance while the engine is running, hold while it is paused, and go back to their first frame on stop, with the editor's fixture scene gaining a four-cell sheet that visibly spins.
