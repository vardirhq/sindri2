# World presentation plan

Sindri's voxel world is becoming structurally capable faster than it is becoming
visually convincing. This track closes that gap as a general engine capability:
a scene should be able to establish a complete visual atmosphere through authored
environment settings, with the same result in the editor, native builds, and
WebGPU builds.

**Voxel Lab is the acceptance lab for this track.** Causeway remains the product
showcase, but presentation work is developed and inspected first in Voxel Lab so
lighting, post processing, fog, sky, and later effects can be judged against a
controlled voxel scene without coupling renderer work to game design.

## Rules

- Environment is authored scene state, not a collection of host-only renderer switches.
- `sindri-render` owns GPU implementation; `sindri-scene` is the seam from authored state to rendering.
- Voxel Lab must use the same scene/rendering path the editor and games use. Do not create a parallel presentation renderer for the lab.
- Native and WebGPU remain one feature contract.
- Effects are opt-in and should have a cheap/off path.
- Gameplay decisions belong in Decay. Rendering implementation does not.
- Each slice updates parity/capability documentation and carries visible Voxel Lab proof before it is called complete.

## Ordered slices

### 1. Environment foundation and bloom

Introduce an authored environment component and connect the existing bloom renderer to it. Initial environment state covers background/clear colour, ambient colour/intensity as forward-compatible authored data, and bloom settings. Voxel Lab is the first visual proof.

Exit criteria:

- a scene can author the environment rather than a host choosing bloom settings;
- bloom can be enabled/disabled and tune threshold, knee, intensity, and passes;
- invalid authored values are rejected or normalized at the scene boundary;
- the generic inspector exposes the component;
- Voxel Lab uses the authored component through the shared render path;
- resizing keeps the bloom target correct;
- native/offscreen and browser paths remain valid;
- deterministic render coverage demonstrates bloom changes pixels without post-processing UI/overlay content.

### 2. Directional and ambient lighting

**Implemented.** `sindri.environment` authors ambient colour/intensity. The sun is a light entity: `sindri.light` with kind `directional`, colour and intensity, aimed by its entity's rotation along local -Z (the axis a camera looks along). The first active one lights the world; the Scene view draws each light as a sun with an arrow the way it shines, and a selected one with a trail along its light. Format 10 migrated the old `environment.directional` vector into such an entity, and new scenes start with one. Textured world geometry and engine-owned voxel meshes use the same renderer-owned lighting model in editor viewports and browser Voxel Lab. Scenes without authored lighting retain the previous full-white ambient appearance.

The next slice adds shadows from this same directional light rather than inventing a second sun.

### 3. Shadows

**Implemented.** The authored directional light now owns one WebGPU-friendly shadow map shared by textured world geometry and cached voxel meshes. `sindri.environment.shadows` controls whether shadows are enabled, the world-space coverage distance, map resolution (256–2048), and depth bias. Editor viewports and browser Voxel Lab use the same path, and older environments default to shadows off.

The first implementation deliberately uses one directional map rather than cascades: predictable cost and WebGPU portability matter more than hiding every long-distance alias. Cascaded shadows remain a later quality extension if a real game proves the need.

### 4. Ambient occlusion and contact depth

**Implemented.** Voxel block meshing samples the three neighbouring cells around each exposed face corner and carries a compact 0–3 visibility value into textured world vertices. The renderer interpolates that value across the face and applies authored contact-depth strength without another screen-space pass. Face diagonals flip when needed to keep the AO gradient from producing the familiar checkerboard seam.

This intentionally chooses mesh-time voxel AO over SSAO for the first permanent boundary: it is deterministic, cheap, section-aware, works identically in native and WebGPU builds, and targets the block contacts that need the depth most. General screen-space AO remains a later extension if non-voxel scenes prove they need it.

### 5. Post-processing stack

**Implemented.** The former bloom-only offscreen path is now the ordered world post stack. `sindri.environment.post_process` authors exposure, contrast, saturation, tone mapping (`none`, `reinhard`, or `aces`), and vignette; the existing bloom controls run inside the same chain. Grading is applied before bloom thresholding, vignette is applied after the bloom composite, and overlay/UI rendering remains outside the stack so it stays crisp.

The cheap path is still real: a neutral post-process plus disabled bloom draws directly to the frame target. Older environments deserialize to those neutral values. Voxel Lab deliberately enables ACES, a small exposure/contrast/saturation lift, bloom, and a restrained vignette as the visual proof.

### 6. Fog and atmosphere — **Implemented**

Distance fog first, then height/exponential fog. Fog integrates with the environment background and gives large voxel worlds an intentional atmospheric horizon rather than exposing residency distance.

### 7. Sky and sun

Procedural horizon/zenith sky, visible sun, and one authored sun direction shared with directional lighting and shadows. Textured/cubemap skies, stars, moon, and clouds are later extensions.

### 8. Environment profiles

Reusable environment project assets such as clear day, sunset, foggy morning, cave, and night. Scenes reference a profile and may override it. Add focused editor UX once the data model has been proven through the generic inspector.

### 9. Environment volumes

Spatial environment overrides with blending. This is the foundation for caves, interiors, underwater spaces, and biome-local atmosphere without replacing the global environment.

### 10. Local lights and emissive materials

Point lights, spot lights, range, colour, intensity, optional shadows, and emissive material contribution. Decay may request gameplay changes through typed engine intent; it does not implement lighting.

### 11. Wind and environmental motion

A shared world wind model with direction, strength, gust strength, and gust frequency. Foliage, particles, smoke, weather, and later clouds opt into it.

### 12. Weather

Rain, snow, dust/ash, and storms compose existing environment systems rather than forming a separate visual universe. Weather may drive particles, fog, sky, lighting, wind, surface response, and audio hooks.

### 13. Water

Begin with a stylized transparent surface, animated distortion, depth colour, shoreline fade, underwater fog, and environment transition. Reflection, refraction, foam, caustics, and richer waves follow only when the basic path is useful.

### 14. Advanced cinematic effects

Optional depth of field, radial/motion blur, screen distortion, heat haze, film grain, chromatic aberration, LUT grading, lens effects, and volumetric light. These come after the world fundamentals because expensive blur cannot rescue flat lighting.

## Visual acceptance

The target is not photorealism. A Sindri voxel scene should gain readable form and atmosphere from layers that agree with one another:

voxel geometry -> lighting -> shadows/contact depth -> atmosphere/sky -> post-processing -> crisp UI.

Every slice should make that progression visibly inspectable in Voxel Lab. The lab is allowed to become attractive; being a test does not require looking like one.
