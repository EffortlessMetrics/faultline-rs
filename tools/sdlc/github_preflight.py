"""Pure late-qualification admission checks."""

from __future__ import annotations

from typing import Any, Iterable

from manifest import EvidenceError, is_nonempty_string


def load_policy(value: dict[str, Any]) -> dict[str, Any]:
    if value.get("schema_version") != 1:
        raise EvidenceError("SDLC policy schema_version must be 1")
    for field in ("merge_ready_label", "evidence_check_name"):
        if not is_nonempty_string(value.get(field)):
            raise EvidenceError(f"SDLC policy {field} must be a non-empty string")
    minutes = value.get("max_qualification_minutes")
    if not isinstance(minutes, int) or minutes <= 0:
        raise EvidenceError("SDLC policy max_qualification_minutes must be positive")
    trusted = value.get("trusted_evidence_app_slugs", [])
    if not isinstance(trusted, list) or not all(is_nonempty_string(item) for item in trusted):
        raise EvidenceError("SDLC policy trusted_evidence_app_slugs must be a string list")
    return value


def latest_matching_check(
    checks: Iterable[dict[str, Any]], name: str, sha: str
) -> dict[str, Any] | None:
    matches = [item for item in checks if item.get("name") == name and item.get("head_sha") == sha]
    matches.sort(key=lambda item: str(item.get("completed_at") or item.get("started_at") or ""))
    return matches[-1] if matches else None


def evaluate_preflight(
    *,
    pull: dict[str, Any],
    checks: dict[str, Any],
    threads: list[dict[str, Any]],
    expected_sha: str,
    policy: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    if pull.get("state") != "open":
        errors.append("pull request is not open")
    if pull.get("draft") is True:
        errors.append("pull request is still draft")
    head = pull.get("head")
    actual_sha = head.get("sha") if isinstance(head, dict) else None
    if actual_sha != expected_sha:
        errors.append(f"pull request head moved: expected {expected_sha}, found {actual_sha}")
    labels = pull.get("labels", [])
    label_names = {
        item.get("name")
        for item in labels
        if isinstance(item, dict) and is_nonempty_string(item.get("name"))
    }
    if policy["merge_ready_label"] not in label_names:
        errors.append(f"missing {policy['merge_ready_label']} label")

    check_runs = checks.get("check_runs", [])
    if not isinstance(check_runs, list):
        check_runs = []
    evidence_check = latest_matching_check(
        [item for item in check_runs if isinstance(item, dict)],
        policy["evidence_check_name"],
        expected_sha,
    )
    if evidence_check is None:
        errors.append(f"missing {policy['evidence_check_name']} check for exact head")
    else:
        if evidence_check.get("status") != "completed":
            errors.append(f"{policy['evidence_check_name']} check is not completed")
        if evidence_check.get("conclusion") != "success":
            errors.append(
                f"{policy['evidence_check_name']} conclusion is {evidence_check.get('conclusion')}"
            )
        external_id = evidence_check.get("external_id")
        if not isinstance(external_id, str) or not external_id.startswith(
            "faultline-evidence-v1:"
        ):
            errors.append(f"{policy['evidence_check_name']} lacks a v1 evidence digest")
        trusted = policy.get("trusted_evidence_app_slugs", [])
        if trusted:
            app = evidence_check.get("app")
            slug = app.get("slug") if isinstance(app, dict) else None
            if slug not in trusted:
                errors.append(
                    f"{policy['evidence_check_name']} was published by untrusted app: {slug}"
                )

    if policy.get("block_unresolved_review_threads", True):
        for thread in (item for item in threads if not item.get("isResolved")):
            comments = thread.get("comments", {})
            nodes = comments.get("nodes", []) if isinstance(comments, dict) else []
            first = nodes[0] if isinstance(nodes, list) and nodes else {}
            url = (
                first.get("url", "unknown review thread")
                if isinstance(first, dict)
                else "unknown review thread"
            )
            state = "outdated" if thread.get("isOutdated") else "current"
            errors.append(f"unresolved {state} review thread: {url}")
    return errors
