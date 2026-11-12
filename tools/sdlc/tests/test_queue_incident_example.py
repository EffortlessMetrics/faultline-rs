from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools" / "sdlc"
sys.path.insert(0, str(TOOLS))

import queue_incident  # noqa: E402


class QueueIncidentExampleTests(unittest.TestCase):
    def test_checked_queue_incident_example_is_valid(self) -> None:
        example = json.loads(
            (ROOT / "docs" / "examples" / "queue-incident.v1.json").read_text(
                encoding="utf-8"
            )
        )
        queue_incident.validate_incident(example)


if __name__ == "__main__":
    unittest.main()
