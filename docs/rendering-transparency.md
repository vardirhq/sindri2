# Transparent rendering policy

Sindri treats transparency as an explicit render-order concern rather than relying on depth writes to produce accidental results.

## Blend modes

`SpriteRenderer::new` uses straight-alpha blending because `Texture2D::from_rgba8` accepts ordinary, non-premultiplied RGBA pixels. `SpriteRenderer::with_blend_mode` additionally supports:

- `Opaque` for replacement without transparency
- `Alpha` for straight-alpha textures
- `PremultipliedAlpha` when RGB channels have already been multiplied by alpha
- `Additive` for light, glow, and similar effects

A texture's encoding must match its selected blend mode. Mixing straight and premultiplied alpha produces visible fringes and incorrect color.

## Draw order

Opaque 3D passes render before transparent overlays. Transparent draws use `TransparentOrder` and are sorted in ascending key order:

1. Lower integer layer first.
2. Within a layer, greater distance from the camera first (back-to-front).
3. Exact ties use ascending submission index.

The distance is geometry rather than an authored number: how far in front of the camera the draw is, measured along the camera's forward axis rather than as a straight line to the eye, so two things side by side at the same depth sort as equally far away. `layer` is the one authored override, and it wins — a sprite in a higher layer draws in front of something nearer the camera.

The distance must be finite; NaN and infinity are rejected when the key is created. Measuring in view space rather than dividing a clip-space depth is what keeps that from being a real risk: a draw sitting exactly on the camera plane still produces a number. Submission indices make ordering deterministic across native and browser targets without treating insertion order as an undocumented side effect.

Transparent sprite passes do not write depth. Callers must sort their prepared draw list before encoding. `SpriteBatchRenderer` consumes instances in that final order and preserves it within one instanced draw for each texture/blend-mode group.

## Depth is read, never written

What a batch does about the depth the opaque stage wrote is `SpriteDepth`, and there are two answers:

- `Ignore` draws over the world whatever the depth buffer holds. A screen-space overlay is not in the world, so nothing in the world may occlude it.
- `Test` hides the batch behind opaque geometry nearer the camera, which is what being in the world means.

Neither writes. Blending is order-dependent, so a depth write would make the result depend on draw order twice: once through the sort above, and once through whichever sprite happened to reach the buffer first. The comparison is pipeline state, so the renderer holds one pipeline per answer and the frame command carries which one a batch wants.

Both attach the depth buffer read-only, so the frame must have cleared it — including a frame with no opaque geometry at all, which is what `encode_clear` is for.
