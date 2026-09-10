# Screenshots

Every image here is produced by a capture binary in this repository rather than
by a screen grab, so it can be regenerated after a change instead of slowly
drifting away from what the engine actually draws.

Run all of these from the repository root.

## The editor

Needs a display; `xvfb-run` provides one on a machine without.

```
xvfb-run --auto-servernum --server-args="-screen 0 1600x1200x24" \
  ./scripts/capture-editor.sh site/screenshots/editor-cube-scene.png

xvfb-run --auto-servernum --server-args="-screen 0 1600x1200x24" \
  ./scripts/capture-editor.sh site/screenshots/editor-orbital-scene.png \
  games/orbital-baked/assets/orbital.scene.json
```

The second argument is the scene to open. `editor-orbital-scene.png` uses it to
photograph the editor holding a whole game — a seventy-nine entity hierarchy and
a real asset tree — rather than the two-cube demo, because that is what the
editor is for.

## The games

`orbital-capture` and `orbital-baked-capture` take an output path, a width, a
height, and which moment to photograph: `title`, `combat` (the cast, before the
first upgrade covers the field), `spectacle` (a deliberately excessive build,
for the effect vocabulary), or `playing` (twelve seconds in, with the upgrade
chooser up).

`orbital-baked-capture` also takes `boss-0` through `boss-11`, which open a boss
rush on that boss and photograph the fight seven seconds in. Bosses otherwise
arrive on a timer, so photographing the twelfth one meant playing for most of an
hour.

```
cargo run -p orbital-baked --bin orbital-baked-capture -- \
  site/screenshots/orbital-baked-combat.png 1440 900 combat
cargo run -p orbital-baked --bin orbital-baked-capture -- \
  site/screenshots/orbital-baked-spectacle.png 1440 900 spectacle
cargo run -p orbital-baked --bin orbital-baked-capture -- \
  site/screenshots/orbital-baked-title.png 1440 900 title
cargo run -p orbital-baked --bin orbital-baked-capture -- \
  site/screenshots/orbital-baked-upgrade.png 1440 900 playing
cargo run -p orbital-baked --bin orbital-baked-capture -- \
  site/screenshots/orbital-baked-phone.png 390 844 combat
cargo run -p orbital-baked --bin orbital-baked-capture -- \
  site/screenshots/orbital-baked-aegis.png 1440 900 boss-11

cargo run -p orbital-last-stand --bin orbital-capture -- \
  site/screenshots/orbital-last-stand-combat.png 1440 900 combat
cargo run -p orbital-last-stand --bin orbital-capture -- \
  site/screenshots/orbital-last-stand-phone.png 390 844 title

cargo run -p sindri-gather --bin gather-capture -- site/screenshots/gather.png
cargo run -p sindri-cube --bin capture -- site/screenshots/scene-frame-pipeline.png
```

The phone-shaped captures are 390x844 because that is the viewport
`it_fits_a_phone` holds the game to; they are the same game at the size the
layout is actually asserted against.

## What is deliberately not here

A screenshot of the browser build. The games render identically in a browser and
natively — that is the point of one renderer — so a browser capture would show
the same pixels with more machinery behind it. What a browser capture is good
for is proving that claim, and that belongs in a test rather than on a page.
