#!/usr/bin/env python3
"""Publish candidate-bound evidence and preflight late qualification."""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.parse
from typing import Any

from github_api import (
    DEFAULT_API_URL,
    DEFAULT_GRAPHQL_URL,
    evaluate_preflight,
    fetch_review_threads,
    load_policy,
    request_json,
)
from manifest import (
    DEFAULT_CHECK_NAME,
    EvidenceError,
    build_check_payload,
    load_json,
    manifest_digest,
    render_summary,
    readiness_errors,
    validate_manifest,
)


def cmd_validate(args: argparse.Namespace) -> int:
    manifest = load_json(args.manifest)
    errors = readiness_errors(manifest) if args.require_ready else validate_manifest(manifest)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(manifest_digest(manifest))
    return 0


def cmd_summary(args: argparse.Namespace) -> int:
    print(render_summary(load_json(args.manifest)), end="")
    return 0


def cmd_publish(args: argparse.Namespace) -> int:
    manifest = load_json(args.manifest)
    candidate = manifest.get("candidate", {})
    if args.expected_sha and candidate.get("head_sha") != args.expected_sha:
        raise EvidenceError(
            f"manifest head {candidate.get('head_sha')} does not match expected {args.expected_sha}"
        )
    payload = build_check_payload(manifest, name=args.check_name, details_url=args.details_url)
    if args.dry_run:
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0
    token = args.token or os.environ.get("GITHUB_TOKEN")
    if not token:
        raise EvidenceError("publish requires --token or GITHUB_TOKEN")
    repository = candidate["repository"]
    pull = request_json(
        f"{args.api_url.rstrip('/')}/repos/{repository}/pulls/{candidate['pull_request']}",
        token=token,
    )
    head = pull.get("head")
    actual_head = head.get("sha") if isinstance(head, dict) else None
    if actual_head != candidate["head_sha"]:
        raise EvidenceError(
            f"pull request head moved: manifest {candidate['head_sha']}, current {actual_head}"
        )
    commit = request_json(
        f"{args.api_url.rstrip('/')}/repos/{repository}/git/commits/{candidate['head_sha']}",
        token=token,
    )
    tree = commit.get("tree")
    actual_tree = tree.get("sha") if isinstance(tree, dict) else None
    if actual_tree != candidate["tree_sha"]:
        raise EvidenceError(
            f"candidate tree mismatch: manifest {candidate['tree_sha']}, current {actual_tree}"
        )
    response = request_json(
        f"{args.api_url.rstrip('/')}/repos/{repository}/check-runs",
        token=token,
        method="POST",
        body=payload,
    )
    print(response.get("html_url") or response.get("url") or "published")
    return 0


def cmd_preflight(args: argparse.Namespace) -> int:
    policy = load_policy(load_json(args.policy))
    token = args.token or os.environ.get("GITHUB_TOKEN")
    if args.pull_json:
        pull = load_json(args.pull_json)
    else:
        if not token:
            raise EvidenceError("preflight requires --token or GITHUB_TOKEN")
        pull = request_json(
            f"{args.api_url.rstrip('/')}/repos/{args.repository}/pulls/{args.pull_request}",
            token=token,
        )
    if args.checks_json:
        checks = load_json(args.checks_json)
    else:
        if not token:
            raise EvidenceError("preflight requires --token or GITHUB_TOKEN")
        encoded_sha = urllib.parse.quote(args.expected_sha, safe="")
        check_name = urllib.parse.quote(policy["evidence_check_name"], safe="")
        checks = request_json(
            f"{args.api_url.rstrip('/')}/repos/{args.repository}/commits/{encoded_sha}/check-runs"
            f"?check_name={check_name}&filter=latest&per_page=100",
            token=token,
        )
    if args.threads_json:
        thread_value = load_json(args.threads_json)
        threads = thread_value.get("threads", [])
        if not isinstance(threads, list):
            raise EvidenceError("threads fixture must contain a threads list")
    else:
        if not token:
            raise EvidenceError("preflight requires --token or GITHUB_TOKEN")
        threads = fetch_review_threads(
            args.repository,
            args.pull_request,
            token=token,
            graphql_url=args.graphql_url,
        )
    errors = evaluate_preflight(
        pull=pull,
        checks=checks,
        threads=[item for item in threads if isinstance(item, dict)],
        expected_sha=args.expected_sha,
        policy=policy,
    )
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(args.expected_sha)
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    validate = subparsers.add_parser("validate")
    validate.add_argument("--manifest", required=True)
    validate.add_argument("--require-ready", action="store_true")
    validate.set_defaults(func=cmd_validate)
    summary = subparsers.add_parser("summary")
    summary.add_argument("--manifest", required=True)
    summary.set_defaults(func=cmd_summary)
    publish = subparsers.add_parser("publish-check")
    publish.add_argument("--manifest", required=True)
    publish.add_argument("--expected-sha")
    publish.add_argument("--check-name", default=DEFAULT_CHECK_NAME)
    publish.add_argument("--details-url")
    publish.add_argument("--token")
    publish.add_argument("--api-url", default=os.environ.get("GITHUB_API_URL", DEFAULT_API_URL))
    publish.add_argument("--dry-run", action="store_true")
    publish.set_defaults(func=cmd_publish)
    preflight = subparsers.add_parser("preflight")
    preflight.add_argument("--repository", required=True)
    preflight.add_argument("--pull-request", type=int, required=True)
    preflight.add_argument("--expected-sha", required=True)
    preflight.add_argument("--policy", default=".github/sdlc-policy.json")
    preflight.add_argument("--token")
    preflight.add_argument("--api-url", default=os.environ.get("GITHUB_API_URL", DEFAULT_API_URL))
    preflight.add_argument(
        "--graphql-url", default=os.environ.get("GITHUB_GRAPHQL_URL", DEFAULT_GRAPHQL_URL)
    )
    preflight.add_argument("--pull-json")
    preflight.add_argument("--checks-json")
    preflight.add_argument("--threads-json")
    preflight.set_defaults(func=cmd_preflight)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return int(args.func(args))
    except (EvidenceError, OSError, json.JSONDecodeError, ValueError, KeyError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
