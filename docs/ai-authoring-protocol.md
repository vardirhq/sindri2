# Sindri AI Authoring Protocol

**Status:** Experimental design contract  
**Protocol version:** 1 draft  
**Scope:** Editor-independent AI authoring proposals  
**Last updated:** 2026-09-11

## Purpose

Sindri's AI system should be usable and testable without the editor. The editor
is one user interface for it, not its architectural home.

The AI boundary consists of provider adapters, focused context construction,
an inert proposal format, deterministic validation, and evaluation. A host can
be the Sindri editor, a CLI, a test runner, or another authoring tool. Only the
host decides whether and how a valid proposal is applied.

This protocol defines the serialized boundary between an untrusted model and a
Sindri-aware host. It deliberately does not define chat UI, model installation,
or editor integration.

## Goals

- Work with local models and local OpenAI-compatible endpoints.
- Make model output deterministic to parse and safe to reject.
- Keep generated proposals inert until a host explicitly accepts them.
- Use serialized scene identities rather than runtime entity handles.
- Allow the same proposal to be evaluated by a headless test runner.
- Produce useful errors that can be shown to a person or used for one bounded
  repair attempt.
- Version the model contract independently of provider APIs and editor UI.

## Non-goals

- Giving a model direct access to the world or filesystem.
- Serializing `WorldCommand` as the public AI protocol.
- Applying partially valid proposals.
- Hiding unsupported requests behind best-effort guesses.
- Defining autonomous background agents.
- Requiring an inference service in games built with Sindri.

## Boundary

The system has three separable parts:

| Part | Responsibility | Editor dependency |
| --- | --- | --- |
| AI core | Provider transport, context packets, proposals, validation reports, evaluation, audit records | None |
| Sindri host adapter | Export authoritative manifests and scene context; resolve references; translate accepted operations into checked commands | Engine/project APIs only |
| Editor client | Conversation UI, context controls, proposal diff, acceptance, undo presentation | Editor |

The editor may eventually own an adapter instance, but the AI core must remain
usable from a command line and test suite.

## Trust model

Every model response is untrusted input, including output produced by a local
model. A response is not executable simply because it matches JSON syntax.

A host must:

1. Parse the response under size and depth limits.
2. Validate the protocol version and exact schema.
3. Verify the authoring manifest hash.
4. Resolve all entity and asset references without guessing.
5. Validate component payloads through authoritative Sindri schemas.
6. Dry-run the complete proposal against temporary state.
7. Present the normalized result for explicit acceptance.
8. Apply all accepted operations as one checked transaction.

Failure at any stage rejects the entire proposal. Validation and preview must
not mutate project state.

## Request context

The host sends a focused, immutable context packet. Its exact transport may
vary, but the logical shape is:

```json
{
  "protocol_version": 1,
  "request_id": "0199-2d4d-7a30",
  "manifest_hash": "sha256:8f58...",
  "user_request": "Rename the selected player to Pilot",
  "selection": [
    {
      "scene_id": "player",
      "name": "Player",
      "components": ["sindri.transform2d", "sindri.sprite"]
    }
  ],
  "scene_excerpt": [],
  "schemas": [],
  "assets": [],
  "documentation": [],
  "limits": {
    "maximum_operations": 32
  }
}
```

The packet should contain only information needed for the current request.
Component schemas and documentation are selected from generated, authoritative
data rather than copied from a handwritten master prompt.

Selection is request context, not a durable identity. The model must emit the
stable `scene_id` supplied for a selected authored entity.

## Proposal envelope

Every response uses one envelope:

```json
{
  "protocol_version": 1,
  "request_id": "0199-2d4d-7a30",
  "manifest_hash": "sha256:8f58...",
  "outcome": "proposal",
  "label": "Rename player",
  "summary": "Rename Player to Pilot.",
  "question": null,
  "operations": [
    {
      "op": "set_name",
      "entity": { "scene_id": "player" },
      "name": "Pilot"
    }
  ]
}
```

Unknown fields are rejected in protocol version 1. This catches misspellings
and prevents providers from smuggling ambiguous data through permissive maps.

### Outcomes

| Outcome | Meaning | Operations |
| --- | --- | --- |
| `proposal` | The request can be represented by supported operations. | One or more |
| `clarification` | Required identity or intent is ambiguous. | Empty |
| `answer` | The request asks for explanation, not mutation. | Empty |
| `refusal` | The request is unsafe or outside the exposed capability set. | Empty |

`question` must be a non-empty string only for `clarification`. A host may use
`summary` as the user-facing answer or refusal reason.

## References

### Existing entity

```json
{ "scene_id": "player" }
```

`scene_id` is a serialized `SceneEntityId`, never a runtime `EntityId` and never
an array index. A readable entity name is descriptive context and cannot be used
as identity. If two entities named `Enemy` exist and no stable identity is
available, the correct outcome is `clarification`.

### Entity created in the same proposal

```json
{ "alias": "new_camera" }
```

Aliases are proposal-local identifiers. They must be unique, must be declared
by an earlier `spawn` operation, and never enter the saved scene.

### Assets

Asset values inside component payloads must use project-relative identifiers
provided by the request context. Absolute paths, parent traversal, URLs, and
invented asset names are rejected. The host uses component field semantics from
the authoring manifest to identify and validate asset-bearing fields.

## Version 1 operations

Protocol operations describe authoring intent. They are not a serialization of
runtime command internals.

### `set_scene_name`

```json
{ "op": "set_scene_name", "name": "Moon Garden" }
```

### `spawn`

```json
{
  "op": "spawn",
  "alias": "new_camera",
  "name": "Camera",
  "parent": { "scene_id": "camera_rig" },
  "components": {
    "sindri.transform2d": { "translation": [0.0, 0.0] }
  }
}
```

The host allocates the real `SceneEntityId`. Embedded component payloads are
validated exactly as `set_component` payloads are.

### `despawn`

```json
{ "op": "despawn", "entity": { "scene_id": "obsolete_marker" } }
```

Descendant behavior must be defined by the host command contract and included
in the preview. A provider cannot select its own cascade semantics.

### `set_name`

```json
{
  "op": "set_name",
  "entity": { "scene_id": "player" },
  "name": "Pilot"
}
```

### `set_transform`

```json
{
  "op": "set_transform",
  "entity": { "scene_id": "player" },
  "translation": [4.0, 2.0, 0.0],
  "rotation_degrees": [0.0, 0.0, 0.0],
  "scale": [1.0, 1.0, 1.0]
}
```

The host normalizes this portable representation to the current scene transform
contract. All numbers must be finite.

### `set_parent`

```json
{
  "op": "set_parent",
  "entity": { "scene_id": "camera" },
  "parent": { "scene_id": "camera_rig" }
}
```

`parent` may be `null`. Cycles and invalid cross-scene relationships are
rejected during dry-run.

### `set_component`

```json
{
  "op": "set_component",
  "entity": { "scene_id": "player" },
  "type_name": "sindri.sprite",
  "payload": {
    "texture": "textures/player.png",
    "width": 32.0,
    "height": 32.0
  }
}
```

The component type must appear in the request's manifest and its payload must
pass the registered component schema. Models must not infer fields from similar
engines.

### `remove_component`

```json
{
  "op": "remove_component",
  "entity": { "scene_id": "player" },
  "type_name": "sindri.sprite"
}
```

### `set_disabled`

```json
{
  "op": "set_disabled",
  "entity": { "scene_id": "debug_grid" },
  "disabled": true
}
```

## Deliberately excluded operations

Protocol version 1 does not expose:

- runtime entity handles
- source-ID mutation
- inverse-only restore commands
- editor metadata mutation
- arbitrary filesystem reads or writes
- shell commands
- network requests
- dependency or project-manifest changes
- direct Decay or Weave file replacement

Source editing needs its own bounded patch protocol and atomic relationship with
scene changes. It should not be approximated by adding an unrestricted
`write_file` operation. Until that contract exists, a model may explain or draft
source in an `answer`, but the proposal host cannot apply it.

## Validation pipeline

Validation produces a normalized proposal or a list of stable diagnostics:

1. **Envelope validation** checks JSON shape, size, version, outcome invariants,
   request identity, and operation count.
2. **Manifest validation** rejects proposals produced against a different
   capability hash.
3. **Reference resolution** maps scene IDs and earlier aliases to host entities.
4. **Capability validation** checks operation and component availability.
5. **Payload validation** invokes authoritative component schemas and validates
   asset fields against the project asset catalog.
6. **Structural validation** detects duplicate aliases, parent cycles, use after
   despawn, and conflicting operations.
7. **Dry-run** applies the normalized operation sequence to temporary state.
8. **Diff construction** reports the complete effect for review.

Suggested diagnostic codes include:

| Code | Meaning |
| --- | --- |
| `protocol.unsupported_version` | The envelope version is not supported. |
| `protocol.unknown_field` | The response includes a field outside the schema. |
| `request.mismatch` | The response belongs to another request. |
| `manifest.stale` | The proposal targets a different manifest hash. |
| `reference.unknown_entity` | A scene ID cannot be resolved. |
| `reference.unknown_alias` | An alias is used before declaration. |
| `reference.ambiguous` | Context cannot identify one safe target. |
| `capability.unknown_component` | The component is absent from the manifest. |
| `payload.invalid` | A component payload fails its registered schema. |
| `asset.invalid_reference` | An asset reference is unavailable or unsafe. |
| `structure.parent_cycle` | The proposed hierarchy contains a cycle. |
| `limit.exceeded` | A configured proposal resource limit was exceeded. |
| `dry_run.failed` | Checked application failed against temporary state. |

Diagnostics should carry an operation index and JSON pointer when applicable.
Human-readable text is supplementary; consumers should branch on codes.

## Application contract

Validation does not grant permission to mutate. The host must retain the
normalized proposal and preview until the user explicitly accepts it.

For a Sindri host adapter, acceptance translates the operations into normal
checked world commands. The complete group is applied atomically and recorded
as one undo step using `label`. A failure leaves the live world unchanged.

The adapter, not the AI core, owns translation to the current command API. This
keeps provider and evaluation code independent of editor internals while
preserving the editor's normal mutation and undo contracts.

## Resource and security limits

A host should configure conservative limits for:

- response bytes and JSON depth
- operation count
- payload bytes per operation
- new entities per proposal
- total referenced entities and assets
- inference timeout and retry count
- repair attempts

Provider responses must never select a new endpoint, model, API key, or context
file. Local-first mode must not silently fall back to a cloud provider.

Project text can contain prompt injection. Entity names, source comments, asset
metadata, and documentation excerpts are data, not instructions. Context packets
should label their sources, and the validator remains the enforcement boundary.

## Provider independence

The logical request and proposal schema are provider-neutral. Adapters may use:

- a provider's strict JSON schema response mode
- tool calling with one `submit_proposal` tool
- constrained decoding
- plain JSON output as a last resort

All paths must converge on the same parser and validator. Provider-specific tool
call envelopes are transport details and must not leak into stored proposals.

## Headless conformance testing

The protocol is intentionally testable without Sindri or the editor running.
A standalone evaluator can supply fixed context packets to any compatible local
endpoint, parse the response, and score:

- valid envelope production
- exact request and manifest binding
- correct outcome selection
- stable entity targeting
- operation and component selection
- refusal to invent unavailable capabilities
- resistance to instructions embedded in project data
- latency and malformed-output rate

Deterministic structural checks are the first gate. Semantic checks should use
small expectations such as required operation kinds, exact target IDs, forbidden
operations, and expected clarification. Model prose is not graded for style.

A useful local model is one that passes a published Sindri corpus repeatedly,
not merely one that responds to an HTTP health check.

## Versioning

`protocol_version` changes only for incompatible envelope or operation changes.
New optional capabilities should normally be advertised through the authoring
manifest. Version 1 rejects unknown fields, so compatible schema extensions must
be introduced deliberately rather than assumed.

Stored audit records should include:

- protocol version
- request ID
- manifest hash
- provider and model identifier
- a hash of the context packet
- raw response or its hash, according to user privacy settings
- normalized proposal
- validation diagnostics
- user decision
- application result

Audit records must not contain provider credentials.

## Host adapter readiness

What a Sindri host adapter needs from the engine, audited against it rather than
assumed. The operations were designed against the real command surface, and it
shows: every one of them already had a checked command behind it.

| Protocol requirement | Engine | Where |
| --- | :-: | --- |
| `set_scene_name` | ✅ | `WorldCommand::SetSceneName` |
| `spawn` | ✅ | `WorldCommand::Spawn` |
| `despawn` | ✅ | `WorldCommand::Despawn` |
| `set_name` | ✅ | `WorldCommand::SetName` |
| `set_transform` | ✅ | `WorldCommand::SetTransform3D` |
| `set_parent` | ✅ | `WorldCommand::SetParent` |
| `set_component` | ✅ | `WorldCommand::SetComponent` |
| `remove_component` | ✅ | `WorldCommand::RemoveComponent` |
| `set_disabled` | ✅ | `WorldCommand::SetDisabled` |
| Applied atomically, one undo step carrying `label`, failure leaves the world unchanged | ✅ | `Transaction` — a rejected command rolls back the ones already applied |
| Resolve entity references without guessing | ✅ | `World::entity_for_source_id`, `World::source_id_map` |
| Dry-run the complete proposal against temporary state | ✅ | `Transaction::rehearse` |
| Verify the authoring manifest hash | ⚠️ | `docs/generated/sindri-capabilities.json` carries the components and its schema, engine and scene-format versions, but no hash. Blocked on the first open question below |

Resolution is unambiguous by construction rather than by convention:
`WorldCommand::SetSourceId` refuses to give two entities the same stable ID, and
a scene carrying a duplicate fails to load. So a reference resolves to one
entity or to nothing, and a host never has to choose between candidates.

`rehearse` exists because validation must never be the thing that mutates.
`Transaction::apply` already rolls back a rejected group, so this is not about
safety — it is about answering *what would this do* without the answer being
visible in the editor for a frame, and about the preview being the real outcome
rather than a description of one.

## Open design questions

- The exact generated authoring manifest schema and compatibility policy.
- A bounded source-patch protocol for Decay and Weave.
- Atomic coordination between world transactions and accepted source edits.
- Whether aliases should support references across separately approved proposals.
- Which proposal resource limits should vary by project or model tier.
- The minimum corpus pass rates required for Sindri to recommend a local model.

These questions do not block headless protocol evaluation. They should be
resolved before editor application support is enabled.
