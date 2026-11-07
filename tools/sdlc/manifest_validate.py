"""Evidence-manifest structural and readiness validation."""

from __future__ import annotations

from typing import Any

from manifest_base import (
    RECONCILIATION_STATES,
    SCHEMA_VERSION,
    expect_sha,
    expect_string_list,
    is_nonempty_string,
)
from manifest_evidence import validate_executions, validate_qualification, validate_reviews


def validate_manifest(manifest: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if manifest.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"schema_version must be {SCHEMA_VERSION}")

    candidate = manifest.get("candidate")
    if not isinstance(candidate, dict):
        errors.append("candidate must be an object")
        candidate = {}
    if not is_nonempty_string(candidate.get("repository")):
        errors.append("candidate.repository must be a non-empty owner/name string")
    expect_sha(errors, "candidate.head_sha", candidate.get("head_sha"))
    expect_sha(errors, "candidate.tree_sha", candidate.get("tree_sha"))
    if not isinstance(candidate.get("pull_request"), int) or candidate.get("pull_request", 0) <= 0:
        errors.append("candidate.pull_request must be a positive integer")

    producer = manifest.get("producer")
    if not isinstance(producer, dict):
        errors.append("producer must be an object")
    else:
        for field in ("harness", "model", "workflow_version"):
            if not is_nonempty_string(producer.get(field)):
                errors.append(f"producer.{field} must be a non-empty string")

    snapshot = manifest.get("tested_snapshot")
    if not isinstance(snapshot, dict):
        errors.append("tested_snapshot must be an object")
        snapshot = {}
    if snapshot.get("kind") not in {"git_tree", "working_tree_archive"}:
        errors.append("tested_snapshot.kind must be git_tree or working_tree_archive")
    if not is_nonempty_string(snapshot.get("identifier")):
        errors.append("tested_snapshot.identifier must be a non-empty string")
    if snapshot.get("reconciliation") not in RECONCILIATION_STATES:
        errors.append("tested_snapshot.reconciliation must be matched, reusable_with_limits, or stale")
    if snapshot.get("reconciliation") == "matched":
        identifier = snapshot.get("identifier")
        tree_sha = candidate.get("tree_sha")
        if isinstance(identifier, str) and isinstance(tree_sha, str) and identifier != tree_sha:
            errors.append("matched tested_snapshot.identifier must equal candidate.tree_sha")

    execution_ids = validate_executions(manifest.get("executions"), errors)
    validate_reviews(manifest.get("reviews"), candidate.get("head_sha"), errors)
    expect_string_list(errors, "limitations", manifest.get("limitations"))
    validate_qualification(manifest.get("qualification"), execution_ids, errors)
    _reconcile_required_executions(manifest, errors)
    return errors


def _reconcile_required_executions(manifest: dict[str, Any], errors: list[str]) -> None:
    """Require the two required-execution representations to agree.

    Executions carry a ``required`` boolean and qualification carries
    ``required_execution_ids``. Either representation alone deciding
    readiness is fail-open: a required execution omitted from the ID list
    (or a listed ID whose execution is not marked required) must be a
    structural error, not a silent success.
    """
    executions = manifest.get("executions")
    qualification = manifest.get("qualification")
    if not isinstance(executions, list) or not isinstance(qualification, dict):
        return
    flagged: set[str] = set()
    usable = True
    for execution in executions:
        if not isinstance(execution, dict):
            usable = False
            continue
        execution_id = execution.get("id")
        required = execution.get("required")
        if not isinstance(execution_id, str) or not execution_id.strip():
            usable = False
            continue
        if not isinstance(required, bool):
            usable = False
            continue
        if required:
            flagged.add(execution_id)
    required_ids = qualification.get("required_execution_ids")
    if not isinstance(required_ids, list) or not all(
        is_nonempty_string(item) for item in required_ids
    ):
        return
    if not usable:
        return
    listed = set(required_ids)
    if flagged != listed:
        missing = sorted(flagged - listed)
        extra = sorted(listed - flagged)
        errors.append(
            "qualification.required_execution_ids must match executions marked "
            f"required (missing: {missing}, extra: {extra})"
        )


def readiness_errors(manifest: dict[str, Any]) -> list[str]:
    errors = validate_manifest(manifest)
    if errors:
        return errors
    if manifest["tested_snapshot"]["reconciliation"] != "matched":
        errors.append("tested snapshot is not reconciled exactly to the candidate tree")
    executions = {item["id"]: item for item in manifest["executions"]}
    required_ids = set(manifest["qualification"]["required_execution_ids"])
    for execution_id, execution in executions.items():
        if execution.get("required") or execution_id in required_ids:
            if execution["status"] != "passed":
                errors.append(f"required execution {execution_id} is {execution['status']}")
    reviews = manifest["reviews"]
    if any(item["result"] == "failed_to_review" for item in reviews):
        errors.append("a substantive reviewer failed to review the candidate")
    for kind in manifest["qualification"]["required_review_kinds"]:
        if not any(
            item["kind"] == kind and item["result"] == "approved" and item["independent"]
            for item in reviews
        ):
            errors.append(f"missing independent approved review for kind: {kind}")
    for limitation in manifest["qualification"]["blocking_limitations"]:
        errors.append(f"blocking limitation: {limitation}")
    return errors
