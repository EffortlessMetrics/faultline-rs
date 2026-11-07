# Late qualification pilot

This is the repository reference slice for issue #66. It adds the evidence and
preflight machinery needed for the target delivery flow without yet removing the
existing push and pull-request workflows.

## Target flow

1. The author works and tests in a warm coding session while the pull request is
   Draft or Ready for review.
2. The session records the actual tested snapshot, commands, results,
   limitations and independent review in an evidence manifest.
3. A GitHub App installation token publishes that manifest as a candidate-bound
   `Warm evidence` Check. Publishing the Check does not launch GitHub Actions.
4. Applying `merge-ready` (or manually dispatching the pilot with an exact head
   SHA) requests late qualification.
5. Preflight rejects a moved head, Draft PR, missing label, stale/failed evidence
   Check, or any unresolved review thread, including paginated and outdated
   threads.
6. The pilot checks out the exact admitted head and runs `cargo xtask ci-full`
   under a 20-minute job ceiling.
7. Green qualification does not merge, arm auto-merge or enter a merge queue. A
   separate product/current-state judgment still authorizes the protected
   squash merge.

The old workflows remain live during the pilot. Cutover means changing evidence
production, request handling and branch enforcement together after the behavior
is demonstrated; it is not part of this slice.

## Evidence contract

The schema is [`schemas/sdlc/evidence-v1.schema.json`](../../schemas/sdlc/evidence-v1.schema.json).
The standard-library implementation is [`tools/sdlc/evidence.py`](../../tools/sdlc/evidence.py).
A complete example is [`docs/examples/sdlc-evidence.v1.json`](../examples/sdlc-evidence.v1.json).

The manifest keeps these objects distinct:

- **candidate** — exact repository, pull request, commit and Git tree;
- **tested snapshot** — what bytes were actually exercised and how they were
  reconciled to the candidate;
- **executions** — commands, result, duration, claims and original evidence;
- **reviews** — question, reviewer, candidate, substantive result and source;
- **limitations** — what the evidence does not establish;
- **qualification requirements** — which executions and review questions must
  be current before the warm-evidence Check may report success.

`matched`, `reusable_with_limits` and `stale` are different states. The late
preflight accepts only an exact `matched` snapshot for its required proof. That
does not erase still-useful older evidence; it prevents that evidence from being
represented as proof of a different candidate.

## Local use

Validate and summarize a manifest:

```bash
python3 tools/sdlc/evidence.py validate \
  --manifest docs/examples/sdlc-evidence.v1.json \
  --require-ready

python3 tools/sdlc/evidence.py summary \
  --manifest docs/examples/sdlc-evidence.v1.json
```

Publish a dry-run Check payload:

```bash
python3 tools/sdlc/evidence.py publish-check \
  --manifest docs/examples/sdlc-evidence.v1.json \
  --expected-sha aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --dry-run
```

A real publication requires an installation token with Checks write permission. The publisher first verifies the live PR head and Git tree before creating the Check:

```bash
GITHUB_TOKEN="$INSTALLATION_TOKEN" \
python3 tools/sdlc/evidence.py publish-check \
  --manifest /path/to/evidence.json \
  --expected-sha "$PR_HEAD_SHA" \
  --details-url "$RETAINED_EVIDENCE_URL"
```

Do not place the token or per-run logs in Git. The Check contains a concise
candidate-bound summary and digest; larger retained evidence remains at the
referenced access-controlled location.

Run the behavior tests:

```bash
python3 -m unittest discover -s tools/sdlc/tests -p 'test_*.py'
```

## Qualification request

After this workflow is merged, the pilot can be requested in either form:

- apply `merge-ready` to a non-Draft PR whose exact head already has a successful
  `Warm evidence` Check; or
- manually dispatch **Late Qualification Pilot** with the PR number and exact
  expected head SHA.

The label is an admission request, not merge approval. Moving the head after the
request invalidates admission. Re-publishing evidence on a new candidate is an
explicit action rather than a green Check drifting forward.

## Deliberate limits of this slice

- It does not disable CI Fast on pushes or CI Full on ordinary PR events.
- It uses the current `cargo xtask ci-full` mandatory floor; risk-selected check
  expansion is a later measured step, not a hidden requirement to run the full
  catalog.
- It does not configure branch rules or create the `merge-ready` label.
- `.github/sdlc-policy.json` leaves `trusted_evidence_app_slugs` empty for the pilot. Populate it with the installed publisher App slug before cutover so another Check publisher cannot impersonate warm evidence.
- It does not implement the batched main-health controller.
- It does not make a green Check equivalent to substantive review. The publisher
  refuses success when the manifest contains `failed_to_review`, missing
  required proof or a non-matching tested snapshot.

Those boundaries let us test the handoff before replacing the live controls.
