# Attribution

This tool is adapted from the **IsoGame Sprite Factory**:

- Project: <https://github.com/MadsenDev/isogame>
- Tool: <https://github.com/MadsenDev/isogame/tree/main/tools/sprite-factory>
- Licence: MIT, as declared by that project's `package.json`. The repository
  carries no separate `LICENSE` file at the time this was ported, so that
  declaration is the whole of the licence statement available.

The port is described file by file in `README.md`. In short: the isometric
camera, the shade-ramp palette, the banded material, the post-processing chain
(downsample, alpha threshold, palette snap, inner outline) and the compass
direction scheme are IsoGame's design, carried over with their reasoning intact.
The rasteriser, the recipe document, the anchor-aligned frame layout and the
Sindri sheet output are new here.

Nothing IsoGame-specific was carried across: its furniture categories, its
sit/lay/use interaction spots, its stackability and collision model, and its
character layer system all describe that game, and would be another game's
vocabulary in a Sindri asset format.
