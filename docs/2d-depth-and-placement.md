# Depth and placement in 2D

Status: implemented for grids. `sindri.grid.placement` derives a transform
from a cell, `depth_step` turns projected depth into Z, and Gather's
seventy-four authored layers are gone. What remains unbuilt is listed under
*What this does not decide*. This continues `docs/2d-model.md` and answers the
question it left open.

Sindri is not going to be the best 3D engine. It can be the best 2D one.

That is a position rather than a consolation. Most engines reach 2D by turning a
3D camera side-on, so their 2D is an arrangement over a general renderer and
their answer to depth is "sort by Y yourself, in game code". Sindri decided
something better in `docs/2d-model.md` — Z is a position, the camera sorts by
it, and parallax is a consequence of depth rather than a system to build — and
then never connected it to the thing a 2D game actually has: a grid.

This note connects them.

## The open question

`docs/2d-model.md` ends by naming what it did not answer:

> How the editor presents a Z that matters for depth but not for gameplay,
> which is a real authoring problem Unity has never fully solved.

Gather answered it by not answering it. Every prop sits at `Z = 0` and carries a
hand-written `layer` instead, and because the decided precedence is *layer
first, then camera distance*, those authored numbers now beat the depth system
outright. Seventy-four overrides, in a scene whose depth model was supposed to
be geometric.

The answer this note proposes: **the author never manages Z.** They state a
cell. Placement derives the transform from it, Z included, and depth follows
from position — which is what `docs/2d-model.md` wanted in the first place.

## The measurement

Every layered draw in every scene checked into this repository:

| Scene | Draws | Range | Distinct |
| --- | --- | --- | --- |
| `game/assets/gather.scene.json` | 89 | 0..120 | 45 |
| `games/orbital-last-stand/assets/orbital.scene.json` | 67 | -200..140 | 10 |
| `games/graphics-lab/assets/graphics-lab.scene.json` | 89 | -120..30 | 23 |
| `games/orbital-baked/assets/orbital.scene.json` | 72 | -200..140 | 10 |
| everything else | 72 | 0..101 | ≤ 3 each |

Orbital's ten layers across a wide range are *groups*: backdrop, play field,
effects, interface. That is what a layer is for, and nothing here asks those
scenes to change.

Gather's forty-five across a narrow one are a *depth*. They are not arbitrary
either — every one of its seventy-four world sprites sits at exactly

    layer = 1 + 2 × (column + row)

which is the isometric diagonal, computed by hand and frozen into the file.

One field, two jobs. Counting the distinct values tells you which job a scene is
asking of it, and a scene asking for the second one is a scene with no other way
to say where something stands.

## What this costs today

All of it observed, none predicted:

- seventy-four integers that must be right by hand, correct only because a test
  asserts the formula;
- a tile volume that needed a `layer_step` field invented for it, purely to
  reach the axis the sprites had already taken over;
- a ground plane that moved down one level, after which every integer was stale
  and the props stood inside the floor;
- a prop and a block beside it sorting by two integers, when a sprite is taller
  than the cell it stands on.

The last is not a bug. It is the shape of the problem, and the section below on
single-point depth says what it does and does not solve.

## Invariants

1. Depth is a consequence of position. Z is the depth axis, as
   `docs/2d-model.md` decided; nothing authors a second one beside it.
2. Position on a grid is stated as a cell. The transform — X, Y and Z — is
   derived from it.
3. `layer` groups. It remains an override and keeps winning, because overriding
   is what it is for; it stops being how ordinary scenery is ordered.
4. A ground anchor is the one point deciding both where a sprite is drawn and
   how deep it is. Two authored values that can disagree is one too many.
5. Depth does not change because art changed. Redrawing a tree taller does not
   move the tree.
6. Elevation raises a thing on screen without moving it toward the viewer.
   Already true of tile volumes; it must become true of everything.
7. Orthogonal and isometric share the rule. The projection supplies the
   arithmetic; it does not get a rule of its own.

## Deriving depth from the grid

A grid already puts cell `(c, r)` on the plane at:

    orthogonal   y = -r · h
    isometric    y = -(c + r) · h / 2

Descending `y` is ascending `r` in one and ascending `c + r` in the other — the
row order and the isometric diagonal. `TileGridComponent::depth_key` already
computes exactly these two for tile volumes. A grid therefore already knows how
far into the scene each of its cells is; no new geometry is required, only the
decision to let that number reach the transform.

A cell further from the viewer takes a Z further from the camera. The scale is
the grid's to state, and two constraints bound it: the orthographic camera's
near and far planes must contain the range (the trap `docs/2d-model.md`
describes), and the step between adjacent cells must stay large enough to
survive `f32` comparison across a big map.

Under a perspective camera the same Z produces parallax, for free, exactly as
`docs/2d-model.md` predicted — with the honest consequence that a distant cell
also draws slightly smaller. That is a property of the camera, and a game that
does not want it keeps the orthographic one.

## Grid-anchored placement

`sindri.grid.occupant` derives a cell *from* a transform: the scene authors a
world position and the runtime works out where it landed. Everything in Gather
is placed that way, which is why a standing stone can straddle the boundary
between two cells, and why nothing moved when the ground beneath it did.

The inverse is missing. A placement component names a grid and a cell, with an
optional offset inside the cell, and the runtime derives the transform —
including the surface height, which `TileSurfaces` already computes and which
already understands slabs and stacked levels.

What follows:

- a prop cannot straddle a cell it does not occupy;
- raising the ground raises what stands on it, because the transform is a
  consequence rather than a copy;
- the editor places a prefab by clicking a cell, which is the gesture the block
  tools already have and the prop tools do not;
- depth comes free, because Z is derived along with X and Y.

Occupancy then stops being derived from a transform that was itself derived
from nothing.

The offset inside a cell is deliberate. Snapping every prop to a cell centre
makes a grid look like a spreadsheet; an offset lets a scene nudge a rock
off-centre without leaving the cell that owns it, and without reintroducing a
position nobody can trace back to a cell.

## The tile volume stops being special

A volume's cells take their Z from grid depth by the same rule, so `layer_step`
— invented because a volume had nowhere else to go — is deleted rather than
generalised. A block and a prop then sort against each other by position,
because they are finally measured on the same axis.

## Ground is not a wall

Depth alone put the ground in front of the player standing on it. The column
ahead of a walker is nearer than the one under their feet, so a flat top face at
exactly the height they stand at won on depth and drew over their legs.

No single depth fixes it. In an isometric projection a walker on a cell of depth
`d` has ground a step ahead at `d + 1` and a wall a step ahead at `d + 1` too:
the first has to lose to them and the second has to beat them, and one number
cannot do both. Giving top faces and side faces different offsets only moves the
contradiction — a cell's wall then sorts a full cell ahead of its own top, and
the wall of a block paints over the ground in front of it.

So the rule is not about the ground. It is about what stands on it: **a cell
whose surface is no higher than your feet has no face that can cover you.** Not
its top, and not the walls holding that top up. A walker therefore sorts one
cell forward, past the ground it is walking into, and terrain keeps the depth it
always had — cells still sort against each other by the column they stand in,
and every ordering already established here is untouched.

Only the cells a step ahead are consulted. A diagonal neighbour is two steps of
depth away and its top never rises far enough to reach the sprite. A step ahead
that *is* higher takes the forward sort back, because then it is a wall and
covering the walker is its job.

One case has no answer: standing in the corner between open ground one step
ahead and a raised block one step ahead. The ground wants the walker moved
forward and the wall wants it left alone, and a single depth answers for one of
them. It answers for the wall, because a walker drawn through a wall is worse
than a seam against the ground. `docs/parity.md` carries the row.

## Standing everywhere

A rule can be right everywhere anybody thought to look and wrong on the open
ground where the player walks, which is how the ground came to be drawn over
them on a phone rather than in a test.

`sweep_occlusion` puts a virtual actor on every column of a volume that has
ground in it and asks the two questions the renderer asks — `face_depth` for a
cell and `standing_depth` for the actor, the functions themselves rather than a
restatement of them — then reports every cell that cannot cover the actor and is
drawn after it regardless. Each finding carries the raised step that took the
actor's clearance away, so the known corner is separated from a fault nothing
accounts for; `OcclusionReport::unexplained` is the one a test asserts on.

The probe is a size and an anchor rather than a sprite, because how far an actor
reaches decides which cells can reach it back. A sweep run with the wrong actor
proves something about a different game.

On Gather: 387 standable columns, the corner reached in 22 of them, all of them
a walker beside the raised edge of a plot.

## What one point per sprite cannot do

A tall sprite has one ground point and therefore one depth. A tree whose trunk
belongs behind the block in front of it, and whose crown belongs in front of the
block behind it, cannot be expressed by one number, and this note does not
pretend otherwise.

Three ways out exist, none chosen here: splitting the art into bands at bake
time, a per-pixel depth channel, or a declared height that sorts a sprite's
upper band separately from its base. Every 2D engine lives with the
single-point rule and accepts the artefact at the margins.

What this fixes is the ordering that is wrong *today*, where the failure is not
a tall tree at a boundary but a waystone drawn behind the ground it stands on.
Those are different sizes of problem, and only one of them is worth a per-pixel
depth buffer.

## Migration boundary

`layer` keeps working, keeps its meaning, and keeps winning. Depth orders
*within* a layer, so every scene in the repository draws as it does today until
its own props move onto the grid.

Gather's seventy-four integers are gone, and their deletion is the proof: a
scene that renders with every authored layer removed has demonstrated that
derived depth reproduces what the hand computed. `layer_step` went with them.
Two Decay scripts that computed the same formula at runtime — the player's and
the wisp's — lost those lines too, which is the same statement made in the other
place it was being made.

Batching improved as a side effect. Gather drew 104 sprite batches with
forty-five distinct layers and draws 85 with one, because a layer boundary is
also a batch boundary and most of Gather's were describing depth rather than
grouping.

The other games do not move. Orbital's ten layers are groups and are already
right.

## What this does not decide

Named so nobody reads this note as covering them. Each is real 2D work, and each
is easier once depth and placement are derived rather than authored:

- 2D lighting, normal-mapped sprites, and coloured light on a tile volume;
- drop shadows that follow the surface a thing stands on;
- cutaway or fade when terrain stands between the camera and the player;
- slice views that hide levels above a chosen one;
- auto-tiling and terrain transitions, which belong to Tile System 2's terrain
  rules;
- pixel-perfect camera snapping and integer-scale presentation, which
  `docs/2d-model.md` already constrains to an orthographic camera.

Parallax is deliberately absent from that list. It is not further work; it is
what a perspective camera does once cells have real depth.

## Acceptance test

In the Sindri editor, starting from an empty scene:

1. create one isometric grid and paint ground across it;
2. place a prefab by clicking a cell, typing no transform;
3. raise the column beneath it and see the prop rise with the ground;
4. walk a character around it — in front, behind, and to either side — with
   stable, correct order and no layer authored anywhere in the scene;
5. switch the grid to orthogonal and see the ordering stay correct;
6. stack a block beside the prop and see the two sort against each other by
   position rather than by an integer;
7. delete every `layer` in the scene and see the render not change;
8. swap the orthographic camera for a perspective one and see parallax, with no
   content edited;
9. save, reopen, and reproduce the same scene and the same frame.

Passing requires no authored layer numbers, no typed transform for a
grid-placed prop, and no draw-order repair.

Step 7 is the one that matters. The others can be satisfied by a careful author;
only that one can be satisfied by the engine.
