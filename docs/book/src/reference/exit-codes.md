# Exit Codes

Faultline uses distinct exit codes to communicate the analysis outcome to calling processes and CI systems.

## CLI exit codes

| Exit code | Operator code | Meaning |
|-----------|--------------|---------|
| `0` | `Success` | `FirstBad` — exact regression commit identified |
| `1` | `SuspectWindow` | `SuspectWindow` — regression narrowed to a range with ambiguity |
| `2` | `ExecutionError` | Runtime error (I/O failure, Git error, etc.) |
| `3` | `Inconclusive` | `Inconclusive` — insufficient evidence to narrow the window |
| `4` | `InvalidInput` | Invalid input (bad flags, invalid boundaries, etc.) |

## Usage in CI

You can use exit codes to drive CI behavior:

```bash
faultline-cli --good v1.0 --bad main --cmd "cargo test"
case $? in
  0) echo "Exact regression commit found" ;;
  1) echo "Suspect window identified — manual review needed" ;;
  2) echo "Execution error — check logs" ;;
  3) echo "Inconclusive — increase --max-probes or improve predicate" ;;
  4) echo "Invalid input — check arguments" ;;
esac
```

## Predicate exit codes

These are the exit codes your predicate command should return. They are distinct from the CLI exit codes above.

| Exit code | Classification | Meaning |
|-----------|---------------|---------|
| `0` | Pass | Revision is good |
| `125` | Skip | Revision is untestable |
| Non-zero (not 125) | Fail | Revision exhibits the regression |
| Timeout | Indeterminate | Predicate did not complete in time |

See the [Predicate Contract](predicate-contract.md) for details.
