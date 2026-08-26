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

## Fields and defaults are different questions

A registration may also record two payloads, and conflating them is a mistake the editor already
made once.

The **field template** is what the component *has*: every field, at the value it takes when nobody
has said. A tool draws a component by filling this out with whatever the instance stored, so the
same component shows the same rows however it was authored.

The **default payload** is what a *fresh* one is. Only some types have one. A component naming a
font, a sheet, another entity, or a clip has no blank the engine can invent, and a button that adds
a component the engine then rejects is worse than no button.

```rust
// Fields and a fresh blank, which for most types are the same object.
components.register_with_default::<Health>("Health", json!({ "current": 10, "maximum": 10 }))?;

// Fields but no blank: the host completes it from whatever it has.
components.register_with_fields::<Portrait>("Portrait", json!({ "image": "", "scale": 1.0 }))?;

// Neither: readable and editable, and drawn from whatever its payload carries.
components.register::<Mystery>("Mystery")?;
```

`register` is for a component nothing has described. Prefer one of the other two for anything an
editor will draw: without a field template a panel shows the fields the payload happens to carry,
so one added by a button and one authored by hand become different-looking components.
`sindri.ui.text` was registered that way and inspected as two rows for a seven-field component;
`sindri.tilemap`'s default omitted `projection`, so a tilemap made in the editor could never be
isometric.

Both payloads are checked at registration against what serde will actually ask the type for, so a
template that has drifted from its struct is a startup error rather than a row quietly missing from
a panel a release later. The check is skipped for a component that is not a struct — an internally
tagged enum like `sindri.camera` has fields decided by its variant.

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
