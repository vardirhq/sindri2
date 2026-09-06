# Graphics Lab

Graphics Lab is Sindri's zero-art visual proving ground. It is intentionally not a normal game: it exists to discover how far the engine's own primitives can be pushed before imported artwork enters the picture.

## Rules

- No textures or sprite artwork.
- Every visible object is built from `sindri.shape`, hierarchy, transforms, animation, blending, and Decay.
- Fonts may be used later for UI because text is a rendering primitive rather than artwork.
- New rendering capabilities should get an excessive showcase here before they graduate into Gather or Orbital Last Stand.
- Prefer several cheap layered primitives over one bespoke engine feature when the same look can be expressed with existing capabilities.

## Current scene

The first scene is a deliberately dense composition rather than a gallery of isolated test cases. It includes a layered reactor, a multi-shell boss, a shielded ship sculpture, orbitals, procedural rocks, sparks, an additive grid, hierarchy-driven counter-rotation, dashed strokes, and independent pulse animation. None of it references a texture.

The shared `showcase.decay` script drives rotation and scale pulses so the scene proves the visual result at runtime instead of merely arranging static shapes in the editor.

## Browser

After merge, the main Pages workflow exports this project through the same `sindri-export` path as Gather and Orbital Last Stand and serves it at:

`/sindri2/examples/graphics-lab/`

The dedicated Graphics Lab browser workflow also exports and smokes the project in Chromium and uploads a screenshot. That gives visual experiments a regression target instead of relying on somebody remembering to open the demo after every renderer change.
