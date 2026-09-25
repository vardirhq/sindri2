# Scorchball

A couch football game for two to four players, each on their own pad. It is a
port of an earlier Unity project, rebuilt as a Sindri project with its
original pixel art, sounds and tuning.

It is the **local-multiplayer genre showcase**: where playing together on one
screen is proven. Gamepads, read by player slot, were added to Sindri for it.

## Playing it

- **Join:** press A (South) or Start on your pad. The first pad to press is
  player 1 (Blue, defending the left goal), the next is player 2 (Red,
  defending the right), then 3 (Blue) and 4 (Red).
- **Pick a player:** before readying, up and down on the d-pad switch between
  the two characters.
- **Ready:** Y (North). Press it again to take it back. When two players are
  ready, a countdown runs to "Kickoff!". A player who joins mid-match readies
  straight into play.
- **Play:** the left stick (or d-pad) walks. Touch the ball to take it, even
  from someone else. The right bumper kicks it where the right stick points,
  or the way you face with the stick at rest.
- **Leave:** unplug the pad.
- **Play against a bot:** Select (Back/View) in the lobby adds a bot to the
  side with fewer players, one bot a side. It readies itself.

A bot looks at the pitch about four times a second and acts on what it saw,
so it reacts a little late and its shots are a little off, as a person's
are. Each look it decides whether to attack with the ball (towards goal,
round whoever is in front, and a shot at the far side of the goal once close
or crowded), support a teammate who has it, defend (goal-side of the ball and
in to tackle once close, rather than chasing), or go for a loose ball it can
reach first. It does not yet pass, pick up power-ups on purpose, or vary in
difficulty.

Across a goal line between the posts is a goal; off the pitch anywhere else is
out, and the ball comes back to the centre spot.

## Power-ups

Every 5 to 15 seconds a signpost drops out of the sky. Its shadow marks where
it will land, growing as it falls, and it sticks in the grass with a thud.
Walk into it for what the sign shows; left alone it goes after ten seconds.

- **Enlarger:** you grow for 20 seconds, and reach the ball from further away.
- **Push Boost:** your kicks go much further for 20 seconds.
- **Wind:** for 20 seconds the pitch blows towards the other team's goal.
- **Fireball:** the ball burns for 20 seconds. It cannot be held, bounces off
  whoever touches it, and sets alight anyone on the other team, who then runs
  wild for five seconds.

## What it is made of

There is no game code. The game is `assets/`:

- `scorchball.scene.json`: the pitch, goals, scoreboard, ball and on-screen text.
- `prefabs/`: a player, the marker over their head, and a power-up's shadow and its signpost.
- `scripts/match.decay`: joining and leaving, ready-up, the countdown, the score,
  power-up spawns and the wind.
- `scripts/player.decay`: walking, skins, readying, kicking, growing and
  burning, and the bot's brain.
- `scripts/ball.decay`: possession, kicks, fire, goals and out of bounds.
- `scripts/powerup.decay`: a signpost falling, landing, and what it gives.

It uses, with no Rust of its own: pads read by **player slot** (`Gamepad`),
**prefabs** spawned when a pad joins, **sprite animation** for the two
characters' four-way walks, **2D physics** so players bump into each other,
**signals** between scripts, **flecks** for fire, **camera shake** on a goal,
**sounds**, and **screen text**.

## Changed from the Unity version

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
player off; every power does what it says;
and a Fireball sets an opponent running wild until it burns out.
`tests/a_bot_plays.rs` adds a bot with Select, which readies itself and scores
on a player who stands still, and has two bots play until one scores. `src/lib.rs`
is the harness that plays it without a window, built from the same public
pieces a host uses.

## Art and sound

The sprites and the two sound effects are from the original Unity project.
`art/README.md` says how the textures were assembled from it.
