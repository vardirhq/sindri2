# Platformer

The first genre showcase: a small side-view platformer. Run and jump across a
painted level, pick up the coins and reach the flag.

Arrow keys or A/D run; Space, W or Up jumps. A tap is a hop and a held press a
full jump; a jump pressed just before landing, or just after running off a
ledge, still counts.

## What it is made of

There is no game code. The game is `assets/`:

- `platformer.scene.json`: the level, the hero, the coins, the flag and the HUD.
- `scripts/hero.decay`: running, jumping, coins, the flag and falling off.
- `scripts/hud.decay`: the coin count and the banner.
- `textures/`: pixel art drawn by `art/draw.py`, deterministic, so running it
  again changes nothing unless the drawing did.

It uses, with no Rust of its own:

- a **tilemap** painted in the editor, made solid by a **Tilemap Collider 2D**,
  with grass tufts left passable;
- the scene's own **gravity**, from a **Physics 2D World**;
- a **dynamic body** with a capsule collider and a **foot sensor** that says
  when the hero is standing, so walls and ceilings never count as floor;
- **sprite animation** clips for idle, run, jump and fall;
- a **camera** that follows the hero and stays inside the level;
- **screen text** filled from Decay.

## Playing it

Open `assets/platformer.scene.json` in the editor and press Play: the Scene view
opens in 2D, framed on the game's camera. It is also exported to the site at
`examples/platformer/`.

## Checked, not just run

`tests/a_run_reaches_the_flag.rs` stands the hero on the painted ground, has a
player hold right and jump at every gap and wall until it reaches the flag
without falling, and checks the camera follows. `src/lib.rs` is the harness
that plays it without a window, built from the same public pieces a host uses.
