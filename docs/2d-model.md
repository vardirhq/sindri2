# How Sindri does 2D

2D is not a separate world. There is one world, one transform, and one set of
spatial rules; a 2D game is one that keeps everything on a plane and points a
camera at it.

This is the Unity model, and it is chosen for a reason that is specific to this
project rather than borrowed: `PROJECT_OVERVIEW.md` already requires that 2D and
3D share infrastructure, and the current design does not honour that. Today an
entity has a `Transform2D` **or** a `Transform3D`, so a sprite and a cube cannot
occupy the same space at all — which makes "put a 3D prop in the sprite scene"
impossible rather than merely unbuilt.

## One transform

`Transform3D` is the transform. There is no separate 2D one.

A 2D entity is an entity whose Z happens not to change and whose rotation
happens to be about Z. Nothing enforces that, because nothing needs to: the
2D-ness lives in the components an entity carries, not in its transform.

`Transform2D` is `Transform3D` with the Z position missing, and the missing Z is
the entire problem — it is what makes a 2D scene unable to have depth, and
therefore unable to have parallax or to share space with anything 3D.

## What Z means

**Z is a position. That is all it is.** Everything else is interpretation, and
each interpreter is explicit about what it does with it:

| Interpreter | What it does with Z |
| --- | --- |
| Physics | Ignores it entirely |
| Orthographic camera | Sorts by it; apparent size and position unaffected |
| Perspective camera | Sorts by it; things further away look smaller and move less |
| Transparent ordering | Derives a sort key from camera distance |

The consequence worth stating plainly: **the same scene under two cameras is the
same game.** Orthographic gives you a flat presentation, perspective gives you
parallax, and no content changes between them. Parallax is therefore not a
system to build — it is what a perspective camera does to layers that are
already at different depths.

### The trap that comes with that

An orthographic camera still clips against its near and far planes. A
background layer at Z = -50 is invisible under an orthographic camera whose far
plane is 10, even though Z does not affect its size. So the orthographic
camera's depth range must contain the Z range the scene actually uses, or
switching cameras silently loses the background.

The renderer's low-level `OrthographicCamera` still has generic projection
defaults, but an authored orthographic `sindri.camera` is a world camera and its
near/far range belongs to the scene. Screen-space UI does not borrow those
planes because it does not use an authored camera at all.

## Sorting is not the depth buffer

Sprites are transparent, and transparent geometry cannot rely on depth testing —
blending is order-dependent, so a depth buffer gives you whichever result the
draw order happened to produce.

So explicit back-to-front ordering stays. `TransparentOrder` orders by layer,
then depth, then insertion; what changed is where the depth comes from. It is no
longer a hand-authored `depth` field on the sprite — it is the distance from the
camera, measured along the camera's forward axis so that two sprites side by
side at the same depth sort as equally far away.

A UI element is the one place where that distance is not where the thing is
drawn. The viewport-owned screen projection reads X and Y for placement, so a
HUD element's Z orders it without moving it — which also means no
HUD can be lost off an authored camera's far plane by being pushed a long way
back.

**Precedence, decided once:** layer first, then camera distance, then insertion
order. A layer is an explicit authored override, so it wins over geometry; two
things in the same layer sort by where they actually are; and insertion order
breaks the remaining ties so a frame is deterministic. This is the rule Unity
has too, and it is the one people are surprised by — a sprite in a higher
sorting layer draws in front of something nearer the camera. Being surprised
once is the cost of being able to override at all.

## Physics ignores Z, and that is a hazard rather than a convenience

2D physics operates on the XY plane. A body's Z position does not affect
whether it collides with anything.

This is the right behaviour and it is also the model's sharpest edge, because Z
is now doing presentation work. A parallax background at Z = -50 and a player at
Z = 0 are, to 2D physics, in exactly the same place. Put a collider on that
background and the player walks into it.

**So collision filtering cannot use Z, and needs its own axis: collision layers
and a matrix saying which collide with which.** That is not a later refinement.
The moment Z means parallax, Z stops being usable as "these things are
elsewhere", and something has to replace it on day one of 2D physics.

**Collision layers are not render layers.** `RenderLayer` already exists and
decides draw order. Conflating the two is a standard engine mistake and the
names must stay apart from the beginning, because they answer different
questions and will eventually want different values.

### What a 2D body may write

A 2D rigidbody owns exactly three numbers on its transform: **X, Y, and rotation
about Z.**

It must never write the Z position — the first physics step would otherwise
flatten every parallax layer into the play plane. It must not write X or Y
rotation, and it must not write scale.

Stated as a rule rather than an implementation detail because it is the kind of
thing that is easy to get right once and then lose in a refactor, and the
failure is silent: the game still runs, the background is just quietly in the
wrong place.

## Keeping Z where you put it

Physics is not the only thing that can flatten a scene. Scripts, animation, and
parenting all write transforms, and the classic way a layered 2D game collapses
is a line like `position = (x, y, 0)` written by someone who was only thinking
about X and Y. Two mechanisms guard against it, and they are not equally strong.

**The 2D-shaped API has nowhere to put a Z.** A 2D transform accessor takes and
returns two numbers; setting a position in 2D reads X and Y, writes X and Y, and
cannot express a change to Z because the signature has no third argument. The
same goes for a 2D body's write-back: it does not take a Z to ignore, it takes
an XY and an angle.

`Transform3D` carries them: `position_2d`, `set_position_2d`, `translate_2d`,
`scale_2d`, `set_scale_2d`, and the pair for the turn about Z, which is the only
turn a flat thing facing the camera has. The three-dimensional fields are still
there and still public, because sometimes a 2D thing genuinely needs to change
layer; what the 2D calls do is make that a thing you say on purpose.

This is the one that actually works, because it is not a rule anyone has to
remember. Anyone thinking in 2D reaches for the 2D call, and the 2D call is
incapable of the mistake. It is the same reasoning that made colour space a
shared constant rather than a convention.

**An explicit Z lock, for everything else.** The 3D API still exists and can
still write Z, which is correct — sometimes a 2D thing genuinely needs to change
layer. A transform may declare its Z locked, and checked write paths respect it.

Be honest about the difference: this is a check, not an impossibility. It holds
for writes that go through a path that can check, and a direct field assignment
bypasses it. It earns its place by being visible in the inspector and by saying
what the author meant — "this stays on its layer" — rather than by being
airtight. The API shape above is the real defence.

It lives on the transform, as `z_locked`. The alternatives were a component of
its own and a flag on the 2D body, and the transform wins because the lock is
about one number on the transform: a component would have to be looked up by
every path that writes a position, and a body would only cover the entities
that have one, which is the wrong half — the scripts and animations that
flatten a scene mostly belong to things with no physics at all.

What respects it is `WorldCommand::SetTransform3D`, which is the single write
path tools use, so the check sits where the editor and other command-driven
authoring paths pass through. The Decay host enforces the same lock at its world
boundary. A refused command changes nothing
and never enters the history, so a transaction that contains one rolls back
whole.

Removing a transform counts as moving it. An entity without one is at Z = 0, so
dropping a locked transform lands a parallax layer in the play plane exactly as
writing a different number would.

## Pixel-perfect requires an orthographic camera

Pixel art wants one texel to land on one pixel. Under a perspective camera it
cannot, because apparent scale depends on Z, so different layers land on
different fractions of a pixel.

This is not a limitation to engineer around; it is why Unity keeps both cameras
and why Hollow Knight can use a perspective one — it is painted at high
resolution rather than being pixel art. The choice belongs to the game.

`ROADMAP.md`'s pixel-snapping item is therefore an orthographic-camera feature,
and should say so rather than being attempted generally.

## Screen space is a different question

A HUD is not in the world at all, so it is not on the 2D/3D axis and it is not a
property of the world camera. It is a property of **which component draws it**:
`sindri.ui.image` is anchored to a viewport edge and ignores authored cameras
entirely, while `sindri.sprite` is placed by its transform, drawn through the
world camera, and hidden by opaque geometry in front of it.

This started as one component with a `space` field, and that was wrong in a way
worth writing down, because the same mistake is available in every engine. The
two spaces do not hold the same fields: an anchor is what a HUD element is
positioned by and decides nothing at all for a thing in the world. So the
inspector hid the anchor on a world sprite — which is the clearest possible
statement that these were two components sharing a name. A component that has to
hide half of itself depending on one of its own fields is two components, and
they are now spelled as two. `docs/scene-extraction.md` holds the table of what
each draws.

That split is also what gives a scene two kinds of entity, in the way a Unity
scene has objects and UI. Nothing declares which an entity is: carrying a
`sindri.ui.*` component says it, the editor groups the hierarchy by it, and the
Add Component menu offers one family or the other rather than letting one
transform be drawn twice in two spaces.

The two are separated at the batch, not at the draw: a UI image and a world
sprite never share a draw call, because they differ in projection and pipeline.
Each sorts within its batch by its own depth rule, so a world sprite's order
changes when its camera moves while a HUD's remains tied to screen-space Z.

## Scene format

Replacing `Transform2D` with `Transform3D` changes the serialized format, so
this is scene format version 2 and the first real use of `SceneMigrator`, which
was deliberately built before a second version existed.

The migration is mechanical: a `transform_2d` becomes a `transform_3d` with
Z = 0, its `rotation_radians` becomes a quaternion about Z, and its two-component
scale gains a Z of 1. Nothing is lost.

One case is not mechanical, which this document originally missed. Format 1
allowed an entity to carry both transforms, and they described positions in
different spaces, so there is no merge of them that is reliably the same scene.
The migration refuses such an entity and names it, rather than preferring one
and moving something without saying so.

Golden fixtures cover both versions after this: a version 1 document must still
load, and must produce exactly the version 2 document the migration promises.

## What this removes from the plan

- **Parallax** stops being a system to port and becomes a consequence of depth
  plus a perspective camera
- **Milestone 9's dual presentation** — its exit gate asks that shared gameplay
  logic support both sprite isometric and orthographic 3D. Under one world with
  one transform that is a camera choice, not two presentation paths
- **Mixing 2D and 3D** stops being a feature at all, which is what makes
  Milestone 8's "3D prop in the sprite scene" trivial rather than impossible

## What it does not answer yet

- Whether sprites become real quads in the 3D pipeline or keep a dedicated
  sprite path. Sorting and batching argue for keeping the sprite path; sharing
  argues for quads. Decide when there is a reason to, not before
- Where collision layers are defined — project settings, the scene, or both
- How the editor presents a Z that matters for depth but not for gameplay,
  which is a real authoring problem Unity has never fully solved
