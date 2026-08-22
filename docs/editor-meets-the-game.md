# Taking the editor to the game

`ROADMAP.md` schedules this under the companion game: open Gather in the editor
and record what authoring it there is actually like. This is that record. It is
a session note rather than a contract — it describes one afternoon in August
2026, against the editor at that commit, and it will age.

It exists because the alternative was guessing. The engine's stated priority is
that a feature is not complete if only the runtime understands it, and the whole
argument for a companion game is that using a thing finds what reading about it
does not. Gather was authored entirely by hand in JSON. Nobody had opened it in
the tool that is supposed to make that unnecessary.

## What worked

**It opens and it is the same picture.** `sindri-editor game/assets/gather.scene.json`
loads all 68 entities, resolves all five textures and all four scripts, and
draws the game in both viewports — the scene view through the editor camera, the
game view through the authored one. Nothing needed to be told about the game;
the extractor derived both frames from the world, which is the architecture's
central bet and it paid.

**Play runs the game's actual rules.** Pressing Play advances the scripts: the
orbs visibly bob, because `orb.decay` says they should. This is the parity claim
in its strongest form — the editor is running the same Decay sources the
standalone game runs, through the same `Scripts` host, against the same world.
No separate editor path, no stub.

**The project browser is right.** It reads the scene's own directory, lists the
four scripts as Script and the five textures as Texture, and got the folder
structure from the disk rather than from the scene.

## What it found

**A cold open reported twelve errors against a working game.** The status bar
read "12 Errors, 0 Warnings" — one per scripted entity. None of them were real.
The editor compiles every frame, asset loading is asynchronous by design, and in
the window between the scene landing and its scripts arriving every scripted
entity is briefly missing its source. `Scripts::compile` reported that as
`MissingSource`, the console recorded it, and the console keeps what it is told,
so the count stayed up long after the scripts had compiled and were running.

It reproduced on the first open and not the second, which is what made it worth
chasing rather than dismissing: it is a race, so it appears on a cold cache —
exactly when somebody opens a project for the first time. Fixed, with the test
that catches it in `editor/src/scripts.rs`. A source that will never arrive
still reports, so the fix did not trade a phantom error for a silent one.

**The hierarchy stops working somewhere below 68 entities.** Forty-nine of
Gather's entities are floor tiles named `Floor 0,0` through `Floor 6,6`, and
they are sorted into the middle of the list. Every entity a person would
actually want to select — the player, the five orbs, the banner — is below all
of them. There is a search box and it works, but needing to search for the
player in a scene with one player is the tool failing rather than the author
being helped.

The tilemap removes this particular 49. It will not remove the next one: any
scene with a repeated element hits the same wall, and the honest reading is that
the hierarchy needs grouping, not that scenes should have fewer entities.

**There was no way to select anything in the viewport.** That was much worse at
68 entities than at 8: the hierarchy was the only way in and it was full of
floor. It is fixed for world sprites, filled tilemap cells, and meshes; clicking
the Scene view now drives the same selection as the hierarchy and inspector.
Screen-space overlays remain hierarchy-selected.

**The screenshot script could not photograph anything but the fixture.**
`scripts/capture-editor.sh` hard-coded `cargo run --package sindri-editor` with
no arguments, so the one tool for looking at the editor could not be pointed at
the scene in question. It takes a scene path now.

## What was not learned

The player did not move under synthetic key events, and that is not evidence of
anything: `xdotool key` into an unfocused window under a bare Xvfb with no
window manager is a poor imitation of a keypress, and the orbs bobbing proves
the script host was running. Whether driving the player from inside the editor
feels right is a question for a person at a real keyboard, and it is still open.

Nothing here says whether the editor is *pleasant*. It says the editor can open
the game, draw it, and run it, and that finding an entity in it is miserable.
