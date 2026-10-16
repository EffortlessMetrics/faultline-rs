"""Execution, review and qualification validation."""

from __future__ import annotations

from typing import Any

from manifest_base import (
    EXECUTION_STATUSES,
    REVIEW_RESULTS,
    expect_sha,
    expect_string_list,
    is_nonempty_string,
)


def validate_executions(value: Any, errors: list[str]) -> set[str]:
    execution_ids: set[str] = set()
    if not isinstance(value, list) or not value:
        errors.append("executions must be a non-empty list")
        return execution_ids
    for index, execution in enumerate(value):
        prefix = f"executions[{index}]"
        if not isinstance(execution, dict):
            errors.append(f"{prefix} must be an object")
            continue
        execution_id = execution.get("id")
        if not is_nonempty_string(execution_id):
            errors.append(f"{prefix}.id must be a non-empty string")
        elif execution_id in execution_ids:
            errors.append(f"duplicate execution id: {execution_id}")
        else:
            execution_ids.add(execution_id)
        command = execution.get("command")
        if not isinstance(command, list) or not command or not all(
            is_nonempty_string(part) for part in command
        ):
            errors.append(f"{prefix}.command must be a non-empty argv list")
        status = execution.get("status")
        if status not in EXECUTION_STATUSES:
            errors.append(f"{prefix}.status is invalid")
        exit_code = execution.get("exit_code")
        if exit_code is not None and not isinstance(exit_code, int):
            errors.append(f"{prefix}.exit_code must be an integer or null")
        if status == "passed" and exit_code != 0:
            errors.append(f"{prefix}: passed execution must have exit_code 0")
        duration_ms = execution.get("duration_ms")
        if not isinstance(duration_ms, int) or duration_ms < 0:
            errors.append(f"{prefix}.duration_ms must be a non-negative integer")
        if not isinstance(execution.get("required"), bool):
            errors.append(f"{prefix}.required must be boolean")
        expect_string_list(errors, f"{prefix}.establishes", execution.get("establishes"))
        evidence_urls = execution.get("evidence_urls", [])
        if not isinstance(evidence_urls, list) or not all(
            is_nonempty_string(item) for item in evidence_urls
        ):
            errors.append(f"{prefix}.evidence_urls must be a list of non-empty strings")
    return execution_ids


def validate_reviews(value: Any, candidate_sha: Any, errors: list[str]) -> None:
    review_ids: set[str] = set()
    if not isinstance(value, list) or not value:
        errors.append("reviews must be a non-empty list")
        return
    for index, review in enumerate(value):
        prefix = f"reviews[{index}]"
        if not isinstance(review, dict):
            errors.append(f"{prefix} must be an object")
            continue
        review_id = review.get("id")
        if not is_nonempty_string(review_id):
            errors.append(f"{prefix}.id must be a non-empty string")
        elif review_id in review_ids:
            errors.append(f"duplicate review id: {review_id}")
        else:
            review_ids.add(review_id)
        for field in ("kind", "reviewer"):
            if not is_nonempty_string(review.get(field)):
                errors.append(f"{prefix}.{field} must be a non-empty string")
        if review.get("result") not in REVIEW_RESULTS:
            errors.append(f"{prefix}.result is invalid")
        if not isinstance(review.get("independent"), bool):
            errors.append(f"{prefix}.independent must be boolean")
        expect_sha(errors, f"{prefix}.candidate_sha", review.get("candidate_sha"))
        if review.get("candidate_sha") != candidate_sha:
            errors.append(f"{prefix}.candidate_sha must equal candidate.head_sha")
        if not is_nonempty_string(review.get("evidence_url")):
            errors.append(f"{prefix}.evidence_url must be a non-empty string")


def validate_qualification(value: Any, execution_ids: set[str], errors: list[str]) -> None:
    if not isinstance(value, dict):
        errors.append("qualification must be an object")
        value = {}
    required = value.get("required_execution_ids")
    expect_string_list(errors, "qualification.required_execution_ids", required)
    if isinstance(required, list):
        for execution_id in required:
            if execution_id not in execution_ids:
                errors.append(f"qualification requires unknown execution id: {execution_id}")
    expect_string_list(errors, "qualification.required_review_kinds", value.get("required_review_kinds"))
    blocking = value.get("blocking_limitations")
    if not isinstance(blocking, list) or not all(is_nonempty_string(item) for item in blocking):
        errors.append("qualification.blocking_limitations must be a list of non-empty strings")
