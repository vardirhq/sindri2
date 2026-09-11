#!/usr/bin/env python3
"""Dependency-free validation for Sindri AI authoring proposals."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any

PROTOCOL_VERSION = 1
DEFAULT_MAX_RESPONSE_BYTES = 256 * 1024
DEFAULT_MAX_OPERATIONS = 32
OUTCOMES = {"proposal", "clarification", "answer", "refusal"}


@dataclass(frozen=True)
class Diagnostic:
    code: str
    path: str
    message: str

    def as_dict(self) -> dict[str, str]:
        return {"code": self.code, "path": self.path, "message": self.message}


def parse_response(
    text: str, *, max_bytes: int = DEFAULT_MAX_RESPONSE_BYTES
) -> tuple[dict[str, Any] | None, list[Diagnostic]]:
    """Parse a response under a byte limit and require one JSON object."""
    if len(text.encode("utf-8")) > max_bytes:
        return None, [
            Diagnostic(
                "limit.exceeded", "", f"response exceeds {max_bytes} bytes"
            )
        ]
    try:
        value = json.loads(text)
    except json.JSONDecodeError as error:
        return None, [
            Diagnostic("protocol.invalid_json", "", f"invalid JSON: {error.msg}")
        ]
    if not isinstance(value, dict):
        return None, [
            Diagnostic("protocol.invalid_type", "", "response must be a JSON object")
        ]
    return value, []


def validate_proposal(
    proposal: dict[str, Any],
    *,
    request_id: str | None = None,
    manifest_hash: str | None = None,
    maximum_operations: int = DEFAULT_MAX_OPERATIONS,
) -> list[Diagnostic]:
    """Validate protocol structure and proposal-local reference semantics."""
    errors: list[Diagnostic] = []
    required = {
        "protocol_version",
        "request_id",
        "manifest_hash",
        "outcome",
        "label",
        "summary",
        "question",
        "operations",
    }
    _exact_fields(proposal, required, set(), "", errors)

    version = proposal.get("protocol_version")
    if not _is_integer(version) or version != PROTOCOL_VERSION:
        errors.append(
            Diagnostic(
                "protocol.unsupported_version",
                "/protocol_version",
                f"expected protocol version {PROTOCOL_VERSION}",
            )
        )

    response_request_id = _required_string(proposal, "request_id", "", errors)
    response_manifest = _required_string(proposal, "manifest_hash", "", errors)
    _required_string(proposal, "label", "", errors)
    _required_string(proposal, "summary", "", errors)

    if request_id is not None and response_request_id != request_id:
        errors.append(
            Diagnostic("request.mismatch", "/request_id", "request ID does not match")
        )
    if manifest_hash is not None and response_manifest != manifest_hash:
        errors.append(
            Diagnostic(
                "manifest.stale",
                "/manifest_hash",
                "authoring manifest hash does not match",
            )
        )

    outcome = proposal.get("outcome")
    if outcome not in OUTCOMES:
        errors.append(
            Diagnostic(
                "protocol.invalid_value",
                "/outcome",
                f"outcome must be one of {sorted(OUTCOMES)}",
            )
        )

    question = proposal.get("question")
    if outcome == "clarification":
        if not isinstance(question, str) or not question.strip():
            errors.append(
                Diagnostic(
                    "protocol.outcome_invariant",
                    "/question",
                    "clarification requires a non-empty question",
                )
            )
    elif question is not None:
        errors.append(
            Diagnostic(
                "protocol.outcome_invariant",
                "/question",
                "question must be null unless outcome is clarification",
            )
        )

    operations = proposal.get("operations")
    if not isinstance(operations, list):
        errors.append(
            Diagnostic("protocol.invalid_type", "/operations", "must be an array")
        )
        return errors
    if len(operations) > maximum_operations:
        errors.append(
            Diagnostic(
                "limit.exceeded",
                "/operations",
                f"proposal exceeds {maximum_operations} operations",
            )
        )
    if outcome == "proposal" and not operations:
        errors.append(
            Diagnostic(
                "protocol.outcome_invariant",
                "/operations",
                "proposal outcome requires at least one operation",
            )
        )
    if outcome in OUTCOMES - {"proposal"} and operations:
        errors.append(
            Diagnostic(
                "protocol.outcome_invariant",
                "/operations",
                f"{outcome} outcome requires an empty operation list",
            )
        )

    aliases: set[str] = set()
    despawned: set[str] = set()
    for index, operation in enumerate(operations[:maximum_operations]):
        path = f"/operations/{index}"
        if not isinstance(operation, dict):
            errors.append(Diagnostic("protocol.invalid_type", path, "must be an object"))
            continue
        op = operation.get("op")
        if not isinstance(op, str):
            errors.append(
                Diagnostic("protocol.missing_field", f"{path}/op", "op is required")
            )
            continue
        _validate_operation(operation, path, aliases, despawned, errors)
    return errors


def _validate_operation(
    operation: dict[str, Any],
    path: str,
    aliases: set[str],
    despawned: set[str],
    errors: list[Diagnostic],
) -> None:
    op = operation["op"]
    specs: dict[str, tuple[set[str], set[str]]] = {
        "set_scene_name": ({"op", "name"}, set()),
        "spawn": ({"op", "alias", "name", "parent", "components"}, set()),
        "despawn": ({"op", "entity"}, set()),
        "set_name": ({"op", "entity", "name"}, set()),
        "set_transform": (
            {"op", "entity", "translation", "rotation_degrees", "scale"},
            set(),
        ),
        "set_parent": ({"op", "entity", "parent"}, set()),
        "set_component": ({"op", "entity", "type_name", "payload"}, set()),
        "remove_component": ({"op", "entity", "type_name"}, set()),
        "set_disabled": ({"op", "entity", "disabled"}, set()),
    }
    if op not in specs:
        errors.append(
            Diagnostic("protocol.unknown_operation", f"{path}/op", f"unknown operation {op!r}")
        )
        return
    required, optional = specs[op]
    _exact_fields(operation, required, optional, path, errors)

    if op == "set_scene_name":
        _required_string(operation, "name", path, errors)
        return
    if op == "spawn":
        alias = _required_string(operation, "alias", path, errors)
        _required_string(operation, "name", path, errors)
        duplicate_alias = alias in aliases if alias else False
        if alias:
            if duplicate_alias:
                errors.append(
                    Diagnostic(
                        "reference.duplicate_alias", f"{path}/alias", "alias already declared"
                    )
                )
        _entity_ref(operation.get("parent"), f"{path}/parent", aliases, errors, nullable=True)
        if alias and not duplicate_alias:
            aliases.add(alias)
        components = operation.get("components")
        if not isinstance(components, dict):
            errors.append(
                Diagnostic("protocol.invalid_type", f"{path}/components", "must be an object")
            )
        else:
            for type_name, payload in components.items():
                if not isinstance(type_name, str) or not type_name:
                    errors.append(
                        Diagnostic(
                            "protocol.invalid_value",
                            f"{path}/components",
                            "component names must be non-empty strings",
                        )
                    )
                _json_value(payload, f"{path}/components/{_pointer(type_name)}", 0, errors)
        return

    entity = operation.get("entity")
    target = _entity_ref(entity, f"{path}/entity", aliases, errors)
    if target is not None and target in despawned:
        errors.append(
            Diagnostic("reference.use_after_despawn", f"{path}/entity", "entity was despawned")
        )
    if op == "despawn":
        if target is not None:
            despawned.add(target)
        return
    if op == "set_name":
        _required_string(operation, "name", path, errors)
    elif op == "set_transform":
        _vector(operation.get("translation"), 3, f"{path}/translation", errors)
        _vector(operation.get("rotation_degrees"), 3, f"{path}/rotation_degrees", errors)
        _vector(operation.get("scale"), 3, f"{path}/scale", errors)
    elif op == "set_parent":
        _entity_ref(operation.get("parent"), f"{path}/parent", aliases, errors, nullable=True)
    elif op == "set_component":
        _required_string(operation, "type_name", path, errors)
        _json_value(operation.get("payload"), f"{path}/payload", 0, errors)
    elif op == "remove_component":
        _required_string(operation, "type_name", path, errors)
    elif op == "set_disabled" and not isinstance(operation.get("disabled"), bool):
        errors.append(
            Diagnostic("protocol.invalid_type", f"{path}/disabled", "must be a boolean")
        )


def _entity_ref(
    value: Any,
    path: str,
    aliases: set[str],
    errors: list[Diagnostic],
    *,
    nullable: bool = False,
) -> str | None:
    if value is None and nullable:
        return None
    if not isinstance(value, dict):
        errors.append(Diagnostic("protocol.invalid_type", path, "must be an entity reference"))
        return None
    keys = set(value)
    if keys == {"scene_id"}:
        scene_id = value["scene_id"]
        if not isinstance(scene_id, str) or not scene_id:
            errors.append(
                Diagnostic("protocol.invalid_value", f"{path}/scene_id", "must be non-empty")
            )
            return None
        return f"scene:{scene_id}"
    if keys == {"alias"}:
        alias = value["alias"]
        if not isinstance(alias, str) or not alias:
            errors.append(
                Diagnostic("protocol.invalid_value", f"{path}/alias", "must be non-empty")
            )
            return None
        if alias not in aliases:
            errors.append(
                Diagnostic(
                    "reference.unknown_alias", f"{path}/alias", "alias is not declared earlier"
                )
            )
            return None
        return f"alias:{alias}"
    errors.append(
        Diagnostic(
            "protocol.invalid_entity_ref",
            path,
            "expected exactly one scene_id or alias field",
        )
    )
    return None


def _exact_fields(
    value: dict[str, Any],
    required: set[str],
    optional: set[str],
    path: str,
    errors: list[Diagnostic],
) -> None:
    for field in sorted(required - value.keys()):
        errors.append(
            Diagnostic("protocol.missing_field", f"{path}/{field}", "field is required")
        )
    for field in sorted(value.keys() - required - optional):
        errors.append(
            Diagnostic("protocol.unknown_field", f"{path}/{field}", "field is not allowed")
        )


def _required_string(
    value: dict[str, Any], field: str, path: str, errors: list[Diagnostic]
) -> str | None:
    item = value.get(field)
    if not isinstance(item, str) or not item.strip():
        errors.append(
            Diagnostic("protocol.invalid_type", f"{path}/{field}", "must be a non-empty string")
        )
        return None
    return item


def _vector(value: Any, size: int, path: str, errors: list[Diagnostic]) -> None:
    if not isinstance(value, list) or len(value) != size:
        errors.append(
            Diagnostic("protocol.invalid_type", path, f"must be an array of {size} numbers")
        )
        return
    for index, item in enumerate(value):
        if not _is_number(item) or not math.isfinite(item):
            errors.append(
                Diagnostic(
                    "protocol.invalid_number", f"{path}/{index}", "must be a finite number"
                )
            )


def _json_value(value: Any, path: str, depth: int, errors: list[Diagnostic]) -> None:
    if depth > 16:
        errors.append(Diagnostic("limit.exceeded", path, "JSON value exceeds maximum depth"))
        return
    if value is None or isinstance(value, (str, bool)):
        return
    if _is_number(value):
        if not math.isfinite(value):
            errors.append(Diagnostic("protocol.invalid_number", path, "must be finite"))
        return
    if isinstance(value, list):
        for index, item in enumerate(value):
            _json_value(item, f"{path}/{index}", depth + 1, errors)
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                errors.append(Diagnostic("protocol.invalid_type", path, "object keys must be strings"))
                continue
            _json_value(item, f"{path}/{_pointer(key)}", depth + 1, errors)
        return
    errors.append(Diagnostic("protocol.invalid_type", path, "unsupported JSON value"))


def _pointer(value: str) -> str:
    return value.replace("~", "~0").replace("/", "~1")


def _is_integer(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _is_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)
