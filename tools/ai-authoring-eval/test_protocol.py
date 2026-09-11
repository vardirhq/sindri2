#!/usr/bin/env python3
"""Offline tests for the Sindri AI proposal validator."""

from __future__ import annotations

import json
import math
import unittest

from protocol import parse_response, validate_proposal


def proposal(*operations: dict[str, object]) -> dict[str, object]:
    return {
        "protocol_version": 1,
        "request_id": "request-1",
        "manifest_hash": "sha256:test",
        "outcome": "proposal",
        "label": "Test proposal",
        "summary": "Make the requested change.",
        "question": None,
        "operations": list(operations),
    }


class ParseTests(unittest.TestCase):
    def test_parses_object(self) -> None:
        value, errors = parse_response(json.dumps(proposal({
            "op": "set_scene_name", "name": "Test"
        })))
        self.assertIsNotNone(value)
        self.assertEqual(errors, [])

    def test_rejects_non_object(self) -> None:
        value, errors = parse_response("[]")
        self.assertIsNone(value)
        self.assertEqual(errors[0].code, "protocol.invalid_type")

    def test_enforces_response_size(self) -> None:
        value, errors = parse_response("{}", max_bytes=1)
        self.assertIsNone(value)
        self.assertEqual(errors[0].code, "limit.exceeded")


class ValidationTests(unittest.TestCase):
    def test_accepts_stable_entity_reference(self) -> None:
        value = proposal({
            "op": "set_name",
            "entity": {"scene_id": "player"},
            "name": "Pilot",
        })
        self.assertEqual(
            validate_proposal(
                value, request_id="request-1", manifest_hash="sha256:test"
            ),
            [],
        )

    def test_accepts_earlier_spawn_alias(self) -> None:
        value = proposal(
            {
                "op": "spawn",
                "alias": "rig",
                "name": "Camera Rig",
                "parent": None,
                "components": {},
            },
            {
                "op": "spawn",
                "alias": "camera",
                "name": "Camera",
                "parent": {"alias": "rig"},
                "components": {},
            },
        )
        self.assertEqual(validate_proposal(value), [])

    def test_rejects_forward_alias_reference(self) -> None:
        value = proposal(
            {
                "op": "spawn",
                "alias": "camera",
                "name": "Camera",
                "parent": {"alias": "rig"},
                "components": {},
            },
            {
                "op": "spawn",
                "alias": "rig",
                "name": "Camera Rig",
                "parent": None,
                "components": {},
            },
        )
        errors = validate_proposal(value)
        self.assertIn("reference.unknown_alias", {error.code for error in errors})

    def test_rejects_spawn_parented_to_its_own_alias(self) -> None:
        value = proposal({
            "op": "spawn",
            "alias": "loop",
            "name": "Loop",
            "parent": {"alias": "loop"},
            "components": {},
        })
        errors = validate_proposal(value)
        self.assertIn("reference.unknown_alias", {error.code for error in errors})

    def test_rejects_runtime_entity_id_shape(self) -> None:
        value = proposal({
            "op": "set_disabled", "entity": {"entity_id": 42}, "disabled": True
        })
        errors = validate_proposal(value)
        self.assertIn("protocol.invalid_entity_ref", {error.code for error in errors})

    def test_rejects_unknown_fields(self) -> None:
        value = proposal({"op": "set_scene_name", "name": "Test"})
        value["confidence"] = 0.95
        errors = validate_proposal(value)
        self.assertIn("protocol.unknown_field", {error.code for error in errors})

    def test_rejects_non_finite_transform(self) -> None:
        value = proposal({
            "op": "set_transform",
            "entity": {"scene_id": "player"},
            "translation": [math.inf, 0.0, 0.0],
            "rotation_degrees": [0.0, 0.0, 0.0],
            "scale": [1.0, 1.0, 1.0],
        })
        errors = validate_proposal(value)
        self.assertIn("protocol.invalid_number", {error.code for error in errors})

    def test_clarification_requires_question_and_no_operations(self) -> None:
        value = proposal({"op": "set_scene_name", "name": "Test"})
        value["outcome"] = "clarification"
        value["question"] = None
        errors = validate_proposal(value)
        codes = [error.code for error in errors]
        self.assertEqual(codes.count("protocol.outcome_invariant"), 2)

    def test_rejects_use_after_despawn(self) -> None:
        value = proposal(
            {"op": "despawn", "entity": {"scene_id": "marker"}},
            {
                "op": "set_name",
                "entity": {"scene_id": "marker"},
                "name": "Too Late",
            },
        )
        errors = validate_proposal(value)
        self.assertIn("reference.use_after_despawn", {error.code for error in errors})

    def test_enforces_operation_limit(self) -> None:
        value = proposal(
            {"op": "set_scene_name", "name": "One"},
            {"op": "set_scene_name", "name": "Two"},
        )
        errors = validate_proposal(value, maximum_operations=1)
        self.assertIn("limit.exceeded", {error.code for error in errors})


if __name__ == "__main__":
    unittest.main()
