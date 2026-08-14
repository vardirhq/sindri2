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
2. Within a layer, greater camera depth first (back-to-front).
3. Exact ties use ascending submission index.

Depth must be finite; NaN and infinity are rejected when the key is created. Submission indices make ordering deterministic across native and browser targets without treating insertion order as an undocumented side effect.

Transparent sprite passes do not write depth. Callers must sort their prepared draw list before encoding. Sprite batching will preserve this ordering contract when it is added.
