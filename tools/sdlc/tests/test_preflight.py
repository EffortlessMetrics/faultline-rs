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

import github_api  # noqa: E402
from common import HEAD, valid_checks, valid_policy, valid_pull  # noqa: E402


class PreflightTests(unittest.TestCase):
    def evaluate(self, *, pull=None, checks=None, threads=None, policy=None) -> list[str]:
        return github_api.evaluate_preflight(
            pull=pull or valid_pull(),
            checks=checks or valid_checks(),
            threads=threads or [],
            expected_sha=HEAD,
            policy=policy or valid_policy(),
        )

    def test_ready_state_passes(self) -> None:
        self.assertEqual([], self.evaluate())

    def test_moved_head_fails(self) -> None:
        pull = valid_pull()
        pull["head"]["sha"] = "d" * 40
        self.assertTrue(any("head moved" in item for item in self.evaluate(pull=pull)))

    def test_missing_merge_ready_label_fails(self) -> None:
        pull = valid_pull()
        pull["labels"] = []
        self.assertIn("missing merge-ready label", self.evaluate(pull=pull))

    def test_green_colour_without_evidence_digest_fails(self) -> None:
        checks = valid_checks()
        checks["check_runs"][0]["external_id"] = "not-evidence"
        self.assertIn("Warm evidence lacks a v1 evidence digest", self.evaluate(checks=checks))

    def test_untrusted_publisher_fails_when_policy_is_pinned(self) -> None:
        policy = valid_policy()
        policy["trusted_evidence_app_slugs"] = ["trusted-publisher"]
        self.assertIn(
            "Warm evidence was published by untrusted app: faultline-evidence-publisher",
            self.evaluate(policy=policy),
        )

    def test_unresolved_current_and_outdated_threads_both_fail(self) -> None:
        threads = [
            {
                "isResolved": False,
                "isOutdated": False,
                "comments": {"nodes": [{"url": "https://example.invalid/current"}]},
            },
            {
                "isResolved": False,
                "isOutdated": True,
                "comments": {"nodes": [{"url": "https://example.invalid/outdated"}]},
            },
        ]
        errors = self.evaluate(threads=threads)
        self.assertIn("unresolved current review thread: https://example.invalid/current", errors)
        self.assertIn("unresolved outdated review thread: https://example.invalid/outdated", errors)

    def test_offline_preflight_command(self) -> None:
        command = TOOLS / "evidence.py"
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            fixtures = {
                "policy.json": valid_policy(),
                "pull.json": valid_pull(),
                "checks.json": valid_checks(),
                "threads.json": {"threads": []},
            }
            for name, value in fixtures.items():
                (base / name).write_text(json.dumps(value), encoding="utf-8")
            result = subprocess.run(
                [
                    sys.executable,
                    str(command),
                    "preflight",
                    "--repository",
                    "EffortlessMetrics/faultline-rs",
                    "--pull-request",
                    "123",
                    "--expected-sha",
                    HEAD,
                    "--policy",
                    str(base / "policy.json"),
                    "--pull-json",
                    str(base / "pull.json"),
                    "--checks-json",
                    str(base / "checks.json"),
                    "--threads-json",
                    str(base / "threads.json"),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, result.returncode, result.stderr)
            self.assertEqual(HEAD, result.stdout.strip())


if __name__ == "__main__":
    unittest.main()
