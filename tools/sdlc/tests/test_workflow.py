import json
import os
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

from common import HEAD, valid_checks, valid_pull

ROOT = Path(__file__).resolve().parents[3]


class WorkflowContractTests(unittest.TestCase):
    def test_pilot_workflow_is_late_and_bounded(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "late-qualification.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("workflow_dispatch:", workflow)
        self.assertIn("github.event_name == 'workflow_dispatch' ||", workflow)
        self.assertIn("types: [labeled]", workflow)
        self.assertIn("github.event.label.name == 'merge-ready'", workflow)
        self.assertIn("timeout-minutes: 20", workflow)
        self.assertNotIn("\n  push:", workflow)
        self.assertIn("needs.preflight.outputs.head_sha", workflow)

    def test_live_merge_ready_label_on_both_entry_points(self) -> None:
        workflow = (ROOT / ".github/workflows/late-qualification.yml").read_text(encoding="utf-8")
        preflight_job = workflow.split("\n  preflight:\n")[1].split("\n  qualify:\n")[0]
        self.assertIn("PR_NUMBER: ${{ steps.context.outputs.pr_number }}", preflight_job)
        self.assertIn("EXPECTED_SHA: ${{ steps.context.outputs.expected_sha }}", preflight_job)
        # The route fixtures below must match the inputs GitHub passes to the resolver.
        context_step = preflight_job.split("        id: context\n", 1)[1]
        context_env = context_step.split("        env:\n", 1)[1].split("        run: |", 1)[0]
        bindings = {line.strip() for line in context_env.splitlines()}
        for binding in (
            "EVENT_NAME: ${{ github.event_name }}",
            "EVENT_PR_NUMBER: ${{ github.event.pull_request.number }}",
            "EVENT_HEAD_SHA: ${{ github.event.pull_request.head.sha }}",
            "INPUT_PR_NUMBER: ${{ inputs.pr_number }}",
            "INPUT_HEAD_SHA: ${{ inputs.expected_head_sha }}",
        ):
            self.assertIn(binding, bindings)
        # Exercise the two Bash steps used on the Ubuntu runner.
        run_block = r"^        run: \|\n((?:^          .*\n|^\n)+)"
        resolve, verify = [
            textwrap.dedent(block)
            for block in re.findall(run_block, preflight_job, re.MULTILINE)
        ]
        # Supply the CLI's offline inputs and carry the resolver's outputs between steps.
        offline_verify = r"""
            python3() {
                "$PYTHON" -B "$@" --pull-json "$PULL" --checks-json "$CHECKS" \
                    --threads-json "$THREADS"
            }
        """ + verify
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            pull = base / "pull.json"
            checks = base / "checks.json"
            threads = base / "threads.json"
            checks.write_text(json.dumps(valid_checks()), encoding="utf-8")
            threads.write_text('{"threads": []}', encoding="utf-8")
            output = base / "output"
            env = {
                "PATH": os.environ["PATH"],
                "PYTHON": sys.executable,
                "PULL": str(pull),
                "CHECKS": str(checks),
                "THREADS": str(threads),
                "GITHUB_OUTPUT": str(output),
                "GITHUB_STEP_SUMMARY": str(base / "summary"),
                "GITHUB_REPOSITORY": "EffortlessMetrics/faultline-rs",
            }
            for event, prefix in (("workflow_dispatch", "INPUT"), ("pull_request", "EVENT")):
                with self.subTest(event=event):
                    output.write_text("", encoding="utf-8")
                    request = {
                        "EVENT_NAME": event,
                        f"{prefix}_PR_NUMBER": "123",
                        f"{prefix}_HEAD_SHA": HEAD,
                    }
                    subprocess.run(
                        ["bash", "-c", resolve], env=env | request, cwd=ROOT, check=True
                    )
                    resolved = dict(line.split("=", 1) for line in output.read_text().splitlines())
                    self.assertEqual({"pr_number": "123", "expected_sha": HEAD}, resolved)
                    candidate = {
                        "PR_NUMBER": resolved["pr_number"],
                        "EXPECTED_SHA": resolved["expected_sha"],
                    }
                    for labeled in (False, True):
                        with self.subTest(labeled=labeled):
                            state = valid_pull()
                            if not labeled:
                                state["labels"] = []
                            pull.write_text(json.dumps(state), encoding="utf-8")
                            output.write_text("", encoding="utf-8")
                            result = subprocess.run(
                                ["bash", "-c", offline_verify],
                                env=env | candidate,
                                cwd=ROOT,
                                capture_output=True,
                                text=True,
                            )
                            if labeled:
                                self.assertEqual(0, result.returncode, result.stderr)
                                self.assertEqual(f"head_sha={HEAD}", output.read_text().strip())
                            else:
                                self.assertEqual(1, result.returncode, result.stderr)
                                self.assertIn("missing merge-ready label", result.stderr)
                                self.assertEqual("", output.read_text())


if __name__ == "__main__":
    unittest.main()
