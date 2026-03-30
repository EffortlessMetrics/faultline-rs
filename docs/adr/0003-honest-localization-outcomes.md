# ADR-0003: Honest localization outcomes

## Status
Accepted.

## Decision

The localization engine returns one of:

- exact first-bad commit
- suspect window
- inconclusive result

It does not claim more certainty than the evidence supports.
