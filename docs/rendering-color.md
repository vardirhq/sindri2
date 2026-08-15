# Colour handling

Colour in Sindri is a round trip, and it only closes if both halves agree.

1. Source pixels are authored in sRGB. `Texture2D` uploads them in an sRGB format, so sampling
   decodes them to linear.
2. Shaders work in linear.
3. A colour target must re-encode on write, which only an sRGB format does.

Skip step 3 and nothing fails. The frame still renders, every pipeline still validates, and every
headless test still passes — the image is simply the wrong colour. Linear values get stored as
though they were sRGB, which crushes everything dark and saturated:

| Authored | Stored without the encode |
| --- | --- |
| orange `(240, 114, 43)` | `(224, 44, 5)` — reads as pure red |
| navy `(18, 34, 55)` | `(1, 3, 9)` — reads as black |

This is exactly what happened when the editor's viewport defined its own target format and picked a
linear one. The offscreen capture of the same scene was correct, so the two disagreed for weeks
without anything reporting a problem.

## One format, three targets

Offscreen and in-editor targets share [`sindri_render::COLOR_TARGET_FORMAT`] rather than each
choosing. There is nothing left to drift.

Swapchains cannot use a constant, because their format is negotiated with the surface. Rather than
accepting whatever comes back, `SurfaceProfile` requires an sRGB format and fails with
`GpuError::NoSrgbSurfaceFormat` if the surface offers none. A host that cannot encode colour is a
configuration to report, not one to render badly.

## The other end: who samples the target

Encoding on write is only half of it. Something reads that texture afterwards,
and it has its own assumption about what the stored bytes mean.

The editor renders its viewport into an sRGB target — correct — and then hands it
to egui to draw. egui's shader states its expectation plainly:

```wgsl
// We expect "normal" textures that are NOT sRGB-aware.
let tex_gamma = sample_texture(in);
```

It treats whatever it samples as already gamma-encoded, and decodes it before
writing to an sRGB surface. Give it an sRGB *view* and the hardware decodes on
read as well: two decodes against one encode, which is the same missing-encode
arithmetic as above, arriving from the opposite direction. Authored orange
`(240, 114, 43)` displayed as `(221, 43, 6)` — exactly `linear_from_srgb` of
itself.

So the viewport texture carries two views of the same bytes: the sRGB view the
scene renders into, and the linear view egui samples. Neither half has to know
what the other assumed, and no conversion happens twice.

The general rule: **a colour target's format is a claim about its bytes, and
every reader has to agree with it.** One shared constant fixes the writers. The
readers have to be checked separately, because a sampler that disagrees produces
exactly the same silent failure.

## Why a pixel check exists

Every automated check the project had would pass a colour-space mistake: it compiles, it lints, it
renders, it does not crash. Only looking at the image catches it.

So the headless capture looks. After writing its PNG, `sindri-cube`'s `capture` binary verifies that
the colours the scene authored actually appear in the rendered frame, within a tolerance wide enough
for texture filtering and a software rasteriser but far tighter than a colour-space error, which
moves channels by 40 to 70. A failure reports the image's most common colours, so the diagnosis is
in the CI log rather than requiring the artifact to be downloaded.

The PNG is written before the check runs, so a frame that fails verification is still uploaded and
can be looked at.

That check covered one image, and the bug above lived in the other. CI has always captured a
screenshot of the editor window, but nothing looked at it, so the viewport could disagree with the
offscreen capture of the same scene for a release without failing anything.

Both are now held to the same expectation. `AUTHORED_COLORS` and the tolerance live in one place in
`sindri-cube`, and the `verify` binary reads back a captured PNG — decoded through the engine's own
`TextureAssetDecoder`, so the check does not introduce a second opinion about what an image contains
— and applies them. A capture that nobody examines is an artifact, not a test.
