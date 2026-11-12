from __future__ import annotations

import argparse
import copy
import hashlib
import hmac
import json
import os
import re
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any

SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")
DIGEST_PATTERN = re.compile(r"^[0-9a-f]{64}$")
DESTROY_REASONS = {"merged", "invalidated", "dequeued"}
MERGE_METHODS = {"MERGE", "SQUASH", "REBASE"}
GROUPING_STRATEGIES = {"ALLGREEN", "HEADGREEN"}


class QueueIncidentError(RuntimeError):
    pass


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def sha256_hex(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def now_rfc3339() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def require_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise QueueIncidentError(f"{field} must be a non-empty string")
    return value


def require_bool(value: Any, field: str) -> bool:
    if not isinstance(value, bool):
        raise QueueIncidentError(f"{field} must be a boolean")
    return value


def require_positive_int(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise QueueIncidentError(f"{field} must be a positive integer")
    return value


def require_non_negative_int(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise QueueIncidentError(f"{field} must be a non-negative integer")
    return value


def require_sha(value: Any, field: str) -> str:
    text = require_string(value, field)
    if not SHA_PATTERN.fullmatch(text):
        raise QueueIncidentError(f"{field} must be a lowercase 40-character Git SHA")
    return text


def require_digest(value: Any, field: str) -> str:
    text = require_string(value, field)
    if not DIGEST_PATTERN.fullmatch(text):
        raise QueueIncidentError(f"{field} must be a lowercase SHA-256 digest")
    return text


def require_timestamp(value: Any, field: str) -> str:
    text = require_string(value, field)
    try:
        parsed = datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError as error:
        raise QueueIncidentError(f"{field} must be an RFC 3339 timestamp") from error
    if parsed.tzinfo is None:
        raise QueueIncidentError(f"{field} must include a timezone")
    return text


def require_repository(value: Any, field: str) -> str:
    text = require_string(value, field)
    owner, separator, name = text.partition("/")
    if separator != "/" or not owner or not name or "/" in name:
        raise QueueIncidentError(f"{field} must be in owner/name form")
    return text


def require_object(value: Any, field: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise QueueIncidentError(f"{field} must be an object")
    return value


def require_list(value: Any, field: str) -> list[Any]:
    if not isinstance(value, list):
        raise QueueIncidentError(f"{field} must be an array")
    return value


def require_delivery_id(value: Any, field: str = "delivery_id") -> str:
    delivery_id = require_string(value, field)
    if len(delivery_id) > 128:
        raise QueueIncidentError(f"{field} must be at most 128 characters")
    return delivery_id


def verify_webhook_signature(secret: bytes, body: bytes, signature: str) -> None:
    if not secret:
        raise QueueIncidentError("webhook secret must not be empty")
    if not isinstance(body, bytes):
        raise QueueIncidentError("webhook body must be bytes")
    if not isinstance(signature, str) or not signature.startswith("sha256="):
        raise QueueIncidentError("webhook signature must use the sha256= form")
    supplied = signature.removeprefix("sha256=")
    if not DIGEST_PATTERN.fullmatch(supplied):
        raise QueueIncidentError("webhook signature must contain a lowercase SHA-256 digest")
    expected = hmac.new(secret, body, hashlib.sha256).hexdigest()
    if not hmac.compare_digest(expected, supplied):
        raise QueueIncidentError("webhook signature verification failed")


def load_json_bytes(data: bytes, field: str) -> dict[str, Any]:
    try:
        value = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise QueueIncidentError(f"{field} must contain UTF-8 JSON") from error
    return require_object(value, field)


def load_json(path: Path, field: str) -> dict[str, Any]:
    try:
        return load_json_bytes(path.read_bytes(), field)
    except OSError as error:
        raise QueueIncidentError(f"could not read {field} from {path}: {error}") from error


def normalize_merge_group(payload: dict[str, Any], expected_action: str) -> dict[str, Any]:
    action = require_string(payload.get("action"), "event.action")
    if action != expected_action:
        raise QueueIncidentError(
            f"event.action must be {expected_action!r}, found {action!r}"
        )

    repository = require_object(payload.get("repository"), "event.repository")
    repository_name = require_repository(
        repository.get("full_name"), "event.repository.full_name"
    )
    merge_group = require_object(payload.get("merge_group"), "event.merge_group")
    head_commit = require_object(
        merge_group.get("head_commit"), "event.merge_group.head_commit"
    )

    normalized = {
        "repository": repository_name,
        "base_ref": require_string(
            merge_group.get("base_ref"), "event.merge_group.base_ref"
        ),
        "base_sha": require_sha(
            merge_group.get("base_sha"), "event.merge_group.base_sha"
        ),
        "head_ref": require_string(
            merge_group.get("head_ref"), "event.merge_group.head_ref"
        ),
        "head_sha": require_sha(
            merge_group.get("head_sha"), "event.merge_group.head_sha"
        ),
        "head_tree_sha": require_sha(
            head_commit.get("tree_id"), "event.merge_group.head_commit.tree_id"
        ),
        "expanded_head_commit_id": require_sha(
            head_commit.get("id"), "event.merge_group.head_commit.id"
        ),
    }
    return normalized


def normalize_checks_requested(payload: dict[str, Any]) -> dict[str, Any]:
    return normalize_merge_group(payload, "checks_requested")


def normalize_destroyed(payload: dict[str, Any]) -> dict[str, Any]:
    normalized = normalize_merge_group(payload, "destroyed")
    reason = require_string(payload.get("reason"), "event.reason")
    if reason not in DESTROY_REASONS:
        raise QueueIncidentError(
            "event.reason must be merged, invalidated, or dequeued"
        )
    normalized["reason"] = reason
    return normalized


def normalize_queue_snapshot(
    snapshot: dict[str, Any], merge_group: dict[str, Any]
) -> dict[str, Any]:
    if snapshot.get("schema_version") != 1:
        raise QueueIncidentError("queue_snapshot.schema_version must be 1")
    if require_string(snapshot.get("provider"), "queue_snapshot.provider") != "github":
        raise QueueIncidentError("queue_snapshot.provider must be 'github'")

    repository = require_repository(
        snapshot.get("repository"), "queue_snapshot.repository"
    )
    if repository != merge_group["repository"]:
        raise QueueIncidentError("queue snapshot repository does not match event")

    base_sha = require_sha(snapshot.get("base_sha"), "queue_snapshot.base_sha")
    head_sha = require_sha(snapshot.get("head_sha"), "queue_snapshot.head_sha")
    if base_sha != merge_group["base_sha"]:
        raise QueueIncidentError("queue snapshot base SHA does not match event")
    if head_sha != merge_group["head_sha"]:
        raise QueueIncidentError("queue snapshot head SHA does not match event")

    merge_method = require_string(
        snapshot.get("merge_method"), "queue_snapshot.merge_method"
    )
    if merge_method not in MERGE_METHODS:
        raise QueueIncidentError("queue_snapshot.merge_method is unsupported")
    grouping_strategy = require_string(
        snapshot.get("grouping_strategy"), "queue_snapshot.grouping_strategy"
    )
    if grouping_strategy not in GROUPING_STRATEGIES:
        raise QueueIncidentError("queue_snapshot.grouping_strategy is unsupported")

    entries = require_list(snapshot.get("entries"), "queue_snapshot.entries")
    if not entries:
        raise QueueIncidentError("queue_snapshot.entries must not be empty")

    normalized_entries: list[dict[str, Any]] = []
    seen_entry_ids: set[str] = set()
    seen_pull_requests: set[int] = set()
    previous_head = base_sha

    for expected_position, raw_entry in enumerate(entries, start=1):
        entry = require_object(
            raw_entry, f"queue_snapshot.entries[{expected_position - 1}]"
        )
        position = require_positive_int(
            entry.get("position"), f"queue_snapshot.entries[{expected_position - 1}].position"
        )
        if position != expected_position:
            raise QueueIncidentError(
                "queue snapshot positions must be contiguous and begin at 1"
            )
        entry_id = require_string(
            entry.get("queue_entry_id"),
            f"queue_snapshot.entries[{expected_position - 1}].queue_entry_id",
        )
        if entry_id in seen_entry_ids:
            raise QueueIncidentError(f"duplicate queue entry id: {entry_id}")
        seen_entry_ids.add(entry_id)

        pull_request = require_positive_int(
            entry.get("pull_request"),
            f"queue_snapshot.entries[{expected_position - 1}].pull_request",
        )
        if pull_request in seen_pull_requests:
            raise QueueIncidentError(f"duplicate pull request in queue group: {pull_request}")
        seen_pull_requests.add(pull_request)

        integration_base_sha = require_sha(
            entry.get("integration_base_sha"),
            f"queue_snapshot.entries[{expected_position - 1}].integration_base_sha",
        )
        integration_head_sha = require_sha(
            entry.get("integration_head_sha"),
            f"queue_snapshot.entries[{expected_position - 1}].integration_head_sha",
        )
        if integration_base_sha != previous_head:
            raise QueueIncidentError(
                f"queue entry {position} does not continue the cumulative integration chain"
            )

        normalized_entry = {
            "position": position,
            "queue_entry_id": entry_id,
            "pull_request": pull_request,
            "pull_request_head_sha": require_sha(
                entry.get("pull_request_head_sha"),
                f"queue_snapshot.entries[{expected_position - 1}].pull_request_head_sha",
            ),
            "integration_base_sha": integration_base_sha,
            "integration_head_sha": integration_head_sha,
            "state": require_string(
                entry.get("state"),
                f"queue_snapshot.entries[{expected_position - 1}].state",
            ),
            "jump": require_bool(
                entry.get("jump"),
                f"queue_snapshot.entries[{expected_position - 1}].jump",
            ),
            "solo": require_bool(
                entry.get("solo"),
                f"queue_snapshot.entries[{expected_position - 1}].solo",
            ),
        }
        normalized_entries.append(normalized_entry)
        previous_head = integration_head_sha

    if previous_head != head_sha:
        raise QueueIncidentError(
            "final queue entry integration head does not match merge-group head"
        )

    return {
        "schema_version": 1,
        "provider": "github",
        "repository": repository,
        "captured_at": require_timestamp(
            snapshot.get("captured_at"), "queue_snapshot.captured_at"
        ),
        "base_sha": base_sha,
        "head_sha": head_sha,
        "merge_method": merge_method,
        "grouping_strategy": grouping_strategy,
        "build_concurrency": require_positive_int(
            snapshot.get("build_concurrency"), "queue_snapshot.build_concurrency"
        ),
        "maximum_entries_to_build": require_positive_int(
            snapshot.get("maximum_entries_to_build"),
            "queue_snapshot.maximum_entries_to_build",
        ),
        "entries": normalized_entries,
    }


def normalize_object_manifest(
    manifest: dict[str, Any],
    merge_group: dict[str, Any],
    queue_snapshot: dict[str, Any],
) -> dict[str, Any]:
    if manifest.get("schema_version") != 1:
        raise QueueIncidentError("object_manifest.schema_version must be 1")
    repository = require_repository(
        manifest.get("repository"), "object_manifest.repository"
    )
    if repository != merge_group["repository"]:
        raise QueueIncidentError("object manifest repository does not match event")

    head_sha = require_sha(manifest.get("head_sha"), "object_manifest.head_sha")
    head_tree_sha = require_sha(
        manifest.get("head_tree_sha"), "object_manifest.head_tree_sha"
    )
    if head_sha != merge_group["head_sha"]:
        raise QueueIncidentError("object manifest head SHA does not match event")
    if head_tree_sha != merge_group["head_tree_sha"]:
        raise QueueIncidentError("object manifest head tree does not match event")

    bundle = require_object(manifest.get("bundle"), "object_manifest.bundle")
    bundle_path = require_string(bundle.get("path"), "object_manifest.bundle.path")
    parsed_path = PurePosixPath(bundle_path)
    if parsed_path.is_absolute() or ".." in parsed_path.parts:
        raise QueueIncidentError(
            "object_manifest.bundle.path must be a safe relative path"
        )

    verified_commits = [
        require_sha(value, f"object_manifest.verified_commits[{index}]")
        for index, value in enumerate(
            require_list(
                manifest.get("verified_commits"),
                "object_manifest.verified_commits",
            )
        )
    ]
    if len(verified_commits) != len(set(verified_commits)):
        raise QueueIncidentError("object_manifest.verified_commits contains duplicates")

    required_commits = {merge_group["base_sha"], merge_group["head_sha"]}
    for entry in queue_snapshot["entries"]:
        required_commits.add(entry["pull_request_head_sha"])
        required_commits.add(entry["integration_base_sha"])
        required_commits.add(entry["integration_head_sha"])
    missing = sorted(required_commits.difference(verified_commits))
    if missing:
        raise QueueIncidentError(
            "object manifest is missing required commits: " + ", ".join(missing)
        )

    verification = require_object(
        manifest.get("verification"), "object_manifest.verification"
    )
    command = [
        require_string(value, f"object_manifest.verification.command[{index}]")
        for index, value in enumerate(
            require_list(
                verification.get("command"),
                "object_manifest.verification.command",
            )
        )
    ]
    if not command:
        raise QueueIncidentError("object_manifest.verification.command must not be empty")
    if require_string(
        verification.get("result"), "object_manifest.verification.result"
    ) != "passed":
        raise QueueIncidentError("object manifest verification must have passed")

    return {
        "schema_version": 1,
        "repository": repository,
        "captured_at": require_timestamp(
            manifest.get("captured_at"), "object_manifest.captured_at"
        ),
        "head_sha": head_sha,
        "head_tree_sha": head_tree_sha,
        "bundle": {
            "path": bundle_path,
            "size": require_non_negative_int(
                bundle.get("size"), "object_manifest.bundle.size"
            ),
            "sha256": require_digest(
                bundle.get("sha256"), "object_manifest.bundle.sha256"
            ),
        },
        "verified_commits": verified_commits,
        "verification": {
            "command": command,
            "completed_at": require_timestamp(
                verification.get("completed_at"),
                "object_manifest.verification.completed_at",
            ),
            "result": "passed",
        },
    }


def incident_identity(
    repository: str,
    base_sha: str,
    head_sha: str,
    queue_snapshot_sha256: str,
) -> str:
    identity = {
        "repository": repository,
        "base_sha": base_sha,
        "head_sha": head_sha,
        "queue_snapshot_sha256": queue_snapshot_sha256,
    }
    return "mq-" + sha256_hex(canonical_json_bytes(identity))


def build_incident(
    *,
    event_payload: dict[str, Any],
    raw_event: bytes,
    queue_snapshot: dict[str, Any],
    object_manifest: dict[str, Any],
    delivery_id: str,
    captured_at: str,
    signature_verified: bool,
) -> dict[str, Any]:
    if not signature_verified:
        raise QueueIncidentError("authenticated webhook delivery is required")
    merge_group = normalize_checks_requested(event_payload)
    normalized_queue = normalize_queue_snapshot(queue_snapshot, merge_group)
    normalized_objects = normalize_object_manifest(
        object_manifest, merge_group, normalized_queue
    )
    delivery = require_delivery_id(delivery_id)
    observed_at = require_timestamp(captured_at, "captured_at")

    event_digest = sha256_hex(raw_event)
    queue_digest = sha256_hex(canonical_json_bytes(normalized_queue))
    object_digest = sha256_hex(canonical_json_bytes(normalized_objects))
    limitations: list[str] = []
    if merge_group["expanded_head_commit_id"] != merge_group["head_sha"]:
        limitations.append(
            "merge_group.head_commit.id differed from merge_group.head_sha; head_sha is authoritative"
        )

    incident = {
        "schema_version": 1,
        "incident_id": incident_identity(
            merge_group["repository"],
            merge_group["base_sha"],
            merge_group["head_sha"],
            queue_digest,
        ),
        "repository": merge_group["repository"],
        "captured_at": observed_at,
        "authentication": {
            "algorithm": "hmac-sha256",
            "verified": True,
        },
        "merge_group": merge_group,
        "event": {
            "delivery_id": delivery,
            "sha256": event_digest,
        },
        "queue_snapshot": normalized_queue,
        "queue_snapshot_sha256": queue_digest,
        "object_manifest": normalized_objects,
        "object_manifest_sha256": object_digest,
        "lifecycle": [
            {
                "delivery_id": delivery,
                "action": "checks_requested",
                "observed_at": observed_at,
                "event_sha256": event_digest,
                "reason": None,
            }
        ],
        "status": "captured",
        "limitations": limitations,
    }
    validate_incident(incident)
    return incident


def validate_incident(incident: dict[str, Any]) -> None:
    if incident.get("schema_version") != 1:
        raise QueueIncidentError("incident.schema_version must be 1")
    repository = require_repository(incident.get("repository"), "incident.repository")
    require_timestamp(incident.get("captured_at"), "incident.captured_at")

    authentication = require_object(
        incident.get("authentication"), "incident.authentication"
    )
    if authentication.get("algorithm") != "hmac-sha256":
        raise QueueIncidentError("incident authentication algorithm is unsupported")
    if authentication.get("verified") is not True:
        raise QueueIncidentError("incident authentication must be verified")

    merge_group = require_object(incident.get("merge_group"), "incident.merge_group")
    for field in (
        "repository",
        "base_ref",
        "base_sha",
        "head_ref",
        "head_sha",
        "head_tree_sha",
        "expanded_head_commit_id",
    ):
        if field.endswith("_sha") or field == "expanded_head_commit_id":
            require_sha(merge_group.get(field), f"incident.merge_group.{field}")
        else:
            require_string(merge_group.get(field), f"incident.merge_group.{field}")
    if merge_group["repository"] != repository:
        raise QueueIncidentError("incident repository does not match merge group")

    normalized_queue = normalize_queue_snapshot(
        require_object(incident.get("queue_snapshot"), "incident.queue_snapshot"),
        merge_group,
    )
    normalized_objects = normalize_object_manifest(
        require_object(incident.get("object_manifest"), "incident.object_manifest"),
        merge_group,
        normalized_queue,
    )
    queue_digest = sha256_hex(canonical_json_bytes(normalized_queue))
    object_digest = sha256_hex(canonical_json_bytes(normalized_objects))
    if require_digest(
        incident.get("queue_snapshot_sha256"), "incident.queue_snapshot_sha256"
    ) != queue_digest:
        raise QueueIncidentError("incident queue snapshot digest does not match content")
    if require_digest(
        incident.get("object_manifest_sha256"), "incident.object_manifest_sha256"
    ) != object_digest:
        raise QueueIncidentError("incident object manifest digest does not match content")

    expected_id = incident_identity(
        repository,
        merge_group["base_sha"],
        merge_group["head_sha"],
        queue_digest,
    )
    if require_string(incident.get("incident_id"), "incident.incident_id") != expected_id:
        raise QueueIncidentError("incident identity does not match immutable inputs")

    event = require_object(incident.get("event"), "incident.event")
    first_delivery = require_delivery_id(event.get("delivery_id"), "incident.event.delivery_id")
    require_digest(event.get("sha256"), "incident.event.sha256")

    lifecycle = require_list(incident.get("lifecycle"), "incident.lifecycle")
    if not lifecycle:
        raise QueueIncidentError("incident.lifecycle must not be empty")
    seen_deliveries: set[str] = set()
    for index, raw_entry in enumerate(lifecycle):
        entry = require_object(raw_entry, f"incident.lifecycle[{index}]")
        delivery = require_delivery_id(
            entry.get("delivery_id"), f"incident.lifecycle[{index}].delivery_id"
        )
        if delivery in seen_deliveries:
            raise QueueIncidentError(f"duplicate lifecycle delivery id: {delivery}")
        seen_deliveries.add(delivery)
        action = require_string(
            entry.get("action"), f"incident.lifecycle[{index}].action"
        )
        if action not in {"checks_requested", "destroyed"}:
            raise QueueIncidentError(f"unsupported lifecycle action: {action}")
        require_timestamp(
            entry.get("observed_at"), f"incident.lifecycle[{index}].observed_at"
        )
        require_digest(
            entry.get("event_sha256"),
            f"incident.lifecycle[{index}].event_sha256",
        )
        reason = entry.get("reason")
        if action == "checks_requested":
            if reason is not None:
                raise QueueIncidentError("checks_requested lifecycle reason must be null")
        elif reason not in DESTROY_REASONS:
            raise QueueIncidentError("destroyed lifecycle reason is invalid")

    first = require_object(lifecycle[0], "incident.lifecycle[0]")
    if first.get("action") != "checks_requested" or first.get("delivery_id") != first_delivery:
        raise QueueIncidentError(
            "incident lifecycle must begin with the captured checks_requested delivery"
        )

    status = require_string(incident.get("status"), "incident.status")
    terminal_entries = [entry for entry in lifecycle if entry.get("action") == "destroyed"]
    if len(terminal_entries) > 1:
        raise QueueIncidentError("incident lifecycle may contain only one destroyed event")
    expected_status = "captured"
    if terminal_entries:
        expected_status = terminal_entries[0]["reason"]
    if status != expected_status:
        raise QueueIncidentError(
            f"incident.status must be {expected_status!r} for its lifecycle"
        )

    limitations = require_list(incident.get("limitations"), "incident.limitations")
    for index, limitation in enumerate(limitations):
        require_string(limitation, f"incident.limitations[{index}]")


def append_destroyed(
    *,
    incident: dict[str, Any],
    event_payload: dict[str, Any],
    raw_event: bytes,
    delivery_id: str,
    observed_at: str,
    signature_verified: bool,
) -> dict[str, Any]:
    if not signature_verified:
        raise QueueIncidentError("authenticated webhook delivery is required")
    validate_incident(incident)
    destroyed = normalize_destroyed(event_payload)
    merge_group = incident["merge_group"]
    for field in ("repository", "base_sha", "head_sha", "head_tree_sha"):
        if destroyed[field] != merge_group[field]:
            raise QueueIncidentError(
                f"destroyed event {field} does not match captured incident"
            )

    delivery = require_delivery_id(delivery_id)
    existing_deliveries = {
        entry["delivery_id"] for entry in incident["lifecycle"]
    }
    if delivery in existing_deliveries:
        raise QueueIncidentError(f"duplicate lifecycle delivery id: {delivery}")
    if any(entry["action"] == "destroyed" for entry in incident["lifecycle"]):
        raise QueueIncidentError("incident already has a terminal destroyed event")

    updated = copy.deepcopy(incident)
    updated["lifecycle"].append(
        {
            "delivery_id": delivery,
            "action": "destroyed",
            "observed_at": require_timestamp(observed_at, "observed_at"),
            "event_sha256": sha256_hex(raw_event),
            "reason": destroyed["reason"],
        }
    )
    updated["status"] = destroyed["reason"]
    validate_incident(updated)
    return updated


def atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    encoded = json.dumps(value, indent=2, sort_keys=True) + "\n"
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            temporary = Path(handle.name)
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except OSError as error:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
        raise QueueIncidentError(f"could not write {path}: {error}") from error


def command_capture(args: argparse.Namespace) -> int:
    event_path = Path(args.event)
    raw_event = event_path.read_bytes()
    secret = os.environ.get("GITHUB_WEBHOOK_SECRET", "").encode("utf-8")
    verify_webhook_signature(secret, raw_event, args.signature)
    incident = build_incident(
        event_payload=load_json_bytes(raw_event, "event"),
        raw_event=raw_event,
        queue_snapshot=load_json(Path(args.queue_snapshot), "queue snapshot"),
        object_manifest=load_json(Path(args.object_manifest), "object manifest"),
        delivery_id=args.delivery_id,
        captured_at=args.captured_at or now_rfc3339(),
        signature_verified=True,
    )
    atomic_write_json(Path(args.output), incident)
    print(incident["incident_id"])
    return 0


def command_validate(args: argparse.Namespace) -> int:
    incident = load_json(Path(args.incident), "incident")
    validate_incident(incident)
    print(incident["incident_id"])
    return 0


def command_append_destroyed(args: argparse.Namespace) -> int:
    incident_path = Path(args.incident)
    event_path = Path(args.event)
    raw_event = event_path.read_bytes()
    secret = os.environ.get("GITHUB_WEBHOOK_SECRET", "").encode("utf-8")
    verify_webhook_signature(secret, raw_event, args.signature)
    updated = append_destroyed(
        incident=load_json(incident_path, "incident"),
        event_payload=load_json_bytes(raw_event, "event"),
        raw_event=raw_event,
        delivery_id=args.delivery_id,
        observed_at=args.observed_at or now_rfc3339(),
        signature_verified=True,
    )
    output = Path(args.output) if args.output else incident_path
    atomic_write_json(output, updated)
    print(updated["incident_id"])
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Build and validate authenticated merge-group incident manifests"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    capture = subparsers.add_parser(
        "capture", description="Build an incident from already-fetched queue and object receipts"
    )
    capture.add_argument("--event", required=True)
    capture.add_argument("--signature", required=True)
    capture.add_argument("--queue-snapshot", required=True)
    capture.add_argument("--object-manifest", required=True)
    capture.add_argument("--delivery-id", required=True)
    capture.add_argument("--captured-at")
    capture.add_argument("--output", required=True)
    capture.set_defaults(handler=command_capture)

    validate = subparsers.add_parser("validate", description="Validate an incident manifest")
    validate.add_argument("--incident", required=True)
    validate.set_defaults(handler=command_validate)

    destroyed = subparsers.add_parser(
        "append-destroyed", description="Append one authenticated terminal lifecycle event"
    )
    destroyed.add_argument("--incident", required=True)
    destroyed.add_argument("--event", required=True)
    destroyed.add_argument("--signature", required=True)
    destroyed.add_argument("--delivery-id", required=True)
    destroyed.add_argument("--observed-at")
    destroyed.add_argument("--output")
    destroyed.set_defaults(handler=command_append_destroyed)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return int(args.handler(args))
    except (OSError, QueueIncidentError) as error:
        print(f"queue incident error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
