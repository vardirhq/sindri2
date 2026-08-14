# Component schema registry

Scene component payloads remain JSON so a newer editor or runtime can preserve data that an older
tool does not understand. Code that needs to interpret a component registers a Rust type with
`ComponentSchemaRegistry` by implementing `SceneComponent`.

```rust
#[derive(serde::Deserialize)]
struct Health {
    current: u16,
    maximum: u16,
}

impl SceneComponent for Health {
    const TYPE_NAME: &'static str = "game.health";
}

let mut components = ComponentSchemaRegistry::default();
components.register::<Health>("Health")?;
```

Each registration records a stable type name, editor-facing display name, and schema version. The
registered Rust type supplies payload validation and typed decoding.

## Unknown components

Callers choose unknown-component behavior explicitly:

- `UnknownComponentPolicy::Preserve` validates known payloads and leaves unknown JSON untouched.
  This is the default compatibility policy for scene round-tripping and editor workflows.
- `UnknownComponentPolicy::Reject` reports the entity and unknown type. This is appropriate when a
  runtime proof requires every component to be actionable.

Known components are always schema-validated under either policy.

## Queries

`ComponentSchemaRegistry::query::<T>(&world)` returns typed `(EntityId, T)` values for every entity
carrying the registered component. Invalid runtime payloads produce an error naming both the entity
and component type rather than being silently skipped.

The combined cube/sprite example registers camera, mesh, and sprite component types, validates its
embedded scene in strict mode, and uses the typed query API during frame extraction.
