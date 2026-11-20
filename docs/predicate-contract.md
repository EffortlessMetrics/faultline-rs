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

## Standard input and Linux cleanup

Predicates inherit standard input. On Linux, Faultline uses an owned process group
for timeout and interrupt cleanup when stdin is not a terminal. This covers
descendants that stay in the group. With terminal stdin, predicates keep the
inherited foreground group so they can read input and receive terminal Ctrl-C
normally. Terminal runs retain the existing direct-child timeout behavior.
Use redirected stdin for a noninteractive run that needs group cleanup, for
example `faultline-rs --good GOOD --bad BAD --cmd "cargo test" </dev/null`.

## Guidance

Best results come from predicates that are:
- deterministic
- local
- isolated from external mutable services
- narrow enough to evaluate quickly

When the predicate is noisy, `faultline` will reduce confidence and may return a suspect window instead of an exact first-bad commit.
