# Sindri Next

## Project overview

Sindri Next is a **general-purpose 2D and 3D game engine written primarily in Rust**, designed to run both natively and in modern web browsers.

The engine should use Rust for its runtime, rendering, world model, physics integration, asset systems, and performance-sensitive functionality while providing a **first-class TypeScript/JavaScript API for browser game developers**.

The long-term developer experience should support both of these equally valid workflows:

```rust
use sindri::prelude::*;

fn main() {
    Engine::new()
        .run(MyGame::new());
}
```

and:

```ts
import { Engine, Scene, Sprite } from "@sindri/engine";

const engine = await Engine.create({
  canvas: "#game",
});

engine.start();
```

The web API should not be treated as a thin afterthought around raw WebAssembly exports.

It should feel like a deliberately designed TypeScript game engine.

Sindri should support:

- native desktop games
- browser games
- GPU-accelerated 2D
- realtime 3D
- isometric games
- tile/grid-based games
- physics-based games
- Rust-authored gameplay
- TypeScript/JavaScript-authored gameplay on the web
- optional scripting systems
- visual scene editing through the Sindri Editor
- shared project, scene, asset, and serialization formats across targets

The project should **not** become an attempt to clone Unity, Unreal, Godot, Three.js, Babylon.js, or Phaser feature-for-feature.

Its goal should instead be:

> A coherent, lightweight Rust-powered game engine with excellent native and web support, a first-class TypeScript API, strong 2D/3D fundamentals, and an editor that understands the same engine model used at runtime.

---

# Core principles

## One engine, multiple targets

Do not create separate engines for:

- native
- web
- 2D
- 3D
- isometric

These should be capabilities of the same engine architecture.

Conceptually:

```text
                    Sindri

        ┌─────────────┼─────────────┐
        │             │             │
       2D            3D         Isometric
        │             │             │
        └─────────────┼─────────────┘
                      │
                 Engine Core
                      │
                 GPU Renderer
                      │
                    wgpu
              ┌───────┴────────┐
              │                │
           Native            Web
                           WASM/WebGPU
```

The engine should expose consistent scene, entity, transform, asset, input, timing, and rendering concepts regardless of target.

---

# Why Rust

Rust should remain the primary engine implementation language.

Reasons include:

- Sindri already has substantial Rust engine work.
- `wgpu` gives Sindri a modern GPU abstraction.
- Rust can target native platforms and WebAssembly.
- The renderer and simulation can remain largely shared across desktop and browser targets.
- Performance-sensitive systems remain outside JavaScript.
- Rust provides strong internal correctness guarantees for increasingly complex engine code.
- The existing Sindri architecture and concepts can be reused rather than discarded.

JavaScript and TypeScript should be supported as **game-authoring languages for web users**, not as replacements for the Rust engine implementation.

---

# Rendering architecture

## wgpu as the rendering foundation

Use `wgpu` as Sindri's primary GPU abstraction.

The rendering architecture should be designed around modern GPU concepts without exposing unnecessary low-level complexity to normal game developers.

The renderer should eventually support:

- sprites
- meshes
- textures
- materials
- cameras
- render targets
- depth buffers
- lighting
- text
- particles
- tilemaps
- post-processing
- offscreen rendering

Do not attempt to implement all of this immediately.

The first requirement is a clean rendering foundation capable of supporting both 2D and 3D.

Conceptually:

```text
wgpu
  │
  ▼
Sindri GPU Layer
  │
  ├── textures
  ├── buffers
  ├── shaders
  ├── pipelines
  ├── render targets
  └── render passes
       │
       ▼
Renderer
  │
  ├── SpriteRenderer
  ├── MeshRenderer
  ├── TextRenderer
  ├── TilemapRenderer
  └── ParticleRenderer
```

---

# 2D and 3D should share infrastructure

Do not implement the engine as a 2D engine with 3D bolted onto it later.

Instead, share appropriate lower-level concepts.

Examples include:

- scenes
- entities
- assets
- textures
- materials
- cameras
- input
- timing
- events
- rendering passes
- visibility
- layers
- resource handles

At the same time, do not force 2D into unnecessarily complicated 3D abstractions merely for architectural purity.

For example, separate transforms may be reasonable:

```rust
struct Transform2D {
    position: Vec2,
    rotation: f32,
    scale: Vec2,
}

struct Transform3D {
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
}
```

The architecture should favor clarity over artificial unification.

---

# Suggested workspace structure

The exact naming may change, but the architecture should trend toward something like:

```text
crates/
  sindri-core/
  sindri-gpu/
  sindri-render/
  sindri-2d/
  sindri-3d/
  sindri-iso/
  sindri-assets/
  sindri-input/
  sindri-physics/
  sindri-platform/
  sindri-platform-desktop/
  sindri-platform-web/
  sindri-script/
  sindri-editor-protocol/

packages/
  sindri/
  create-sindri/

editor/

examples/

docs/
```

Not every crate should be created immediately.

Split crates when there is a real architectural boundary, not simply to produce a visually impressive workspace.

---

# sindri-core

`sindri-core` should contain concepts with no dependency on a specific renderer, operating system, browser, editor, or game genre.

Potential responsibilities:

- engine lifecycle
- application lifecycle
- world
- scenes
- entities
- component storage or lightweight ECS
- time
- frame state
- fixed updates
- event system
- commands
- resource identifiers
- asset identifiers
- serialization-friendly core types

The core should ideally not care whether a game is being rendered in 2D, 3D, on desktop, or in a browser.

---

# World and entity model

Continue the lightweight ECS-style direction rather than introducing a highly complex ECS architecture prematurely.

A reasonable conceptual model is:

```text
World
 ├── Entity
 │    ├── Transform
 │    ├── Sprite / Mesh
 │    ├── Physics
 │    ├── Script
 │    └── custom components
 │
 ├── Resources
 └── Systems
```

Important properties:

- stable entity identifiers
- safe creation/destruction
- component querying
- serialization
- editor inspection
- runtime/editor parity
- support for both Rust and web APIs

Avoid introducing an advanced archetype ECS or scheduler unless profiling demonstrates a need.

---

# Scene model

Scenes should be first-class serialized engine concepts.

A scene should contain:

- entities
- components
- hierarchy
- transforms
- asset references
- cameras
- lights
- environment configuration
- optional scene metadata

Scenes should not contain editor-only UI state unless it belongs in a clearly separated editor metadata section.

A project should be able to load the same scene from:

- native runtime
- browser runtime
- editor viewport
- automated tests

---

# Platform abstraction

Separate platform-specific behavior from engine behavior.

Conceptually:

```text
sindri-platform
      │
      ├── desktop
      │     ├── window
      │     ├── native input
      │     ├── filesystem
      │     └── audio/device integration
      │
      └── web
            ├── canvas
            ├── browser input
            ├── browser lifecycle
            ├── web asset loading
            └── JS/WASM integration
```

Avoid filling the entire engine with:

```rust
#[cfg(target_arch = "wasm32")]
```

Platform differences should be localized wherever practical.

---

# Web runtime

The browser target should compile the Rust engine to WebAssembly.

The intended stack is conceptually:

```text
TypeScript game
      │
      ▼
@sindri/engine
      │
      ▼
TypeScript wrapper/API
      │
      ▼
WASM bindings
      │
      ▼
Sindri Rust runtime
      │
      ▼
wgpu
      │
      ▼
WebGPU
      │
      ▼
Browser GPU
```

WebGPU should be the preferred browser rendering backend.

A WebGL2 fallback may be considered later if it provides worthwhile compatibility, but it should not complicate the initial architecture.

---

# First-class TypeScript API

The browser API is a major product surface.

Do not expose raw generated WebAssembly bindings directly to normal users.

Instead, create a deliberate TypeScript package such as:

```text
@sindri/engine
```

Installation should eventually be as simple as:

```bash
npm install @sindri/engine
```

Basic usage should feel natural:

```ts
import {
  Engine,
  Scene,
  Sprite,
  Vec2,
} from "@sindri/engine";

const engine = await Engine.create({
  canvas: "#game",
});

const scene = new Scene();

const player = new Sprite({
  texture: "assets/player.png",
  position: new Vec2(100, 100),
});

scene.add(player);

engine.setScene(scene);
engine.start();
```

3D should use the same engine:

```ts
import {
  Engine,
  Scene,
  Mesh,
  PerspectiveCamera,
  BoxGeometry,
  StandardMaterial,
} from "@sindri/engine";

const engine = await Engine.create({
  canvas: "#game",
});

const scene = new Scene();

const camera = new PerspectiveCamera({
  fov: 60,
});

const cube = new Mesh({
  geometry: new BoxGeometry(1, 1, 1),
  material: new StandardMaterial(),
});

scene.add(camera);
scene.add(cube);

engine.setScene(scene);
engine.start();
```

These examples are illustrative rather than strict API requirements.

API quality should be judged by ergonomics, predictability, typing, and runtime efficiency.

---

# WASM boundary rules

Avoid extremely chatty communication between JavaScript and WebAssembly.

Bad architecture:

```ts
for (const sprite of sprites) {
  wasm.drawSprite(
    sprite.x,
    sprite.y,
    sprite.texture,
  );
}
```

Better architecture:

```ts
const enemy = world.spawn({
  transform: {
    x: 100,
    y: 100,
  },
  sprite: {
    texture: "enemy",
  },
});
```

The Rust runtime should own the authoritative world and process bulk systems internally.

Prefer crossing the JS/WASM boundary for:

- commands
- events
- component changes
- high-level queries
- asset operations
- lifecycle hooks

rather than individual render operations.

---

# TypeScript gameplay

Web developers should be able to build entire games in TypeScript without writing Rust.

Potential gameplay APIs include:

```ts
engine.onUpdate((dt) => {
  if (engine.input.keyDown("ArrowRight")) {
    player.position.x += speed * dt;
  }
});
```

and eventually structured behavior systems.

However, performance implications must be considered carefully.

If TypeScript behavior needs to inspect or mutate thousands of entities every frame, the engine should provide bulk or batched APIs rather than requiring thousands of individual Wasm calls.

---

# Native Rust gameplay

Rust should remain a first-class game development option.

A native game should not require JavaScript.

A conceptual API might remain similar to:

```rust
struct MyGame;

impl Game for MyGame {
    fn update(
        &mut self,
        ctx: &mut EngineContext,
    ) -> Result<()> {
        Ok(())
    }
}
```

Both Rust and TypeScript should ultimately operate against the same underlying world and scene concepts.

---

# 2D module

The 2D module should build on the shared engine and renderer.

Expected long-term capabilities include:

- sprites
- sprite sheets
- animation
- tilemaps
- orthographic cameras
- text
- particles
- 2D lights where appropriate
- camera effects
- sprite batching
- layers
- parallax
- pixel-perfect rendering
- 2D physics integration
- grid utilities
- pathfinding

Existing Sindri 2D functionality should be used as source material rather than rewritten unnecessarily.

---

# 3D module

The 3D module should be introduced progressively.

Do not start with a large physically based rendering system.

Initial milestones should be:

1. triangle
2. colored cube
3. camera
4. depth buffering
5. textured cube
6. mesh abstraction
7. basic materials
8. asset-loaded mesh
9. basic directional light
10. simple scene with several meshes

Later capabilities may include:

- glTF
- skeletal animation
- normal maps
- PBR materials
- multiple lights
- shadows
- skyboxes
- instancing
- post-processing
- Rapier3D

Only add these after the base renderer and scene architecture are proven.

---

# Isometric support

Isometric should be a specialized engine module rather than the identity of the engine.

Possible package:

```text
sindri-iso
```

Responsibilities could include:

- isometric projection
- isometric tile grids
- world/grid conversion
- tile occupancy
- grid pathfinding helpers
- placement footprints
- wall-edge placement
- depth rules for sprite-based isometric worlds
- orthographic-isometric camera helpers
- isometric editor grid overlays

Support both:

## Sprite-based isometric games

```text
3D-looking world
rendered using 2D sprites
```

and:

## Realtime 3D isometric games

```text
actual 3D world
rendered through an orthographic camera
```

The two approaches may share grid and gameplay logic.

---

# Physics

Physics should be modular.

Existing Rapier2D integration can remain the 2D physics backend.

Long-term:

```text
sindri-physics
  ├── sindri-physics-2d
  │     └── Rapier2D
  │
  └── sindri-physics-3d
        └── Rapier3D
```

The base engine should not require physics.

Games that do not use physics should not need to initialize or ship unnecessary physics functionality.

---

# Input

Input should expose consistent concepts across desktop and web.

Potential concepts:

```text
keyboard
mouse
pointer
touch
gamepad
actions
axes
```

Game code should generally use action mappings rather than hardcoding platform input everywhere.

Example:

```ts
if (input.actionDown("move_right")) {
  // ...
}
```

The platform backend translates browser/native input into common engine input state.

---

# Asset system

The asset system should support at minimum:

- textures
- sprite sheets
- audio
- shaders
- fonts
- mesh/model assets
- materials
- scenes

Assets should use stable logical identifiers or handles.

Avoid exposing raw filesystem paths as the permanent runtime representation because browser games do not have normal filesystem access.

The same project should ideally resolve:

```text
assets/player.png
```

appropriately whether running:

- from disk
- in the editor
- from a native build
- from a web server

---

# Web asset loading

Browser asset loading should use asynchronous APIs naturally.

The engine should hide unnecessary differences between native and browser asset loading while avoiding fake synchronous abstractions over genuinely asynchronous browser behavior.

Engine startup may therefore be async on web:

```ts
const engine = await Engine.create({
  canvas: "#game",
});
```

This is acceptable and preferable to awkward hidden initialization behavior.

---

# Scripting architecture

Scripting should become optional rather than inseparable from the engine core.

Potential architecture:

```text
sindri-script
   │
   ├── Lua
   └── potentially other adapters
```

Lua can continue to be supported for native/editor-oriented workflows.

The browser TypeScript API itself can act as the scripting/gameplay layer for many web projects.

Do not require Lua inside every web build unless there is a strong reason.

The engine should keep runtime scripting interfaces generic enough that different scripting strategies can coexist.

---

# Sindri Editor

The existing Sindri Editor should be preserved and evolved.

It should become the main visual authoring environment for all Sindri project types.

The editor remains conceptually:

```text
Tauri
  +
React / TypeScript UI
  +
Sindri engine/runtime integration
```

The editor should not become a separate engine implementation.

It should operate on the same:

- scenes
- components
- entities
- assets
- serialization formats
- engine metadata

used by actual games.

---

# Editor responsibilities

The long-term editor should support:

- project browser
- scene hierarchy
- entity creation/deletion
- component inspector
- transform editing
- asset browser
- scene loading/saving
- undo/redo
- script editing
- 2D viewport
- 3D viewport
- isometric grid tools
- gizmos
- camera controls
- play mode
- pause/step
- web preview
- native preview
- build/export controls
- debugging information
- AI-assisted engine actions

Do not require all of this in the first version.

---

# Editor viewport

The editor viewport should render using the actual Sindri runtime rather than reimplementing game rendering in React.

Conceptually:

```text
Editor UI
   │
   ▼
Sindri viewport integration
   │
   ▼
Actual scene
   │
   ▼
Actual Sindri renderer
```

The viewport should eventually support:

```text
2D mode
3D mode
isometric mode
```

based on the scene and active tools.

---

# 2D editor mode

Typical tools:

- translate
- rotate
- scale
- sprite bounds
- tile painting
- collision outlines
- camera framing
- anchor editing
- pixel snapping
- grid snapping

---

# 3D editor mode

Typical tools:

- translate gizmo
- rotation gizmo
- scale gizmo
- perspective camera navigation
- orthographic views
- lighting visualization
- camera preview
- mesh bounds
- collider visualization

---

# Isometric editor mode

Potential tools:

- isometric grid
- tile placement
- footprint placement
- occupancy visualization
- wall-edge tools
- height layers
- pathfinding overlays
- interaction spot visualization

This is where lessons learned from IsoGame can be applied without making the engine itself isometric-specific.

---

# Native and web previews

The editor should eventually provide two preview targets.

```text
▶ Native Preview

▶ Web Preview
```

Native Preview runs the native Sindri runtime.

Web Preview runs the actual WASM/browser build, preferably in an embedded web view or development browser environment.

This ensures that browser-specific issues can be tested without deploying the game.

---

# Project format

A Sindri project should be target-independent where possible.

Conceptually:

```text
my-game/
├── assets/
├── scenes/
├── scripts/
├── src/
├── sindri.toml
└── package.json       # when TypeScript/web is used
```

A Rust-authored project may also contain:

```text
Cargo.toml
```

The same scene and asset files should remain usable across native and web builds.

---

# Project configuration

A configuration file such as:

```text
sindri.toml
```

could eventually define:

```toml
[project]
name = "My Game"

[window]
width = 1280
height = 720

[features]
2d = true
3d = true
physics2d = false
physics3d = true

[web]
canvas = "#game"

[assets]
root = "assets"
```

This format is illustrative.

Avoid designing an enormous configuration schema before features require it.

---

# CLI

A Sindri CLI would improve onboarding and build workflows.

Possible commands:

```bash
sindri new my-game

sindri dev

sindri build

sindri build --target web

sindri build --target native

sindri editor

sindri test
```

For npm users:

```bash
npm create sindri-game
```

could create a browser-focused project.

---

# Browser project experience

A generated TypeScript project might look like:

```text
my-game/
├── public/
│   └── assets/
├── src/
│   ├── main.ts
│   └── game.ts
├── index.html
├── sindri.toml
├── package.json
└── tsconfig.json
```

The desired workflow:

```bash
npm install
npm run dev
```

and the developer immediately gets a WebGPU/WASM-powered Sindri game.

---

# Distribution

For web projects, builds may eventually produce:

```text
dist/
├── index.html
├── game.js
├── sindri.wasm
├── assets/
└── optional generated chunks
```

The result should work on ordinary static hosting where possible.

Avoid requiring a Sindri-specific production server.

---

# Editor and runtime parity

Retain Sindri's existing philosophy that a feature is not truly complete if only the runtime understands it.

Important systems should remain aligned across:

```text
engine
runtime
serialization
editor
web API
AI tooling
```

For example, if a new `PointLight` component is added:

```text
Engine        understands PointLight
Serialization saves PointLight
Editor        edits PointLight
Web API       exposes PointLight
AI tools      understand PointLight
```

This parity should remain an explicit engineering priority.

---

# AI tooling

Preserve the existing AI-assisted tooling direction.

The AI should operate against actual engine concepts, not merely UI automation.

Examples:

```text
"Add a point light above the player."

"Create a 10 by 10 isometric room."

"Give this entity a 3D rigid body."

"Add a camera and point it toward the selected object."

"Create a TypeScript behavior that makes this entity spin."

"Add collision to every wall."
```

AI operations should execute through structured engine/editor commands.

Local-first AI support may remain valuable, but AI should not become a dependency of the engine runtime.

---

# Lessons to bring over from IsoGame

The existing IsoGame project should not be copied wholesale into Sindri.

However, several concepts are valuable and should inform future systems.

These include:

- precise isometric projection
- world-to-screen conversion
- screen-to-world conversion
- tile occupancy
- furniture/object footprints
- pathfinding
- placement rules
- layered placement planes
- interaction points
- generated sprite anchors
- pixel-perfect integer zoom
- depth ordering
- wall-edge placement
- sprite-based isometric rendering
- generated asset metadata

These belong in reusable engine or tooling modules, not in a Habbo-specific application layer.

---

# Asset generation pipeline

A particularly interesting future direction is to connect 3D-authored assets to both 2D and 3D runtime usage.

Conceptually:

```text
3D asset
   │
   ├── realtime 3D
   │
   └── generated 2D representation
          │
          ├── sprites
          ├── sprite sheets
          ├── anchors
          ├── footprints
          ├── collision metadata
          └── interaction points
```

This could allow the same source asset to be used in:

- a 3D Sindri game
- a sprite-based isometric Sindri game

Do not make this a blocker for the core engine architecture.

It is a future differentiator.

---

# Public API design

Public APIs should be:

- typed
- explicit
- predictable
- relatively small
- composable
- difficult to misuse
- stable where possible

Avoid exposing renderer implementation details unless users need low-level control.

Prefer:

```ts
const sprite = world.spawn({
  transform: {
    position: [100, 100],
  },
  sprite: {
    texture: "player",
  },
});
```

over APIs that require users to understand command encoders, GPU buffers, or render passes for ordinary game code.

Advanced low-level APIs may exist separately.

---

# Engine lifecycle

A consistent lifecycle should exist across native and web.

Potential stages:

```text
create
initialize
load
start
update
fixed update
render
pause
resume
resize
stop
destroy
```

Exact public APIs may differ between Rust and TypeScript, but semantics should remain consistent.

---

# Time and simulation

The engine should maintain:

- frame delta
- elapsed time
- fixed-step simulation
- maximum frame delta safeguards
- pause state
- optional time scale

Simulation should not assume 60 FPS.

Rendering frame rate and fixed simulation rate should be separable.

---

# Rendering proof-of-concept milestones

Before building broad engine features, validate the rendering architecture.

## Phase 1

Render:

```text
triangle
```

## Phase 2

Render:

```text
colored 3D cube
```

with:

- projection matrix
- camera
- depth buffer

## Phase 3

Render:

```text
textured cube
```

## Phase 4

Render:

```text
2D textured sprite
```

through the new renderer.

## Phase 5

Render:

```text
3D cube + 2D sprite
```

from one engine runtime.

This proves that 2D and 3D can coexist without requiring separate engines.

---

# First web milestone

The first web milestone should be intentionally small.

A TypeScript project should be able to:

```ts
const engine = await Engine.create({
  canvas: "#game",
});

engine.start();
```

and display GPU-rendered content through Wasm/WebGPU.

Then add:

- keyboard input
- pointer input
- resize handling
- asset loading
- one sprite
- one mesh

Do not attempt to port every existing Sindri feature to web before proving the basic architecture.

---

# First editor milestone

The editor should successfully load the new engine model and display a scene containing:

- one camera
- one sprite
- one cube

It should allow selecting an entity and editing its transform.

That simple test validates:

```text
scene serialization
runtime
editor
2D
3D
component inspection
```

at once.

---

# Migration strategy from existing Sindri

Treat the current Sindri engine as a source of proven systems.

Do not blindly copy every dependency and architectural decision.

For each existing subsystem:

1. understand the current implementation
2. determine whether it belongs in core, 2D, platform, editor, or tooling
3. identify assumptions that prevent web or 3D support
4. port or refactor the useful behavior
5. add tests
6. validate native behavior
7. validate browser compatibility where relevant

Likely systems worth carrying forward include:

- engine lifecycle
- input model
- cameras
- sprite batching
- text rendering
- tilemaps
- particles
- lighting concepts
- world/entity model
- scene serialization
- undo/redo
- A* pathfinding
- scripting concepts
- editor protocol
- AI action architecture

---

# Avoid carrying forward unnecessary coupling

The new architecture should avoid making core engine crates directly depend on features such as:

- desktop windowing
- audio
- Lua
- Rapier
- editor server
- HTTP
- AI
- Tauri

These should be optional or layered dependencies.

A simple engine build should remain simple.

---

# Dependency philosophy

Use established Rust libraries where they solve difficult generic infrastructure well.

Examples may include:

- `wgpu`
- `winit`
- `glam`
- `serde`
- Rapier
- `wasm-bindgen`
- `web-sys`

Do not reinvent foundational libraries merely to say Sindri owns every layer.

Build differentiation in:

- developer experience
- engine integration
- editor/runtime parity
- web/native consistency
- clean APIs
- tooling
- workflow
- isometric/grid support

---

# Performance philosophy

Do not optimize everything prematurely.

Performance-sensitive architecture should focus first on obvious boundaries:

- batching render operations
- minimizing JS/WASM calls
- avoiding unnecessary allocations in frame loops
- GPU-friendly render data
- sensible asset lifetime management
- efficient scene traversal
- stable entity handles

Use profiling before introducing complicated systems.

---

# Testing strategy

The project should include several levels of testing.

## Unit tests

Good candidates:

- transforms
- matrices
- projection math
- asset handles
- entity operations
- scene serialization
- pathfinding
- grid conversion
- isometric conversion
- event behavior

## Rendering tests

Where practical:

- offscreen rendering
- known pixel output
- screenshot regression

## Browser tests

Validate:

- Wasm startup
- WebGPU initialization
- canvas resize
- keyboard input
- pointer input
- asset loading
- TypeScript API behavior

## Editor integration tests

Validate:

- scene loading
- scene saving
- component editing
- undo/redo
- engine/editor parity

---

# Example suite

Keep examples small and curated.

Do not accumulate dozens of abandoned demonstrations.

Suggested eventual examples:

```text
examples/
  hello-sindri/
  hello-2d/
  platformer/
  scripted-asteroids/
  hello-3d/
  iso-room/
  web-2d/
  web-3d/
  editor-scene/
```

Every example should demonstrate a real workflow.

---

# Documentation

Documentation should be considered part of the engine product.

Key documentation areas:

```text
getting started
architecture
2D rendering
3D rendering
web deployment
TypeScript API
Rust API
world/entities
scenes
assets
input
physics
isometric/grid systems
editor
scripting
build targets
```

Rust and TypeScript examples should be shown side-by-side where appropriate.

---

# Development priorities

The project should be developed in roughly this order.

## Stage 1 — Foundation

Establish:

- clean workspace
- core lifecycle
- shared world/entity model
- scene format
- platform abstraction
- GPU initialization
- desktop build
- browser build

## Stage 2 — Renderer validation

Implement:

- triangle
- cube
- sprite
- camera
- texture
- depth
- combined 2D/3D scene

## Stage 3 — Web SDK

Implement:

- Wasm bindings
- TypeScript wrapper
- npm package
- canvas setup
- input
- resize
- assets
- simple gameplay callback

## Stage 4 — Existing 2D capability migration

Bring over:

- sprites
- sprite batching
- tilemaps
- text
- particles
- Camera2D
- pathfinding
- useful existing systems

## Stage 5 — Editor migration

Make the editor understand:

- new project format
- new world/entity model
- Transform2D
- Transform3D
- Sprite
- Mesh
- cameras

## Stage 6 — Basic 3D engine

Add:

- mesh assets
- materials
- glTF
- lighting
- simple 3D physics

## Stage 7 — Isometric module

Bring over and generalize lessons from IsoGame.

## Stage 8 — Tooling and polish

Improve:

- CLI
- npm project generator
- editor workflows
- build/export
- AI tooling
- documentation
- profiling
- examples

---

# What not to build initially

Do not block the project on:

- full PBR
- advanced shadows
- skeletal animation
- networking
- multiplayer
- visual shader graphs
- visual scripting
- advanced ECS scheduling
- render graphs
- compute shaders
- WebGL fallback
- mobile-native builds
- console support
- plugin marketplace
- terrain engine
- full animation editor
- sophisticated asset importer
- cloud services

These may become valuable later.

They are not required to prove Sindri's architecture.

---

# Success criteria for the first major release

A successful first major version of the new architecture should allow all of the following.

## Rust native

A developer can make and run a basic native Sindri game in Rust.

## TypeScript browser

A developer can install:

```bash
npm install @sindri/engine
```

and create a working WebGPU game without writing Rust.

## 2D

The engine can render and animate sprites.

## 3D

The engine can render textured meshes with a camera and depth.

## Combined scenes

2D and 3D content can exist within the same runtime.

## Editor

The Sindri Editor can open the project, inspect entities, edit transforms, and preview the actual engine scene.

## Shared scenes

The same serialized scene format can be loaded by:

- editor
- native runtime
- browser runtime

## Isometric readiness

The architecture allows an isometric/grid module without forcing isometric assumptions into the engine core.

---

# Design rule for every major decision

When deciding whether something belongs in the architecture, ask:

> Does this help Sindri remain one coherent engine across 2D, 3D, native, web, runtime, and editor?

If the answer is yes, find the correct shared abstraction.

If it only applies to:

- the browser
- desktop
- 2D
- 3D
- isometric
- scripting
- editor
- AI

keep it in the appropriate module.

Do not let specialized concerns leak into the core merely because they were implemented first.

---

# Product identity

Sindri should ultimately be positioned approximately as:

> **Sindri is a Rust-powered 2D and 3D game engine for native and web games, with a first-class TypeScript API and an integrated visual editor.**

A more developer-focused formulation:

> **Build in Rust or TypeScript. Ship native or to the browser. Use the same engine, scene model, renderer, and tools.**

Its strongest differentiators should become:

- one Rust engine for native + web
- excellent TypeScript browser API
- WebGPU-powered rendering
- unified 2D and 3D architecture
- editor/runtime parity
- strong grid and isometric tooling
- local-first tooling and AI integration
- approachable architecture rather than enormous engine complexity

---

# Final architectural direction

The intended end state is:

```text
                         Sindri Editor
                       Tauri + React/TS
                              │
                              │
                    engine/editor protocol
                              │
                              ▼
                    ┌─────────────────┐
                    │   Sindri Core   │
                    │                 │
                    │ world / scene   │
                    │ entities        │
                    │ assets          │
                    │ timing          │
                    │ events          │
                    └────────┬────────┘
                             │
             ┌───────────────┼───────────────┐
             │               │               │
             ▼               ▼               ▼
         Sindri 2D       Sindri 3D       Sindri Iso
             │               │               │
             └───────────────┼───────────────┘
                             │
                             ▼
                    Sindri Renderer
                             │
                             ▼
                            wgpu
                             │
               ┌─────────────┴─────────────┐
               │                           │
               ▼                           ▼
            Desktop                     Browser
         native executable            Rust → WASM
                                            │
                                            ▼
                                      @sindri/engine
                                      TypeScript API
                                            │
                                            ▼
                                      Browser game
```

Everything built in this repository should move toward this architecture.

The first objective is not maximum features.

The first objective is to establish the foundations strongly enough that **2D, 3D, native, browser, Rust, TypeScript, and the editor all become natural variations of the same engine rather than separate systems that happen to share a name.**
