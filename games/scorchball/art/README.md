# Where the textures came from

Everything in `assets/textures/` was assembled once from the Unity project's
`Assets/Sprites/`, which is not in this repository:

- `players.png`: `Ginger/ginger_full_row.png` over `Yellow/Yellow_full_row.png`.
  Each row is four idle frames (down-right, down-left, up-left, up-right) and
  then twelve walk frames each for up-right, down-right, up-left and down-left.
- `pitch.png`: the plain grass of `field.png`, tiled to 640 by 288, with
  `field_lines.png` on top.
- `signs.png` is the five `powerup_*.png` signposts side by side;
  `shadows.png` is the six frames of `shadow_growing.png` side by side; `zones.png` is the five `area_*.png` rings; `arrows.png`
  is the four `arrow_p*.png` markers; `digits.png` is `numbers.png`.
- `ball.png`, `goal_left.png`, `goal_right.png` and `scoreboard.png` are copied
  as they were.
- `fleck.png` is new: a four-pixel dot for fire.

The Unity project drew at 100 pixels to a unit, with players at twice their
pixel size. Scorchball's scene is five times larger, so a player's 32-pixel
frame is 3.2 units, the pitch 32 by 14.4, and the goals 1.6 by 4.8.
