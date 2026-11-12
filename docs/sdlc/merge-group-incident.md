# Merge-group incident contract

This slice establishes the authenticated, immutable evidence contract used before a failed merge group can be localized. It does not yet query GitHub or fetch a temporary queue ref.

## Why capture precedes diagnosis

A merge queue can rebuild or delete its temporary ref after a failure, reorder entries after a jump, or move to a new target-branch base. Diagnosing the queue afterward from pull-request numbers and current heads would silently substitute a different integration experiment.

The capture boundary therefore retains:

- the exact `merge_group: checks_requested` delivery digest;
- the event base and head commit, refs and head tree;
- a normalized ordered queue-group snapshot;
- one cumulative integration base/head pair per pull request;
- queue merge method and grouping strategy;
- a verified object-bundle receipt covering every base, integration and pull-request head commit;
- one immutable incident identity;
- later terminal lifecycle state without rewriting the original capture.

The portable schema is [`schemas/sdlc/queue-incident-v1.schema.json`](../../schemas/sdlc/queue-incident-v1.schema.json). A checked example is [`docs/examples/queue-incident.v1.json`](../examples/queue-incident.v1.json).

## Authentication

`queue_incident.py capture` accepts the raw event body and the exact `X-Hub-Signature-256` value. The webhook secret is read only from `GITHUB_WEBHOOK_SECRET`; it is not accepted on the command line or written into the incident.

The signature must use HMAC-SHA256 and is compared in constant time before the incident is built.

## Queue snapshot contract

The normalized queue snapshot contains only entries that make up the captured merge group. Positions begin at one and are contiguous. Each entry records:

- provider queue-entry identity;
- pull-request number and immutable head SHA;
- cumulative integration base and head SHA;
- queue state, jump and solo flags.

The entries must form one chain:

```text
event base
  -> entry 1 integration head
  -> entry 2 integration head
  -> ...
  -> event head
```

A missing, reordered or inconsistent entry fails capture. The contract does not guess the order.

## Object receipt contract

The object manifest names a retained bundle and records:

- relative owned path;
- byte size and SHA-256 digest;
- exact event head commit and tree;
- every verified commit needed by the queue snapshot;
- the verification command, time and successful result.

This manifest is a receipt produced by the future live Git/ref capture adapter. This slice validates the receipt and its relationship to the event and queue snapshot; it does not independently run `git bundle verify` or inspect bundle bytes.

## Incident identity

The incident ID is derived from:

```text
repository
+ merge-group base SHA
+ merge-group head SHA
+ canonical queue-snapshot digest
```

A changed base, head, order or entry identity produces a different incident. A repeated authenticated delivery for the same immutable experiment can be deduplicated by delivery ID and incident ID.

Raw events remain separate private evidence. The incident stores their digests and lifecycle identities.

## Lifecycle

The incident begins with one authenticated `checks_requested` entry. One matching authenticated `destroyed` event may later append:

- `merged`;
- `invalidated`;
- `dequeued`.

Appending terminal state does not change the incident ID or original capture. A second terminal event, duplicate delivery ID, or mismatched repository/base/head/tree is rejected.

## Commands

Build an incident after the queue snapshot and object receipt have already been fetched:

```bash
export GITHUB_WEBHOOK_SECRET='from-secret-store'

python3 tools/sdlc/queue_incident.py capture \
  --event private/checks-requested.json \
  --signature "$X_HUB_SIGNATURE_256" \
  --queue-snapshot private/queue-snapshot.json \
  --object-manifest private/object-manifest.json \
  --delivery-id "$X_GITHUB_DELIVERY" \
  --output private/incident.json
```

Validate a retained incident:

```bash
python3 tools/sdlc/queue_incident.py validate \
  --incident private/incident.json
```

Append a matching terminal event:

```bash
python3 tools/sdlc/queue_incident.py append-destroyed \
  --incident private/incident.json \
  --event private/destroyed.json \
  --signature "$X_HUB_SIGNATURE_256" \
  --delivery-id "$X_GITHUB_DELIVERY"
```

Run all SDLC behavior contracts:

```bash
python3 -m unittest discover -s tools/sdlc/tests -p 'test_*.py'
```

`cargo xtask ci-full` now resolves `python3`, `python`, or Windows `py -3` and runs these contracts before golden, schema and scenario checks.

## Deliberate limits

This draft does not yet:

- receive webhooks as a service;
- query GitHub GraphQL merge-queue entries;
- follow pagination;
- fetch or retain the synthetic Git ref;
- produce the object manifest from verified Git objects;
- hold a repository capture lock;
- publish a Check or run candidate code;
- convert the queue snapshot into `faultline-integration` domain values.

Those remain required by #82. The current slice makes their inputs and claim boundaries testable before network and Git mutation are added.
