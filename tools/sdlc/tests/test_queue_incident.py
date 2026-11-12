from __future__ import annotations

import copy
import hashlib
import hmac
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools" / "sdlc"
sys.path.insert(0, str(TOOLS))

import queue_incident  # noqa: E402


def sha(value: int) -> str:
    return f"{value:040x}"


def event_payload(action: str = "checks_requested", reason: str | None = None) -> dict:
    value = {
        "action": action,
        "merge_group": {
            "head_sha": sha(220),
            "head_ref": "refs/heads/gh-readonly-queue/main/pr-120-fixture",
            "base_sha": sha(1),
            "base_ref": "refs/heads/main",
            "head_commit": {
                "id": sha(220),
                "tree_id": sha(900),
            },
        },
        "repository": {
            "full_name": "example/repo",
        },
    }
    if reason is not None:
        value["reason"] = reason
    return value


def queue_snapshot(entry_count: int = 20) -> dict:
    entries = []
    previous = sha(1)
    for position in range(1, entry_count + 1):
        integration_head = sha(200 + position)
        entries.append(
            {
                "position": position,
                "queue_entry_id": f"queue-entry-{position}",
                "pull_request": 1000 + position,
                "pull_request_head_sha": sha(100 + position),
                "integration_base_sha": previous,
                "integration_head_sha": integration_head,
                "state": "AWAITING_CHECKS",
                "jump": False,
                "solo": False,
            }
        )
        previous = integration_head
    return {
        "schema_version": 1,
        "provider": "github",
        "repository": "example/repo",
        "captured_at": "2026-11-12T07:00:00Z",
        "base_sha": sha(1),
        "head_sha": sha(200 + entry_count),
        "merge_method": "MERGE",
        "grouping_strategy": "HEADGREEN",
        "build_concurrency": 20,
        "maximum_entries_to_build": 20,
        "entries": entries,
    }


def object_manifest(snapshot: dict) -> dict:
    commits = {snapshot["base_sha"], snapshot["head_sha"]}
    for entry in snapshot["entries"]:
        commits.add(entry["pull_request_head_sha"])
        commits.add(entry["integration_base_sha"])
        commits.add(entry["integration_head_sha"])
    return {
        "schema_version": 1,
        "repository": "example/repo",
        "captured_at": "2026-11-12T07:00:01Z",
        "head_sha": snapshot["head_sha"],
        "head_tree_sha": sha(900),
        "bundle": {
            "path": "objects/merge-group.bundle",
            "size": 1234,
            "sha256": "a" * 64,
        },
        "verified_commits": sorted(commits),
        "verification": {
            "command": ["git", "bundle", "verify", "objects/merge-group.bundle"],
            "completed_at": "2026-11-12T07:00:02Z",
            "result": "passed",
        },
    }


def raw_event(payload: dict) -> bytes:
    return queue_incident.canonical_json_bytes(payload)


def build_valid_incident(entry_count: int = 20) -> dict:
    payload = event_payload()
    snapshot = queue_snapshot(entry_count)
    payload["merge_group"]["head_sha"] = snapshot["head_sha"]
    payload["merge_group"]["head_commit"]["id"] = snapshot["head_sha"]
    return queue_incident.build_incident(
        event_payload=payload,
        raw_event=raw_event(payload),
        queue_snapshot=snapshot,
        object_manifest=object_manifest(snapshot),
        delivery_id="delivery-checks-requested",
        captured_at="2026-11-12T07:00:03Z",
        signature_verified=True,
    )


class QueueIncidentTests(unittest.TestCase):
    def test_webhook_signature_verification_accepts_exact_body(self) -> None:
        body = b'{"action":"checks_requested"}'
        secret = b"fixture-secret"
        signature = "sha256=" + hmac.new(secret, body, hashlib.sha256).hexdigest()
        queue_incident.verify_webhook_signature(secret, body, signature)

    def test_webhook_signature_verification_rejects_wrong_or_malformed_value(self) -> None:
        body = b"{}"
        with self.assertRaises(queue_incident.QueueIncidentError):
            queue_incident.verify_webhook_signature(b"secret", body, "sha1=abcd")
        with self.assertRaises(queue_incident.QueueIncidentError):
            queue_incident.verify_webhook_signature(
                b"secret", body, "sha256=" + "0" * 64
            )

    def test_twenty_entry_incident_is_deterministic_and_valid(self) -> None:
        left = build_valid_incident()
        right = build_valid_incident()
        queue_incident.validate_incident(left)
        self.assertEqual(left["incident_id"], right["incident_id"])
        self.assertEqual(20, len(left["queue_snapshot"]["entries"]))
        self.assertTrue(left["incident_id"].startswith("mq-"))

    def test_queue_positions_must_be_contiguous(self) -> None:
        payload = event_payload()
        snapshot = queue_snapshot()
        snapshot["entries"][5]["position"] = 9
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError, "positions must be contiguous"
        ):
            queue_incident.build_incident(
                event_payload=payload,
                raw_event=raw_event(payload),
                queue_snapshot=snapshot,
                object_manifest=object_manifest(queue_snapshot()),
                delivery_id="delivery",
                captured_at="2026-11-12T07:00:03Z",
                signature_verified=True,
            )

    def test_queue_entries_must_form_one_cumulative_chain(self) -> None:
        payload = event_payload()
        snapshot = queue_snapshot()
        snapshot["entries"][7]["integration_base_sha"] = sha(9999)
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError, "cumulative integration chain"
        ):
            queue_incident.build_incident(
                event_payload=payload,
                raw_event=raw_event(payload),
                queue_snapshot=snapshot,
                object_manifest=object_manifest(snapshot),
                delivery_id="delivery",
                captured_at="2026-11-12T07:00:03Z",
                signature_verified=True,
            )

    def test_final_integration_head_must_match_event(self) -> None:
        payload = event_payload()
        snapshot = queue_snapshot()
        snapshot["entries"][-1]["integration_head_sha"] = sha(9999)
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError,
            "final queue entry integration head",
        ):
            queue_incident.build_incident(
                event_payload=payload,
                raw_event=raw_event(payload),
                queue_snapshot=snapshot,
                object_manifest=object_manifest(snapshot),
                delivery_id="delivery",
                captured_at="2026-11-12T07:00:03Z",
                signature_verified=True,
            )

    def test_object_manifest_must_cover_every_input_and_integration_commit(self) -> None:
        payload = event_payload()
        snapshot = queue_snapshot()
        objects = object_manifest(snapshot)
        objects["verified_commits"].remove(snapshot["entries"][9]["pull_request_head_sha"])
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError, "missing required commits"
        ):
            queue_incident.build_incident(
                event_payload=payload,
                raw_event=raw_event(payload),
                queue_snapshot=snapshot,
                object_manifest=objects,
                delivery_id="delivery",
                captured_at="2026-11-12T07:00:03Z",
                signature_verified=True,
            )

    def test_incident_validation_rejects_tampered_queue_snapshot(self) -> None:
        incident = build_valid_incident()
        incident["queue_snapshot"]["entries"][0]["state"] = "TAMPERED"
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError, "digest does not match"
        ):
            queue_incident.validate_incident(incident)

    def test_destroyed_event_appends_terminal_lifecycle_without_rewriting_identity(self) -> None:
        incident = build_valid_incident()
        payload = event_payload("destroyed", "invalidated")
        updated = queue_incident.append_destroyed(
            incident=incident,
            event_payload=payload,
            raw_event=raw_event(payload),
            delivery_id="delivery-destroyed",
            observed_at="2026-11-12T08:00:00Z",
            signature_verified=True,
        )
        self.assertEqual(incident["incident_id"], updated["incident_id"])
        self.assertEqual("invalidated", updated["status"])
        self.assertEqual(2, len(updated["lifecycle"]))
        queue_incident.validate_incident(updated)

    def test_destroyed_event_must_match_captured_head_and_be_unique(self) -> None:
        incident = build_valid_incident()
        mismatched = event_payload("destroyed", "dequeued")
        mismatched["merge_group"]["head_sha"] = sha(9999)
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError, "does not match captured incident"
        ):
            queue_incident.append_destroyed(
                incident=incident,
                event_payload=mismatched,
                raw_event=raw_event(mismatched),
                delivery_id="destroyed-1",
                observed_at="2026-11-12T08:00:00Z",
                signature_verified=True,
            )

        valid = event_payload("destroyed", "dequeued")
        updated = queue_incident.append_destroyed(
            incident=incident,
            event_payload=valid,
            raw_event=raw_event(valid),
            delivery_id="destroyed-1",
            observed_at="2026-11-12T08:00:00Z",
            signature_verified=True,
        )
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError, "terminal destroyed event"
        ):
            queue_incident.append_destroyed(
                incident=updated,
                event_payload=valid,
                raw_event=raw_event(valid),
                delivery_id="destroyed-2",
                observed_at="2026-11-12T08:01:00Z",
                signature_verified=True,
            )

    def test_cli_capture_and_validate_require_authenticated_delivery(self) -> None:
        payload = event_payload()
        snapshot = queue_snapshot()
        secret = "fixture-secret"
        body = raw_event(payload)
        signature = "sha256=" + hmac.new(
            secret.encode("utf-8"), body, hashlib.sha256
        ).hexdigest()

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            event_path = root / "event.json"
            queue_path = root / "queue.json"
            objects_path = root / "objects.json"
            incident_path = root / "incident.json"
            event_path.write_bytes(body)
            queue_path.write_text(json.dumps(snapshot), encoding="utf-8")
            objects_path.write_text(
                json.dumps(object_manifest(snapshot)), encoding="utf-8"
            )
            env = os.environ.copy()
            env["GITHUB_WEBHOOK_SECRET"] = secret

            capture = subprocess.run(
                [
                    sys.executable,
                    str(TOOLS / "queue_incident.py"),
                    "capture",
                    "--event",
                    str(event_path),
                    "--signature",
                    signature,
                    "--queue-snapshot",
                    str(queue_path),
                    "--object-manifest",
                    str(objects_path),
                    "--delivery-id",
                    "delivery-cli",
                    "--captured-at",
                    "2026-11-12T07:00:03Z",
                    "--output",
                    str(incident_path),
                ],
                check=False,
                capture_output=True,
                text=True,
                env=env,
            )
            self.assertEqual(0, capture.returncode, capture.stderr)
            self.assertTrue(incident_path.exists())

            validate = subprocess.run(
                [
                    sys.executable,
                    str(TOOLS / "queue_incident.py"),
                    "validate",
                    "--incident",
                    str(incident_path),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, validate.returncode, validate.stderr)

            bad_env = env.copy()
            bad_env["GITHUB_WEBHOOK_SECRET"] = "wrong"
            rejected = subprocess.run(
                [
                    sys.executable,
                    str(TOOLS / "queue_incident.py"),
                    "capture",
                    "--event",
                    str(event_path),
                    "--signature",
                    signature,
                    "--queue-snapshot",
                    str(queue_path),
                    "--object-manifest",
                    str(objects_path),
                    "--delivery-id",
                    "delivery-rejected",
                    "--output",
                    str(root / "rejected.json"),
                ],
                check=False,
                capture_output=True,
                text=True,
                env=bad_env,
            )
            self.assertEqual(2, rejected.returncode)
            self.assertIn("signature verification failed", rejected.stderr)

    def test_event_digest_cannot_diverge_from_initial_lifecycle(self) -> None:
        incident = build_valid_incident()
        tampered = copy.deepcopy(incident)
        tampered["event"]["sha256"] = "b" * 64
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError,
            "initial lifecycle digest does not match",
        ):
            queue_incident.validate_incident(tampered)

    def test_build_and_destroyed_reject_payload_not_parsed_from_raw_body(self) -> None:
        payload = event_payload()
        snapshot = queue_snapshot()
        different_raw = raw_event(
            {**payload, "action": "destroyed", "reason": "dequeued"}
        )
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError,
            "payload does not match raw event body",
        ):
            queue_incident.build_incident(
                event_payload=payload,
                raw_event=different_raw,
                queue_snapshot=snapshot,
                object_manifest=object_manifest(snapshot),
                delivery_id="delivery-mismatch",
                captured_at="2026-11-12T07:00:03Z",
                signature_verified=True,
            )

        incident = build_valid_incident()
        destroyed = event_payload("destroyed", "dequeued")
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError,
            "payload does not match raw event body",
        ):
            queue_incident.append_destroyed(
                incident=incident,
                event_payload=destroyed,
                raw_event=raw_event(event_payload("destroyed", "merged")),
                delivery_id="destroyed-mismatch",
                observed_at="2026-11-12T08:00:00Z",
                signature_verified=True,
            )

    def test_initial_lifecycle_digest_must_match_event_digest(self) -> None:
        incident = build_valid_incident()
        incident["lifecycle"][0]["event_sha256"] = "b" * 64
        with self.assertRaisesRegex(
            queue_incident.QueueIncidentError,
            "initial lifecycle digest does not match",
        ):
            queue_incident.validate_incident(incident)


if __name__ == "__main__":
    unittest.main()
