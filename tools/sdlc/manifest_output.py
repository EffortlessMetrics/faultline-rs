"""Human and GitHub Check projections for evidence manifests."""

from __future__ import annotations

from typing import Any

from manifest_base import DEFAULT_CHECK_NAME, EvidenceError, manifest_digest
from manifest_validate import readiness_errors, validate_manifest


def render_summary(manifest: dict[str, Any]) -> str:
    errors = readiness_errors(manifest)
    candidate = manifest.get("candidate", {})
    lines = [
        "## Warm evidence",
        "",
        f"- Repository: `{candidate.get('repository', 'unknown')}`",
        f"- Pull request: `#{candidate.get('pull_request', 'unknown')}`",
        f"- Candidate head: `{candidate.get('head_sha', 'unknown')}`",
        f"- Candidate tree: `{candidate.get('tree_sha', 'unknown')}`",
        f"- Manifest SHA-256: `{manifest_digest(manifest)}`",
        f"- Qualification readiness: **{'ready' if not errors else 'blocked'}**",
        "",
        "### Executions",
        "",
        "| ID | Required | Result | Duration | Establishes |",
        "| --- | ---: | --- | ---: | --- |",
    ]
    for execution in manifest.get("executions", []):
        lines.append(
            "| `{}` | {} | {} | {} ms | {} |".format(
                execution.get("id", "?"),
                "yes" if execution.get("required") else "no",
                execution.get("status", "?"),
                execution.get("duration_ms", "?"),
                ", ".join(execution.get("establishes", [])),
            )
        )
    lines.extend(["", "### Reviews", ""])
    for review in manifest.get("reviews", []):
        independence = "independent" if review.get("independent") else "author"
        lines.append(
            f"- `{review.get('kind', '?')}` — {review.get('result', '?')} "
            f"by {review.get('reviewer', '?')} ({independence})"
        )
    if manifest.get("limitations"):
        lines.extend(["", "### Limitations", ""])
        lines.extend(f"- {item}" for item in manifest["limitations"])
    if errors:
        lines.extend(["", "### Blocking findings", ""])
        lines.extend(f"- {item}" for item in errors)
    return "\n".join(lines) + "\n"


def build_check_payload(
    manifest: dict[str, Any],
    *,
    name: str = DEFAULT_CHECK_NAME,
    details_url: str | None = None,
) -> dict[str, Any]:
    structural_errors = validate_manifest(manifest)
    if structural_errors:
        raise EvidenceError("invalid evidence manifest: " + "; ".join(structural_errors))
    errors = readiness_errors(manifest)
    payload: dict[str, Any] = {
        "name": name,
        "head_sha": manifest["candidate"]["head_sha"],
        "status": "completed",
        "conclusion": "success" if not errors else "failure",
        "external_id": f"faultline-evidence-v1:{manifest_digest(manifest)}",
        "output": {
            "title": "Warm evidence ready" if not errors else "Warm evidence blocked",
            "summary": render_summary(manifest)[:65535],
        },
    }
    if details_url:
        payload["details_url"] = details_url
    return payload
