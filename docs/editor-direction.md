# Editor direction

What the Sindri editor is for, what it should become, and the order the work
goes in. This is the document `docs/editor-architecture.md` sits under: that one
says how the editor is built, this one says what it is trying to be.

## The problem this starts from

The editor is complete enough to be judged and is not yet one anybody reaches
for. Its parts are individually right — a hierarchy, a generic inspector driven
by the schema registry, a project browser, gizmos, play/pause/step with snapshot
restore — and using it is still work rather than pleasure. That is not a
shortfall of features. It is what the editor is *shaped to do*.

It is shaped as a data-entry tool. The primary surface is a column of fields,
and the primary gesture is typing a number into one of them. That was the right
shape for an editor in 2012, when the alternative was editing the file by hand.
It is the wrong shape now, and the reason is not fashion: entering structured
values is the one job at which a person is slowest and least interested, and a
tool whose main offer is doing that job is a tool people route around.

What a person is uniquely good at — and what no amount of generated content
replaces — is **feeling** whether a game is right, and **judging** what to change
next. The editor should be built around those two, and should treat data entry
as the fallback it is.

## What the editor is for

1. **Closing the loop between a change and its consequence.** Play, feel that
   something is wrong, change one thing, feel it again. Every part of the editor
   should be measured by what it does to the length of that loop. This is the
   first principle and the others are downstream of it.
2. **Making the scene legible.** What is in this world, what is it doing, and
   why did it just do that. A hierarchy of five hundred rows is a file listing,
   not an answer.
3. **Manipulating the thing rather than its numbers.** A radius is dragged in
   the viewport. A curve is scrubbed. A speed is changed while the game runs and
   the change is visible immediately.
4. **Saying plainly when something is wrong**, once, with the thing it is about.

## What follows from that

### Live editing is the centre, not a mode

Today Stop restores the world as it was when Play was pressed, so tuning means
play, stop, guess, play again — and the feel you were tuning against is gone by
the time you are editing. That is the single largest cost in the loop above.

The capability is: **edit while playing, and decide what to keep when the run
ends.** A change made during a run is a command like any other; what it needs is
for Stop to ask which of them survive the snapshot restore instead of assuming
none do. That works because editor mutations already go through checked commands
rather than direct world writes.

### Direct manipulation before fields

The inspector should be where a value is read and corrected, not where it is
found. Anything with a shape belongs in the viewport: collider bounds, camera
frustums, spawn radii, the reach of an effect. Sindri has transform gizmos and
nothing else, and each missing one sends an author back to typing numbers and
pressing Play to see whether they were right.

### Time is a dimension the editor can move in

The engine steps at a fixed rate and the simulation is deterministic. That makes
recording a run and scrubbing it a genuinely available capability rather than an
ambition, and it answers the question authored fields cannot: *why did that
feel unfair?* The History panel is command history; this is world history, and
it is where the editor has the most room to be better than its baselines rather
than level with them.

### Errors are the editor's failure, not the project's

Opening Orbital Last Stand reports dozens of errors, most of them the same
error. A list that long is not information. Group by cause, count the repeats,
name the entity each is about, and where the editor knows the fix, offer it.

### The arrangement belongs to the person using it

Implemented. See below; it is the first slice, because an editor whose panels
cannot be moved is one that has decided what you are working on.

## Order of work

Dependency order, not priority order — each item is cheaper once the ones above
it are done.

1. ~~**A workspace the user arranges.**~~ Done: `editor/src/dock/`. Every panel
   is a tab, every tab is draggable into any slot, and slots resize freely.
2. **Error grouping in the console.** Small, self-contained, and it stops the
   editor lying about the size of a problem.
3. **Gizmos for the shapes that have none** — colliders, cameras, effect radii.
   Each one removes a round trip through Play.
4. **Edit while playing, with a decision at Stop.** The largest single
   improvement to the loop, and the one that needs the most care around the
   snapshot boundary.
5. **Record and scrub a run.** Depends on 4 having settled what a run's
   authority over the world is.

## What is deliberately not here

**Sindri does not integrate AI in the editor, and this document is not a plan to
add one.** All generation happens outside the editor today, and that is the
decision in force: it keeps the editor free of API keys, network calls, per-use
cost, and a dependency on a service being up.

The intent is to revisit this later, so it is worth saying what is already in
place should that happen, and what is not being built now.

What the architecture already gives, at no cost and for its own reasons:

- **Every mutation is a checked, named, reversible command** rather than a world
  write. Anything proposing a change — a person, a script, a tool, eventually a
  model — goes through the same seam, and anything that comes through it can be
  reviewed and undone.
- **Panels are data.** A new panel is one `dock::Panel` variant and one arm of
  the match in `native/workspace.rs`. Adding a surface costs no rearrangement of
  the editor around it.
- **The component schema registry describes components in machine-readable form**
  and `docs/generated/` is regenerated from it, so what the editor knows about a
  project is already written down rather than implicit in UI code.

What is **not** being built, and should not be added speculatively: any network
client, credential storage, provider abstraction, prompt surface, or placeholder
panel. A seam that is already load-bearing for its own reasons costs nothing to
keep clean. Scaffolding for a feature nobody is building is a maintenance
burden and a set of decisions made too early.
