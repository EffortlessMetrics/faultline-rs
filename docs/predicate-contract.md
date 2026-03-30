# Predicate contract

## Required behavior

The predicate must express one of:
- pass
- fail
- skip
- indeterminate (timeout / operational ambiguity)

## Exit classification

- `0` => pass
- `125` => skip / untestable revision
- any other non-zero => fail
- timeout => indeterminate

## Guidance

Best results come from predicates that are:
- deterministic
- local
- isolated from external mutable services
- narrow enough to evaluate quickly

When the predicate is noisy, `faultline` will reduce confidence and may return a suspect window instead of an exact first-bad commit.
