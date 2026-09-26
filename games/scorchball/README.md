# Scorchball

A couch football game for two to four players, each on their own pad. It is a
port of an earlier Unity project, rebuilt as a Sindri project with its
original pixel art, sounds and tuning.

It is the **local-multiplayer genre showcase**: where playing together on one
screen is proven. Gamepads, read by player slot, were added to Sindri for it.
Scorchball is now moving beyond faithful prototype parity into a small complete
game; its responsive Weave title screen is the first piece of that shell.

## Playing it

The title screen offers **Play vs Bot** and **Local Multiplayer**. A mouse/touch
can choose either button; A (South), Start, or Select on a pad enters the local
lobby directly. Play vs Bot remembers the choice until the first human joins,
then places one bot on the opposing side automatically.

- **Join:** press A (South) or Start on your pad. The first pad to press is
  player 1 (Blue, defending the left goal), the next is player 2 (Red,
  defending the right), then 3 (Blue) and 4 (Red).
- **Pick a player:** before readying, up and down on the d-pad switch between
  the two characters.
- **Ready:** Y (North). Press it again to take it back. When two players are
  ready, a countdown runs to "Kickoff!". A player who joins mid-match readies
  straight into play.
- **Play:** the left stick (or d-pad) walks. Hold the left bumper while moving
  to sprint; the player moves faster, the existing walk cycle runs at a
  deliberately ridiculous speed, and grass flecks kick up at full pace. Touch
  the ball to take it, even from someone else. Hold the right bumper to charge
  a shot and release to kick where the right stick points, or the way you face
  with the stick at rest. A quick tap is the ordinary kick. For a fraction of
  a second after a shot, keep using the right stick to bend its path; the
  influence fades quickly rather than steering the ball indefinitely. Hard
  shots kick up flecks and shake the camera, and a near miss can clatter off a
  goal post and rebound back into play. Repeated hard strikes also heat the
  ordinary ball up: its flecks become more frequent until play calms down and it
  cools again.
- **Leave:** unplug the pad.
- **Quick bot:** Select (Back/View) in the lobby still adds a bot to the side
  with fewer players, one bot a side. It readies itself.

A bot looks at the pitch about four times a second and acts on what it saw,
so it reacts a little late and its shots are a little off, as a person's are.
Each look it decides whether to attack with the ball, support a teammate,
defend goal-side, or chase a loose ball it can reach first. Bots now choose
from the same move set as a human rather than receiving AI-only advantages:
they sprint in short bursts when there is ground to cover, choose how long to
charge a shot from the situation, and sometimes add imperfect aftertouch. The
actual movement, kick force, aftertouch window, physics and possession rules
are shared with human players. They do not read the scorch meter or predict
post rebounds; those simply happen through normal play. Bots still do not pass,
pick up power-ups on purpose, or vary in difficulty.

Across a goal line between the posts is a goal; off the pitch anywhere else is
out, and the ball comes back to the centre spot. A goal keeps the existing
camera hit and now showers the players with a short celebration burst while the
GOAL callout is on screen.

## Power-ups

Every 5 to 15 seconds a signpost drops out of the sky. Its shadow marks where
it will land, growing as it falls, and it sticks in the grass with a thud.
Walk into it for what the sign shows; left alone it goes after ten seconds.

- **Enlarger:** you grow for 20 seconds, and reach the ball from further away.
- **Push Boost:** your kicks go much further for 20 seconds.
- **Wind:** for 20 seconds the pitch blows towards the other team's goal.
- **Fireball:** the ball burns for 20 seconds. It cannot be held, bounces off
  whoever touches it, and sets alight anyone on the other team, who then runs
  wild for five seconds. This remains distinct from ordinary shot heat, which
  is visual feedback and does not change possession or hurt players.

## What it is made of

There is no game code. The game is `assets/`:

- `scorchball.scene.json`: the pitch, goals, scoreboard, ball and on-screen text.
- `ui.weave`: the responsive title presentation, including hover/active states
  and a compact narrow-screen composition.
- `prefabs/title.prefab.json`: title structure and real UI button hit targets.
- `prefabs/`: players, the marker over their heads, power-ups, and the title UI.
- `scripts/title.decay`: title choices and transition into the lobby.
- `scripts/match.decay`: joining and leaving, ready-up, the countdown, the score,
  sprint/goal feedback, power-up spawns, bot-mode handoff and the wind.
- `scripts/player.decay`: walking, sprinting, skins, readying, charged kicking,
  short-lived aftertouch, growing and burning, and the player-like bot brain.
- `scripts/ball.decay`: possession, kicks, aftertouch, accumulating shot heat,
  hard-shot feedback, goal-post rebounds, fire, goals and out of bounds.
- `scripts/powerup.decay`: a signpost falling, landing, and what it gives.

It uses, with no Rust of its own: pads read by **player slot** (`Gamepad`),
**prefabs**, **Weave** responsive UI, pointer-aware **UI buttons**, **sprite
animation** for the two characters' four-way walks and silly-fast sprints,
**2D physics** so players bump into each other, **signals** between scripts,
**flecks** for sprinting, celebrations, fire, hard shots and accumulated ball
heat, **camera shake** for goals and impacts, **sounds**, and **screen text**.

## Changed from the Unity version

- A real title screen now leads into local or bot play instead of dropping
  straight into the prototype lobby.
- Players can sprint with the left bumper; it speeds both movement and the
  existing walk animation, deliberately exaggerates the animation speed, and
  kicks up flecks while at sprint pace. Bots use the same sprint multiplier in
  short, situation-driven bursts rather than getting a separate AI speed.
- Kicks can be charged by holding the right bumper. Players also get a brief
  fading aftertouch window after release; bots choose an imperfect correction
  through that same window. Strong shots get extra impact feedback, shots just
  outside the goal mouth rebound from the posts, and repeated hard strikes
  visibly heat the ordinary ball until it cools.
- Goals add a short player-centered celebration burst on top of the existing
  GOAL callout, camera hit and score update.
- Up to four players rather than two; odd slots play for Blue, even for Red.
- Possession, pickups and the Enlarger's reach are decided by distance rather
  than colliders, because a Sindri collider does not scale with its entity.
- Fireball can come up. The Unity spawner's `Random.Range(1, 4)` excluded it.
- Push Boost wears off after 20 seconds rather than lasting the whole match.
- An unready player stands 1.75 times normal size rather than 2.5, so the
  lobby still shows the pitch.
- No music. The Unity project's three tracks are other people's work under
  terms this repository has not checked, and the engine's MP3 support is an
  exception the dependency policy would rather not grow; add an OGG with
  `Audio.loop` in `match.decay` once one is cleared.
- The RPG Maker character sheet (`actor110.png`) and the unused test art were
  left out.

## Checked, not just run

`tests/a_match_is_played.rs` plays with pads it presses itself: two join on
their own sides, ready up and kick off; Blue takes the ball, dribbles and
scores; a sign cannot be taken until it lands; unplugging a pad takes its
player off; every power does what it says; and a Fireball sets an opponent
running wild until it burns out.
`tests/a_bot_plays.rs` adds a bot with Select, which readies itself and scores
on a player who stands still, and has two bots play until one scores. `src/lib.rs`
is the harness that plays it without a window, built from the same public
pieces a host uses.

## Art and sound

The sprites and the two sound effects are from the original Unity project.
`art/README.md` says how the textures were assembled from it.
