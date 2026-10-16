"""Shared evidence-manifest constants and primitives."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
from typing import Any

SCHEMA_VERSION = 1
DEFAULT_CHECK_NAME = "Warm evidence"
SHA_RE = re.compile(r"^[0-9a-f]{40}$")
EXECUTION_STATUSES = {"passed", "failed", "skipped", "error"}
REVIEW_RESULTS = {"approved", "findings", "failed_to_review"}
RECONCILIATION_STATES = {"matched", "reusable_with_limits", "stale"}


class EvidenceError(RuntimeError):
    """Raised when evidence or GitHub state violates the contract."""


def load_json(path: str | Path) -> dict[str, Any]:
    with Path(path).open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise EvidenceError(f"{path}: expected a JSON object")
    return value


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode(
        "utf-8"
    )


def manifest_digest(manifest: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_json(manifest)).hexdigest()


def is_nonempty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def expect_sha(errors: list[str], field: str, value: Any) -> None:
    if not isinstance(value, str) or not SHA_RE.fullmatch(value):
        errors.append(f"{field} must be a lowercase 40-character Git SHA")


def expect_string_list(errors: list[str], field: str, value: Any) -> None:
    if not isinstance(value, list) or not all(is_nonempty_string(item) for item in value):
        errors.append(f"{field} must be a list of non-empty strings")
