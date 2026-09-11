#!/usr/bin/env python3
"""Offline tests for corpus scoring and provider transport."""

from __future__ import annotations

import json
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import run


def envelope(
    *,
    request_id: str = "request-1",
    manifest_hash: str = "sha256:test",
    outcome: str = "proposal",
    question: str | None = None,
    operations: list[dict[str, object]] | None = None,
) -> dict[str, object]:
    return {
        "protocol_version": 1,
        "request_id": request_id,
        "manifest_hash": manifest_hash,
        "outcome": outcome,
        "label": "Test",
        "summary": "Test result.",
        "question": question,
        "operations": operations if operations is not None else [],
    }


class ScoreTests(unittest.TestCase):
    def test_scores_required_operation(self) -> None:
        case = {
            "id": "rename",
            "request": {
                "request_id": "request-1",
                "manifest_hash": "sha256:test",
                "limits": {"maximum_operations": 4},
            },
            "expect": {
                "outcome": "proposal",
                "exact_operation_count": 1,
                "required_operations": [
                    {
                        "op": "set_name",
                        "entity": {"scene_id": "player"},
                        "name": "Pilot",
                    }
                ],
            },
        }
        response = envelope(
            operations=[
                {
                    "op": "set_name",
                    "entity": {"scene_id": "player"},
                    "name": "Pilot",
                }
            ]
        )
        result, passed = run.score_case(case, json.dumps(response))
        self.assertTrue(passed)
        self.assertEqual(result["expectation_errors"], [])

    def test_detects_wrong_stable_target(self) -> None:
        case = {
            "id": "target",
            "request": {
                "request_id": "request-1",
                "manifest_hash": "sha256:test",
                "limits": {},
            },
            "expect": {
                "outcome": "proposal",
                "required_operations": [
                    {"op": "set_disabled", "entity": {"scene_id": "marker"}}
                ],
            },
        }
        response = envelope(
            operations=[
                {
                    "op": "set_disabled",
                    "entity": {"scene_id": "player"},
                    "disabled": True,
                }
            ]
        )
        _, passed = run.score_case(case, json.dumps(response))
        self.assertFalse(passed)

    def test_checks_spawn_parent_relationship_without_fixed_alias(self) -> None:
        operations = [
            {
                "op": "spawn",
                "alias": "arbitrary-rig-alias",
                "name": "Camera Rig",
                "parent": None,
                "components": {},
            },
            {
                "op": "spawn",
                "alias": "new-camera",
                "name": "Camera",
                "parent": {"alias": "arbitrary-rig-alias"},
                "components": {},
            },
        ]
        errors = run.check_expectations(
            envelope(operations=operations),
            {
                "outcome": "proposal",
                "parent_relationships": [
                    {"parent_name": "Camera Rig", "child_name": "Camera"}
                ],
            },
        )
        self.assertEqual(errors, [])


class RequestTests(unittest.TestCase):
    def test_plain_mode_omits_provider_constraint(self) -> None:
        case = {"request": {"request_id": "one"}}
        body = json.loads(
            run.request_body(case, "local-model", {}, "plain", 0.0).decode("utf-8")
        )
        self.assertNotIn("response_format", body)
        self.assertEqual(body["model"], "local-model")

    def test_schema_mode_sends_strict_schema(self) -> None:
        case = {"request": {"request_id": "one"}}
        body = json.loads(
            run.request_body(case, "local-model", {"type": "object"}, "schema", 0.0)
        )
        self.assertTrue(body["response_format"]["json_schema"]["strict"])


class ProviderHandler(BaseHTTPRequestHandler):
    received_authorization: str | None = None

    def do_POST(self) -> None:  # noqa: N802 - HTTP handler API
        length = int(self.headers["Content-Length"])
        json.loads(self.rfile.read(length))
        type(self).received_authorization = self.headers.get("Authorization")
        proposal = envelope(
            operations=[{"op": "set_scene_name", "name": "Observatory"}]
        )
        response = {
            "choices": [{"message": {"content": json.dumps(proposal)}}],
            "usage": {"prompt_tokens": 20, "completion_tokens": 30},
        }
        payload = json.dumps(response).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, format: str, *args: object) -> None:
        return


class TransportTests(unittest.TestCase):
    def test_calls_openai_compatible_endpoint(self) -> None:
        server = ThreadingHTTPServer(("127.0.0.1", 0), ProviderHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            content, latency_ms, usage = run.call_model(
                endpoint=f"http://127.0.0.1:{server.server_port}/v1/chat/completions",
                api_key="local-test-key",
                timeout=2.0,
                body=b"{}",
            )
        finally:
            server.shutdown()
            server.server_close()
            thread.join()
        self.assertEqual(json.loads(content)["protocol_version"], 1)
        self.assertGreaterEqual(latency_ms, 0)
        self.assertEqual(usage["completion_tokens"], 30)
        self.assertEqual(ProviderHandler.received_authorization, "Bearer local-test-key")


if __name__ == "__main__":
    unittest.main()
