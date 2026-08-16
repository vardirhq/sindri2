# The windowed host

A host owns the window, the surface, the clock, and input. A game owns what to
build, what to do with a frame of time, and how to draw. `sindri-desktop` is
where that line is drawn for `winit`, on a desktop and in a browser alike.

## What moved, and why it had to

Both proof examples used to open their own window and run their own event loop.
They arrived at the same design independently, down to the same four startup
states, because the problem has one shape:

```text
resumed          -> create the window, start the device request
device ready     -> build the application, size it to the surface
redraw requested -> advance time, acquire, draw, present, ask for another
resized          -> reconfigure the surface, rebuild what is sized to it
```

Two copies of that is two things to keep in agreement, and the parts most likely
to drift are the ones nothing checks: whether a frame's delta is measured from a
clock or from the previous frame's timestamp, whether a resize rebuilds the depth
target, whether a skipped frame still asks for the next one. The cube example
measured its own delta and capped it by hand; the triangle did not measure time
at all.

## The application's half

```rust
impl DesktopApp for MyGame {
    type Error = MyError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> { /* build renderers */ }
    fn update(&mut self, input: &InputState, delta: Duration) -> Result<Flow, Self::Error> { /* write the world */ }
    fn resize(&mut self, context: &AppContext<'_>) -> Result<(), Self::Error> { /* resize depth targets */ }
    fn render(&mut self, context: &AppContext<'_>, view: &wgpu::TextureView) -> Result<(), Self::Error> { /* encode */ }
}

sindri_desktop::run::<MyGame>(WindowConfig::new("My Game"))?;
```

`create` is deliberately not asynchronous. Requesting a device is, and the host
has already awaited it, so an application that loads nothing at startup does not
have to be async to say so.

Every hook returns a `Result`. A failure stops the host and comes back out of
`run` rather than being logged and drawn over, which is what the examples used to
do with `expect` in the middle of a frame.

## Time

The host reads a clock and derives a delta, rather than letting each application
subtract timestamps. `FrameTimer` cannot produce a negative frame, and forgetting
the previous reading after a pause is one call rather than a rule to remember.

`sindri-platform`'s `SystemClock` is native-only, because `std::time::Instant`
has no meaning in a browser. The host therefore reads a `WindowClock`, which is
`Instant` natively and `performance.now()` on `wasm32` through `web-time` — one
clock, no conditional around something as basic as what time it is.

That delta then goes to the engine rather than to gameplay directly. `EngineHost`
accumulates it into fixed simulation steps, so `Game::fixed_update` runs a whole
number of times at an exact step regardless of how long the frame took, and a
stalled frame is capped instead of being integrated in one lurch. The cube
example used to integrate raw frame deltas with a hand-written cap, which meant
a second of held input turned it further on a slow machine than a fast one; a
test now holds it to the same rotation at 15, 60, and 144 frames per second.

Input goes the same way. The host translates `winit` events and hands them over;
`EngineHost` accumulates them and clears their per-frame edges as part of
advancing. Two `InputState`s would be two answers to whether a key is down, so
there is one, and it belongs to whoever runs the frame.

## Why the browser uses the "desktop" crate

`winit` presents a canvas through the same event loop it presents a window
through. A separate browser host would be a copy of this one with six lines
changed — the canvas attachment, how a future is spawned, and where time comes
from — which is the duplication this crate exists to remove. So the target
conditionals live here, and applications are written once.

What remains conditional in an application is its logger: `env_logger` natively,
`console_log` and a panic hook in a browser. That is a genuine application
choice, not a host detail, so the host does not take it over.

## What the host does not do

It does not know what a scene, a world, or a component is; it hands an
application a device and a texture view. It does not choose a surface format or
decide what a failed acquisition means — that is the
[presentation surface policy](rendering-surface.md). It does not own the game
loop's simulation semantics, which belong to `sindri-platform`.
