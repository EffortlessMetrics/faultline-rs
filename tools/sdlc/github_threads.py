"""Review-thread retrieval, including pagination and outdated threads."""

from __future__ import annotations

from typing import Any

from github_http import DEFAULT_GRAPHQL_URL, graphql
from manifest import EvidenceError


def fetch_review_threads(
    repository: str,
    pull_request: int,
    *,
    token: str,
    graphql_url: str = DEFAULT_GRAPHQL_URL,
) -> list[dict[str, Any]]:
    owner, name = repository.split("/", 1)
    query = """
    query($owner: String!, $name: String!, $number: Int!, $after: String) {
      repository(owner: $owner, name: $name) {
        pullRequest(number: $number) {
          reviewThreads(first: 100, after: $after) {
            nodes { id isResolved isOutdated comments(first: 1) { nodes { url body } } }
            pageInfo { hasNextPage endCursor }
          }
        }
      }
    }
    """
    threads: list[dict[str, Any]] = []
    after: str | None = None
    while True:
        data = graphql(
            query,
            {"owner": owner, "name": name, "number": pull_request, "after": after},
            token=token,
            graphql_url=graphql_url,
        )
        repository_data = data.get("repository")
        pr_data = repository_data.get("pullRequest") if isinstance(repository_data, dict) else None
        thread_data = pr_data.get("reviewThreads") if isinstance(pr_data, dict) else None
        if not isinstance(thread_data, dict):
            raise EvidenceError("pull request reviewThreads were unavailable")
        nodes = thread_data.get("nodes", [])
        if isinstance(nodes, list):
            threads.extend(item for item in nodes if isinstance(item, dict))
        page_info = thread_data.get("pageInfo", {})
        if not isinstance(page_info, dict) or not page_info.get("hasNextPage"):
            break
        after = page_info.get("endCursor")
        if not isinstance(after, str) or not after:
            raise EvidenceError("review-thread pagination omitted endCursor")
    return threads
