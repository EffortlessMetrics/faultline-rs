"""Minimal GitHub REST and GraphQL client for the SDLC pilot."""

from __future__ import annotations

import json
import urllib.error
import urllib.request
from typing import Any

from manifest import EvidenceError, canonical_json

DEFAULT_API_URL = "https://api.github.com"
DEFAULT_GRAPHQL_URL = "https://api.github.com/graphql"


def request_json(
    url: str,
    *,
    token: str,
    method: str = "GET",
    body: dict[str, Any] | None = None,
) -> dict[str, Any]:
    data = None if body is None else canonical_json(body)
    request = urllib.request.Request(
        url,
        data=data,
        method=method,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "X-GitHub-Api-Version": "2022-11-28",
            "Content-Type": "application/json",
            "User-Agent": "faultline-sdlc-evidence/1",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            value = json.load(response)
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")
        raise EvidenceError(f"GitHub API {method} {url} failed: {error.code} {detail}") from error
    except urllib.error.URLError as error:
        raise EvidenceError(f"GitHub API {method} {url} failed: {error}") from error
    if not isinstance(value, dict):
        raise EvidenceError(f"GitHub API {method} {url} returned a non-object")
    return value


def graphql(
    query: str,
    variables: dict[str, Any],
    *,
    token: str,
    graphql_url: str = DEFAULT_GRAPHQL_URL,
) -> dict[str, Any]:
    value = request_json(
        graphql_url,
        token=token,
        method="POST",
        body={"query": query, "variables": variables},
    )
    if value.get("errors"):
        raise EvidenceError(f"GitHub GraphQL returned errors: {value['errors']}")
    data = value.get("data")
    if not isinstance(data, dict):
        raise EvidenceError("GitHub GraphQL response did not contain data")
    return data
