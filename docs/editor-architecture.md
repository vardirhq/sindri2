# Editor architecture decision

Sindri's editor is a native Rust application built with `egui`, `egui-winit`, and
`egui-wgpu`. The editor and game preview share the same `winit` event loop and
`wgpu` device so the viewport can render through the real Sindri runtime without
a browser boundary or a second graphics stack.

The initial shell uses `eframe` to establish the visual language and editor
workflow quickly. Runtime viewport work must use the exposed WGPU render state;
the editor must not create a second device or reimplement scene rendering. Once
the shared desktop platform host exists, Sindri will own the event loop directly
and retain the same `egui` UI layer.

## Dependency direction

```text
sindri-editor
      |
      v
editor commands / protocol
      |
      v
public Sindri engine crates
```

Engine crates never depend on the editor. Editor interactions will become
versioned commands before undo/redo, remote inspection, or AI-assisted actions
are added.

## Visual principles

- Dense, calm workspace rather than default toolkit styling.
- Clear hierarchy between authored content, tools, and runtime status.
- One warm Sindri accent; semantic axis and status colors remain distinct.
- Useful at 1080p, with resizable hierarchy and inspector panels.
- Visual polish is maintained continuously rather than postponed to a rewrite.

## The design system

`editor/src/ui/` is the only place the editor decides what it looks like. It
holds three things and nothing else:

- `theme` — the tokens: the palette, the metrics, the four text sizes, and the
  egui `Style` built from them, so a stock `ComboBox` inside a component section
  already looks like the editor without its call site dressing it.
- `icons` — the icon vocabulary, by meaning rather than by glyph. A camera in
  the hierarchy, in a component heading, and on the viewport's camera control is
  one idea and one table entry.
- `widgets/` — the controls built from both.

Nothing in `ui/` knows what a scene, an entity, or a command is. That is the
boundary that makes it a layer rather than a second copy of the editor, and it
is why a panel under it reads as *what it does* instead of as a list of colours
and offsets.

**The rule that keeps it worth having: a panel does not name a colour, a gap, or
a font size of its own.** A panel needing one that is not there means the token
is missing, and it belongs in `theme`.

A widget belongs in `widgets/` when it centralises painting, spacing,
interaction, or meaning across more than one panel. A wrapper that only renames
an egui call does not: it adds an import list and centralises nothing.

Three of these are painted rather than assembled out of nested layouts, because
an egui layout inherits its parent's direction and a `Frame` inside one takes
the width it is given. The search box drew its magnifier on the far side of the
field inside a right-aligned toolbar; the segmented switch stretched to the
width of whatever panel held it. A control whose appearance must not depend on
where it is used is measured and painted into a region it allocates itself.

Two consequences of that are worth knowing before writing another one:

- A galley laid out with a real colour keeps that colour, so a painted control
  that tints its text at paint time must lay it out with `Color32::PLACEHOLDER`.
- Ids for sub-parts come from the control's own allocation, not from `ui.id()`,
  which every widget in one panel shares.

`widgets/menu` is the other kind of centralising: not painting, but the two
things every call site would get wrong on its own. A menu opens at a fixed
width, so it does not resize with the length of the name of whatever happens to
be selected; and a destructive entry reads as destructive rather than as one
more neutral line of text. A menu cannot return a value through egui's closure,
so every one of them writes what was chosen into a slot the caller owns and the
panel acts on it after the listing has finished drawing — which it must, because
every verb worth putting in a menu writes to the world the rows were read from.

The alignment the inspector depends on is `property::Property`: a fixed label
column, and a value column that begins at the same x on every row in the window.
egui allocates a child region by what its contents measured rather than by what
was asked for, so that column only holds its width because the widget sets it.

## First shell boundary

The editor opens a scene file, displays its entity hierarchy, selects entities,
exposes editable transform values, and renders the prepared Sindri
cube-and-sprite frame into a texture registered with egui. The runtime target
and editor UI share eframe's WGPU device and queue; resizing rebuilds the
viewport's colour and depth targets together through
`sindri_render::ViewportTarget`, which also owns the rule that a target drawn
into through sRGB is sampled through linear.

## Two views of one world

The scene view is where the editor moves around: orbit, pan, zoom, and a choice
of projection, all through `CameraView`. The game view renders the same world
through the authored camera and nothing else, which is the only question it
exists to answer — what would the player see.

That distinction is a tested rule rather than a convention. `camera_for` maps a
tab to a camera, the game tab maps to `CameraView::default()`, and a test holds
it there: an orbit or a pan leaking into the game view would quietly turn it
into a second scene view.

The two share their renderers and textures, because pipelines do not depend on
which camera is looking, and each owns a `ViewportTarget` so egui has a texture
per view. Only the visible one is drawn — rendering the hidden view would spend
a frame's GPU work on something nobody is looking at.

The game view carries no editor chrome: no selection label, no camera hints, no
axis gizmo. A render failure is still reported across it, because a blank view
with no explanation is worse than a view with a message on it.

## Moving the view without moving the scene

Primary drag orbits when no editing handle owns it, secondary drag always
orbits, middle drag or shift-drag pans, and the wheel zooms. All four go through
`CameraView`, so the authored camera stays exactly where the scene put it and
only the editor's view of it moves — which is why a save after looking around
writes nothing. Panning can carry the subject off screen, so there is a reset
control rather than an expectation that the viewer finds their way back.

Q/W/E/R choose Select, Move, Rotate, and Scale. A selected transform's handles
are projected and hit-tested from the same `ViewCamera` that drew the world.
When a handle is hovered or dragging it takes primary input before camera orbit
or viewport selection. Local/world orientation changes move and rotation axes;
scale remains local because `Transform3D::scale` is local. Optional snapping
quantizes translation, rotation, and scale, and a locked transform's movement
axes are constrained to its XY layer. Every frame of a drag is a
`SetTransform3D` transaction with one merge key, so release leaves one undo
step rather than a trail of intermediate transforms.

When tile painting is enabled, primary drag belongs to the brush and secondary
drag orbits; middle or Shift-drag still pans. A pointer is not interpreted by a
second copy of the camera maths. `SceneExtractor::world_camera_for_viewport`
hands the editor the exact view-projection used for that viewport, which the
tilemap tool inverts into a local-space ray before asking `local_to_tile`. The
same answer projects the hover outline back to the screen. Each changed cell is
a `SetComponent` command, with one merge key for the drag, so release makes the
whole stroke one undo step.

The tile palette is derived from the selected map's texture and sprite-sheet
sidecar and cached until either identity changes. It edits the existing compact
palette and flat cell array rather than replacing the component with a typed
serialization, preserving fields from newer scene formats. Grid resizing keeps
the old overlap and fills new cells with `null`. Direct painting is world-space
only: a screen-space tilemap belongs to the Game view, which intentionally does
not accept editor input yet.

## Defaults, and what is remembered

Settings survive a launch through eframe's storage: the project browser's
presentation and how much of the project it lists, the viewport projection, and
which bottom dock is open, alongside the window geometry and panel sizes egui
persists itself. Anything derived from
the scene, the selection, or the current camera is state rather than preference,
and restoring it would be restoring a moment rather than a choice.

Persistence is what makes a default a small decision. A default only has to be a
reasonable first guess, because disagreeing with it costs one click ever rather
than one per launch, so defaults are chosen on their merits and not to satisfy
whoever complained most recently.

Two of them are worth writing down:

- **The project browser opens as a list.** The grid's tiles show a generic icon
  per file type rather than a picture of the asset, so until a thumbnail is a
  thumbnail the grid spends more space to say less. This flips back when there
  is something to look at.
- **The project browser opens on the assets.** A project holds more than what a
  scene can name — Gather keeps a Cargo manifest, a `src/`, a `tests/`, and a
  web page beside its `assets/` — and none of that is something an inspector
  field can point at. The listing starts at the directory asset references
  resolve against, and the rest of the project is a switch in the browser's
  toolbar rather than hidden: deciding what someone's project contains is not
  the browser's to decide, but neither is burying the textures in it.
- **The workspace layout stays viewport-first**, as `design-qa.md` chose. A
  layout that quarters the viewport to fit a second view is a different product
  decision, not a preference, and it belongs behind a named layout rather than
  in the default.

## A project is a directory, and a scene is a file in it

The editor's unit of work is a project: a directory containing `sindri.toml`.
Before that file existed, "the project" was a word for whichever folder the open
scene happened to sit in — enough to browse assets beside a scene, and not enough
to have a name, to be listed, or to be told apart from any other folder with a
`.scene.json` in it.

The welcome window is the front door and is its own window, with the editor's
hidden until a project is open. `docs/project-format.md` is the contract: what
the file holds, what creating a project makes, which project a launch opens, and
why the window is a deferred viewport rather than a screen inside the editor.

Two things about it belong here, because they are about the shell rather than
the format. The editor's window starts hidden and is revealed when a project
opens, which is also what makes closing the welcome window with nothing open
close the editor — a running process with no window is a process with no way
back to it. And a scene carries its project with it: every path that opens a
scene walks up to the nearest manifest, so the browser roots at the project and
the header carries the project's name, while a scene in no project leaves the
editor with none. That last state is not a degraded one. It is what the editor
did before projects existed, and it is still how you edit one file.

## A scene is a file

The editor takes a path — one named on the command line, the one a project
nominates, or the one it was last left in — and reads it from disk. A missing or unreadable file named on the command line
is reported in the interface and the editor opens on nothing, so it starts
anywhere while saying what went wrong — and never by quietly standing another
scene in for the one somebody named.

Saving writes the world back through `World::to_scene` and canonical
serialization, which is what makes it safe to offer: saving a scene nobody
edited reproduces the file byte for byte, so a review sees the edit and nothing
else. Reloading re-reads the file and discards unsaved edits along with their
history, because every runtime handle is replaced.

Which is why nothing throws that work away without asking. Opening another
scene, reloading, discarding changes, and closing the window each raise the same
question, named after the loss they are about to cause, with saving offered as
the third answer; closing cancels the window's close request while the question
stands, and asks again once it is answered.

Whether there is anything to lose is a question for the command history, not a
flag. `CommandHistory::revision` numbers the state the world is in; the editor
remembers the number it last saved, and unsaved work is the two differing. A
flag cannot say this: a merged drag changes the world without growing the stack,
and a bounded stack repeats its depths once it starts dropping entries, so
neither "something was written" nor "the stack is this deep" is the same
question. Undoing back to what was saved reads as saved again, and a state the
history left behind is never numbered twice.

Which project is open is a preference rather than session state, and the distinction is worth naming
because everything else the editor remembers is a choice: it is not where the camera happened to be
pointing when the window closed, it is what someone is working on. A path on the command line still
wins. A remembered project that has moved or been deleted since falls back to the welcome window
rather than to an error — that choice was made last week and its failure is not the user's doing
now. Which scene inside the project is remembered too, so reopening a project puts someone back
where they were working rather than at its front door. The window title carries the file name and
unsaved marker the status bar does, so a task switcher can tell two editors apart.

## A scene brings its own textures

The editor used to bind exactly two textures: a checkerboard and a badge, both handed over by the
cube example, both named by the demo scene. Every other reference in every other scene drew the
magenta missing checker. Nothing had failed to load, because nothing had been asked to load.

A scene's texture references say what it needs, and the directory the scene lives in is where they
resolve — the *scene's* directory, which is not the project root whenever a project keeps its scene
under `assets/`; `docs/project-format.md` has why that distinction is worth writing down twice. So
opening a project's scene shows that project's art. References that name a file go
through `sindri-assets` — a `FileSystemAssetSource` rooted at the scene's directory, feeding the
bounded queue, decoded and uploaded when they arrive. References the engine generates are bound
before any of that starts, and the two cannot be confused because a procedural reference is not a
valid `AssetId`.

Opening a different scene builds a new set rather than re-rooting the old one, which is also how the
previous scene's textures are released: the registry that owned them is dropped, and a `Texture2D`
frees its GPU texture when it goes. Within a scene, an edit that points a mesh somewhere else asks
again — the world is the only statement of what is referenced, so a change to the history's revision
is the signal to re-request, and the loader coalesces so asking about a whole world is cheap.

Loading is genuinely asynchronous, which shows: a scene opens and its textures appear a frame or two
later, drawing as the missing checker until they do. That is the honest behaviour and the one the
browser will force anyway.

Once a texture has loaded, the file behind it is watched. Saving it in an image editor loads it
again and rebinds, within about a second, without restarting anything — and the binding is left
pointing at the old texture until the new one arrives, so an edit does not blink the scene through
the missing checker on its way to showing itself.

## An image is sliced where it can be seen

A sprite sheet is a property of a picture, so slicing one happens on the picture.
Selecting a texture in the browser shows it in the inspector with its grid drawn
over it; columns and rows are drags; each cell takes a name; saving writes the
sidecar beside the image.

Drawn on the image because that is the entire job. A grid of numbers in a panel
says nothing about whether the lines fall between the frames, and a slicer exists
to answer exactly that. The preview samples nearest rather than linear, because a
sheet is usually pixel art and a slicer that blurs what it is cutting is showing
the wrong picture of it.

Cells are outlined individually rather than drawn as lines across the image. A
cell inset by a margin is not on any dividing line, and drawing one would claim
the gutters belong to a sprite. The rects outlined are the ones the *document*
produces — a slicer whose picture and whose output are computed separately is a
slicer that can lie.

A cell is named by picking it on the image. A text field per cell is fine at four
and unusable at two hundred and fifty-six, which is the wall a list of forty-nine
floor tiles already found; so the panel names one cell at a time and lists only
the cells that were given a name. Everything else already has an answer — its
index — and a list of two hundred and fifty-six of those is not a review of
anything. That list doubles as the way back to a cell on a sheet too large to
scan.

The image is decoded on the CPU and handed to egui, not put through the
renderer's `TextureRegistry`. That registry exists to draw a scene; a picture of
an asset nothing in the scene references does not belong in it, and would then
have to be released when the selection changed.

The parts of a sliced image are listed under the image in the browser, because
that is where a person looks for them — they belong to the picture, not to the
directory. Collapsed until asked for: a sheet is as likely to hold sixty-four
frames as four, and a browser that cannot be scrolled past is the failure the
hierarchy already demonstrated with forty-nine floor tiles. The sheet *file* is
not listed beside them, since its whole content is already showing; an orphaned
sheet, whose texture is missing, still is.

A sprite row is not a `ProjectEntry`. A sprite has no file, and giving it one
would offer it as something that could be opened, renamed, or deleted on its own.

## What the editor has to say

Anything the editor reports goes to two places at once, and the split is what makes each of them
work. The notice beside the viewport is one line and is replaced by the next thing that happens; the
console keeps everything, in order. Every failure goes through one call so the two cannot disagree
about what happened.

The console is bounded, and it collapses a message repeated back to back into a count. That second
rule is not tidiness: a render failure recurs every frame, and without it two hundred copies of the
same line would push whatever explains it out of the top within four seconds. It is also what lets
the status bar count errors and warnings honestly — one thing wrong, however many frames said so.

A scene announces itself when it opens: what it is called, how many entities it holds, and every
texture it names that nothing has bound. That last one is why the console had to become real.
An unresolved texture draws the magenta checker rather than failing the frame, which is the right
call and also means being told is the only way anyone finds out; `unresolved_textures` has existed
since bindings did and nothing asked it.

That closes the loop the milestone is judged on — edit a transform, save,
reopen, and the scene is what it was left as — and it is the same file the
runtime and the headless capture load.

A versioned editor/runtime protocol, and editing anything beyond names and
transforms, remain explicit follow-up work.

## Looking at the game the shape a player will

The Game view draws at the shape of a screen rather than at the shape of the
panel it is in. The panel is whatever the window and the splitter left it, which
is a shape nobody plays on — and the overlay a scene is authored in is *as wide
as the aspect ratio*, so the shape is not a cosmetic question. A menu arranged
in a wide editor panel runs off the side of a phone, and until the view could be
told to be a phone there was no way to find that out except by building for one
and looking.

`Screen` in the Game view's strip picks one. The list is short on purpose: a
wide desktop, a squarer laptop, a tall phone, a phone turned sideways, a tablet
— shapes that behave differently, rather than a catalogue of handsets that
differ by a few pixels and tell a designer nothing. `Free` is the panel's own
shape, and the default.

The chosen shape is what the engine is handed, not a crop of a bigger render:
the viewport, the overlay's aspect, and a camera's `fit` all resolve against it,
so what is on screen is what would be on screen. It is also what the pointer is
made relative to, so a button previewed at phone size is clicked where it is
drawn.

It is not a preference and does not outlive the session. It is a thing to look
through while arranging a screen, not a setting about the editor.

## The UI is a canvas in the scene

The overlay a UI element is laid out on is pinned to the viewport. In a game
that is exactly right — it *is* the screen, and no camera can move it, which is
why deleting a gameplay camera cannot lose a HUD. In the Scene view it was
wrong in a way that is obvious the moment you try to work: panning and zooming
moved the world and left the UI stuck to the glass, so there was no way to look
closely at a menu, and no way to see that an element was off the edge.

`UiCanvas` says which of the two an extraction wants. The Scene view asks for
`InScene`, and the overlay becomes a rectangle in the world — two units tall,
one overlay unit to one world unit, centred on the origin — drawn through
whatever is looking at the scene. Pan and zoom reach it because they reach
everything.

Its shape is the **game's**, not the panel's: the chosen device preview, or the
Game view's own shape when that is `Free`. A canvas that reshaped itself around
the editor window would be a canvas that never showed what a player gets, and it
would change every time the splitter moved.

Three things follow from the canvas rather than being arranged separately, and
all three ask the same functions the frame was drawn from:

- **Its outline is drawn**, so the edge of the screen is visible. Without it the
  overlay is things floating at the origin and "runs off the side" is not
  something a picture can show.
- **Clicking resolves through it**, so selecting a button means clicking where
  the button is rather than where it would be if it were still on the viewport.
- **A UI element's gizmo is on it**, so handles travel with the element when the
  view is panned to it.
