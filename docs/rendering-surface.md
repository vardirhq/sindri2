# Presentation surfaces

Asking a surface for its next texture has seven outcomes and only one of them is a frame. The other
six each want a different response, and none of them is rare: a resize produces two of them, a
minimised window produces a third every frame it stays hidden, and a display change or a suspended
application produces the fourth.

Nothing about getting this wrong is loud. Reconfiguring on an occluded frame rebuilds the swapchain
in a loop behind a minimised window. Treating a lost surface as a skipped frame renders nothing,
forever, while the event loop keeps running normally. Both look like a hang rather than a bug with a
location.

## The policy

`SurfaceStatus::action` maps each outcome to one response.

| Outcome | Response | Why |
| --- | --- | --- |
| `Ready` | Present | The texture matches the surface. |
| `Suboptimal` | Reconfigure | Presentable, but stretched — mid-resize the configuration is behind the window. |
| `Outdated` | Reconfigure | The surface changed and its configuration no longer describes it. |
| `Timeout` | Skip | The driver was slow. Nothing about the surface is wrong. |
| `Occluded` | Skip | The window is minimised or covered, and arrives every frame until it is not. |
| `Validation` | Skip | Already reported through the device's error scope. |
| `Lost` | Recreate | The surface object is gone and must be built again. |

Reconfiguring costs the frame it replaces, which is why only the two stale outcomes pay it. Skipping
is for the three cases that resolve themselves, where touching the surface would be work performed
once per frame for as long as the condition lasts. Recreating throws away GPU state and is reserved
for the one outcome where the surface no longer exists.

A validation error skips rather than panics. It has already been reported wherever the device's
error scope sends it, and a panic thrown from the presentation path would replace that report with a
backtrace that names the swapchain instead of the mistake.

## Why it lives in one place

Both proof examples carried their own copy of this decision, identical down to the comments, because
each was written by working out the same seven cases again. A third host would have written a third
copy, and the first one to differ would have differed silently — the same failure mode as the
[colour target](rendering-color.md), where two targets disagreeing about a format cost weeks.

`WindowSurface` owns the surface, its `SurfaceProfile`, and this policy. Hosts call `acquire` and
receive either a texture to draw into or `None`, having already been reconfigured or rebuilt if that
is what the outcome required. An error means the surface was lost and could not be built again,
which retrying does not fix.

```rust
let Some(frame) = surface.acquire(&gpu.device)? else {
    return; // Skipped. Ask for another frame.
};
```

## What the host still owns

`WindowSurface` never learns what a window is. Rebuilding a lost surface needs whatever the surface
was attached to, so the host supplies a closure that builds one rather than the window itself, which
is what lets the same type serve a `winit` window and a browser canvas.

Requesting the next redraw, calling `pre_present_notify`, and presenting the finished texture stay
with the host, because each of those is the host's own vocabulary.

## Testing a decision that needs a GPU

`wgpu::CurrentSurfaceTexture` carries a real swapchain texture in its two successful variants, so
five of the seven outcomes can be constructed in a test and two cannot — including `Suboptimal`,
which is the one whose handling is least obvious.

So `SurfaceStatus` mirrors the outcome without the texture. Classifying is a total match that a new
`wgpu` variant breaks at compile time; the policy is a function from that mirror to a response, and
every case of it is checked. This is the same split as `sindri-desktop`, where `winit`'s key events
cannot be built off a real window and every decision they feed therefore lives in a function that
can be called directly.
