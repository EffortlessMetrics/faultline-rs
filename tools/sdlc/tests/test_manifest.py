from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools" / "sdlc"
sys.path.insert(0, str(TOOLS))

import manifest  # noqa: E402
from common import HEAD, valid_manifest  # noqa: E402


class ManifestTests(unittest.TestCase):
    def test_valid_manifest_is_ready(self) -> None:
        value = valid_manifest()
        self.assertEqual([], manifest.validate_manifest(value))
        self.assertEqual([], manifest.readiness_errors(value))

    def test_snapshot_must_match_candidate_tree(self) -> None:
        value = valid_manifest()
        value["tested_snapshot"]["identifier"] = "c" * 40
        self.assertIn(
            "matched tested_snapshot.identifier must equal candidate.tree_sha",
            manifest.validate_manifest(value),
        )

    def test_stale_snapshot_blocks_qualification(self) -> None:
        value = valid_manifest()
        value["tested_snapshot"]["reconciliation"] = "stale"
        self.assertTrue(
            any("not reconciled exactly" in item for item in manifest.readiness_errors(value))
        )

    def test_required_execution_failure_blocks_qualification(self) -> None:
        value = valid_manifest()
        value["executions"][0]["status"] = "failed"
        value["executions"][0]["exit_code"] = 1
        self.assertIn(
            "required execution focused-tests is failed", manifest.readiness_errors(value)
        )

    def test_failed_to_review_cannot_publish_success(self) -> None:
        value = valid_manifest()
        value["reviews"][0]["result"] = "failed_to_review"
        payload = manifest.build_check_payload(value)
        self.assertEqual("failure", payload["conclusion"])
        self.assertIn("failed to review", payload["output"]["summary"])

    def test_check_payload_is_candidate_bound_and_digest_backed(self) -> None:
        value = valid_manifest()
        payload = manifest.build_check_payload(value)
        self.assertEqual(HEAD, payload["head_sha"])
        self.assertEqual("success", payload["conclusion"])
        self.assertEqual(
            f"faultline-evidence-v1:{manifest.manifest_digest(value)}",
            payload["external_id"],
        )

    def test_structurally_invalid_manifest_cannot_be_published(self) -> None:
        value = valid_manifest()
        value["candidate"]["head_sha"] = "not-a-sha"
        with self.assertRaises(manifest.EvidenceError):
            manifest.build_check_payload(value)

    def test_command_line_validate_and_summary(self) -> None:
        command = TOOLS / "evidence.py"
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "evidence.json"
            path.write_text(json.dumps(valid_manifest()), encoding="utf-8")
            validate = subprocess.run(
                [sys.executable, str(command), "validate", "--manifest", str(path), "--require-ready"],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, validate.returncode, validate.stderr)
            self.assertEqual(64, len(validate.stdout.strip()))
            summary = subprocess.run(
                [sys.executable, str(command), "summary", "--manifest", str(path)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, summary.returncode, summary.stderr)
            self.assertIn("Qualification readiness: **ready**", summary.stdout)

    def test_example_and_schema_are_valid_json(self) -> None:
        json.loads((ROOT / "docs" / "examples" / "sdlc-evidence.v1.json").read_text())
        json.loads((ROOT / "schemas" / "sdlc" / "evidence-v1.schema.json").read_text())


if __name__ == "__main__":
    unittest.main()
