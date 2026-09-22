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

**Implemented.** `sindri.environment` authors ambient colour/intensity and one directional light with direction, colour, and intensity. Textured world geometry and engine-owned voxel meshes use the same renderer-owned lighting model in editor viewports and browser Voxel Lab. Scenes without authored lighting retain the previous full-white ambient appearance.

The next slice adds shadows from this same directional light rather than inventing a second sun.

### 3. Shadows

Directional-light shadow casting and receiving for voxel terrain and general world geometry, with authored distance and quality controls and explicit WebGPU limits.

### 4. Ambient occlusion and contact depth

Start with the cheapest representation that gives voxel corners and contacts convincing depth. Compare mesh-time voxel AO with screen-space AO before choosing the permanent boundary; they solve overlapping but not identical problems.

### 5. Post-processing stack

Generalize the bloom path into an ordered world post stack. Initial effects are exposure, tone mapping, contrast, saturation/colour treatment, bloom, and vignette. World processing happens before crisp overlay/UI rendering.

### 6. Fog and atmosphere

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
