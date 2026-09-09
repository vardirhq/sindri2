# Gather showcase contract

Gather is Sindri's polished, approachable showcase. Orbital Last Stand forces
new capabilities into existence; Gather demonstrates capabilities the engine
has already earned in one coherent game. It should become richer as Sindri
does, but it must not become a smaller combat game or an excuse to invent an
unrelated subsystem.

The durable test is simple: a person playing Gather should notice the feature,
not need this document to be told that a component exists.

## What Gather proves today

| Capability | Current proof | Showcase quality |
| --- | --- | --- |
| Decay | Player movement, orb collection, progress lamps, victory, and Wisp behavior are scripts; Rust hosts them without owning game rules | Strong |
| Tilemap | One authored 9x9 isometric tilemap replaces a floor made from individual sprite entities | Technically clear; regional patterning is the world-refresh foundation |
| Grid coordinates | Player movement, bounds, and orb collection use the tilemap's logical coordinates | Strong |
| Navigation | The Wisp follows the player around authored walls through shared deterministic A* | Real but visually easy to miss |
| Sprite animation | The player advances an authored clip | Real but not conspicuous |
| UI and text | A project-font title, progress lamps, and a victory banner render as screen UI | Functional, not product-like |
| Input | Arrow/WASD movement and the shared touch stick drive the same Decay player | Strong |
| Audio | Background music plus pickup and victory sounds use the shared native/browser path | Complete but lightly presented |
| Editor | The project opens, authors, renders, and plays through the normal editor surfaces | Strong |
| Browser/export | The same project ships through the content-hashed static export and browser host | Strong |
| Determinism | Scripted play and the CI capture reproduce a fixed run | Strong |

## Refresh target

The refreshed game remains small: explore an isometric place, gather its five
orbs, travel with the Wisp, and complete the objective. The work improves the
clarity, composition, and feedback of that loop rather than adding breadth for
its own sake.

| Capability | Refresh proof |
| --- | --- |
| World composition | A compact authored map with paths, obstacles, enclosed spaces, and recognizable landmarks makes its grid legible as a place |
| Navigation | Obstacles and feedback make the Wisp's route around authored walls immediately visible |
| Animation | Player, orbs, Wisp, environmental props, and completion presentation move conspicuously through existing sprite-animation support |
| Weave | Composed stylesheets present title, objective, progress, Wisp status, pause, and completion UI across desktop and portrait viewports |
| Effects | Restrained pooled bursts mark collection, the Wisp, interaction, and completion |
| Audio | Existing music, pickup, and victory paths are supplemented only where a distinct Wisp or UI action needs feedback |
| Gameplay presentation | Collecting all orbs opens or awakens a final landmark, producing a readable `explore -> collect -> return -> complete` arc |
| Verification | Deterministic desktop and portrait captures plus native, editor, and browser checks show the same authored project |

## Deliberate exclusions

These are not omissions to disguise as future Gather work:

- no combat, bosses, modules, profiles, or high-churn prefab spawning;
- no physics merely to duplicate grid movement or collection that already has
  the right model;
- no persistence until the game has progress worth remembering;
- no random layout that weakens deterministic capture and authored landmarks;
- no fake call to an API solely to turn a matrix cell green;
- no 3D prop until Sindri has a real glTF, material, and light authoring path.

Orbital Last Stand already carries the complicated gameplay proofs. Gather's
job is to make mature capabilities understandable and attractive.

## Delivery sequence

Each stage is a separate reviewable PR and leaves the game runnable:

1. **World and motion** — reshape the small map, add landmarks, and make the
   existing animation/navigation proof visible.
2. **Responsive presentation** — migrate all player-facing UI to composed
   Weave stylesheets, with desktop and portrait captures.
3. **Effects and sound polish** — add restrained collection, Wisp, interaction,
   and completion feedback through existing systems.
4. **Objective arc** — make the final landmark and return step turn five
   pickups into a complete short journey.
5. **Showcase verification** — reconcile this document and the integration
   matrix with deterministic native/browser/editor evidence.

If a stage discovers a missing engine primitive, stop that stage and prove the
primitive through Orbital Last Stand or a focused engine test first. Gather may
consume it after it is mature; Gather does not force it into existence.

## Acceptance gate

The refresh is complete when a fresh player can identify the objective, see the
Wisp navigate rather than teleport, recognize animated world elements at a
glance, understand collection and completion feedback without reading logs,
and use the interface at both desktop and portrait sizes. The same scene,
scripts, and composed stylesheets must run in the editor, native host, and
static browser export.
