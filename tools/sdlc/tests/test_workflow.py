from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]


class WorkflowContractTests(unittest.TestCase):
    def test_pilot_workflow_is_late_and_bounded(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "late-qualification.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("types: [labeled]", workflow)
        self.assertIn("github.event.label.name == 'merge-ready'", workflow)
        self.assertIn("timeout-minutes: 20", workflow)
        self.assertNotIn("\n  push:", workflow)
        self.assertIn("needs.preflight.outputs.head_sha", workflow)


if __name__ == "__main__":
    unittest.main()
