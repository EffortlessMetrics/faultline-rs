from __future__ import annotations

HEAD = "a" * 40
TREE = "b" * 40


def valid_manifest() -> dict:
    return {
        "schema_version": 1,
        "candidate": {
            "repository": "EffortlessMetrics/faultline-rs",
            "pull_request": 123,
            "head_sha": HEAD,
            "tree_sha": TREE,
        },
        "producer": {
            "harness": "codex-cloud",
            "model": "gpt-5.6-pro",
            "workflow_version": "sdlc-v1",
        },
        "tested_snapshot": {
            "kind": "git_tree",
            "identifier": TREE,
            "reconciliation": "matched",
        },
        "executions": [
            {
                "id": "focused-tests",
                "command": ["cargo", "test", "-p", "faultline-git"],
                "status": "passed",
                "exit_code": 0,
                "duration_ms": 1250,
                "required": True,
                "establishes": ["focused regression behavior"],
                "evidence_urls": ["https://example.invalid/focused-tests"],
            }
        ],
        "reviews": [
            {
                "id": "correctness-review-1",
                "kind": "correctness",
                "reviewer": "review-tower",
                "candidate_sha": HEAD,
                "result": "approved",
                "independent": True,
                "evidence_url": "https://example.invalid/review",
            }
        ],
        "limitations": ["Does not qualify unrelated release packaging."],
        "qualification": {
            "required_execution_ids": ["focused-tests"],
            "required_review_kinds": ["correctness"],
            "blocking_limitations": [],
        },
    }


def valid_policy() -> dict:
    return {
        "schema_version": 1,
        "merge_ready_label": "merge-ready",
        "evidence_check_name": "Warm evidence",
        "max_qualification_minutes": 20,
        "block_unresolved_review_threads": True,
        "trusted_evidence_app_slugs": [],
    }


def valid_pull() -> dict:
    return {
        "state": "open",
        "draft": False,
        "head": {"sha": HEAD},
        "labels": [{"name": "merge-ready"}],
    }


def valid_checks() -> dict:
    return {
        "check_runs": [
            {
                "name": "Warm evidence",
                "head_sha": HEAD,
                "status": "completed",
                "conclusion": "success",
                "external_id": "faultline-evidence-v1:" + "c" * 64,
                "completed_at": "2026-10-15T00:00:00Z",
                "app": {"slug": "faultline-evidence-publisher"},
            }
        ]
    }
