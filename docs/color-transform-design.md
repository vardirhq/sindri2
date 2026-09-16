# Sprite color transform

Status: implementation track

Mujaffa Remaster exposed a limitation in `sindri.sprite`: `tint` can only multiply channels already present in the source texture. Recolouring artwork whose source hue has little energy in the requested output channel therefore produces weak or missing colours.

Sindri keeps `tint` as the simple/default authoring surface and adds an optional advanced per-sprite colour transform with independent RGBA multipliers and offsets:

`output = sampled * tint * multiply + offset`

The transform identity is `multiply = [1,1,1,1]`, `offset = [0,0,0,0]`, so existing scenes remain visually unchanged when the advanced transform is not authored.

## Authoring

The normal Sprite inspector continues to present texture, tint and layer. Advanced colour controls are collapsed by default and expose Multiply and Offset RGBA values plus Reset. This avoids turning an ordinary tint picker into an aircraft cockpit.

## Runtime and rendering

The values belong to `sindri.sprite`, travel with each `SpriteInstance`, and are evaluated in the sprite fragment shader. They remain per-instance so sprites with different transforms can batch when they use the same texture/layer.

## Decay

Decay exposes the advanced values beneath the sprite surface while preserving the existing `sprite.tint` API.

## Proof

For this feature track the explicit external proof is Mujaffa Remaster's LAKKERING RGB mixer rather than Orbital Last Stand or Gather. Mujaffa is the project that exposed the gap and can compare the result against the preserved Flash behaviour. The capability remains incomplete in Sindri's companion-game matrix until the normal repository proof requirement is satisfied later.
