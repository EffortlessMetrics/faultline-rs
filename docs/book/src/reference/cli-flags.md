# CLI Flags

Reference for all `faultline-cli` command-line flags.

## Required flags

| Flag | Type | Description |
|------|------|-------------|
| `--good <REV>` | string | Known-good revision (commit hash, tag, or branch) |
| `--bad <REV>` | string | Known-bad revision (commit hash, tag, or branch) |

One of `--cmd` or `--program` is also required (see Predicate flags below).

## Predicate flags

Exactly one of these must be provided:

| Flag | Type | Description |
|------|------|-------------|
| `--cmd <SCRIPT>` | string | Shell command to run as the predicate. Executed via the system shell (or the shell specified by `--shell`) |
| `--program <PATH>` | string | Program to execute directly (no shell). Use `--arg` to pass arguments |

Supporting flags:

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--arg <ARG>` | string (repeatable) | — | Arguments passed to `--program`. Allows hyphen values. Not used with `--cmd` |
| `--kind <KIND>` | string | `custom` | Probe kind label: `build`, `test`, `lint`, `perf-threshold`, `custom` |
| `--shell <SHELL>` | string | system default | Shell for `--cmd` predicates: `sh`, `cmd`, `powershell` |
| `--env <KEY=VALUE>` | string (repeatable) | — | Environment variables injected into the predicate process |
| `--timeout-seconds <N>` | integer | `300` | Maximum seconds per predicate invocation before timeout (indeterminate) |

## Repository flags

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--repo <PATH>` | path | `.` | Path to the Git repository |
| `--first-parent` | bool | `false` | Use `--first-parent` history traversal instead of `--ancestry-path` |

## Search flags

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--max-probes <N>` | integer | `64` | Maximum number of predicate invocations before stopping |

## Output flags

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--output-dir <PATH>` | path | `faultline-report` | Directory for output artifacts (`analysis.json`, `index.html`) |
| `--no-render` | bool | `false` | Skip HTML report generation; only produce `analysis.json` |

## Run mode flags

These flags are mutually exclusive:

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--resume` | bool | `false` | Explicitly resume from cached observations (default behavior) |
| `--force` | bool | `false` | Discard cached observations and re-probe all commits |
| `--fresh` | bool | `false` | Delete the entire run directory and start from scratch |

If none of these are specified, faultline resumes from cached observations by default.

## Examples

Basic usage with a shell command:

```bash
faultline-cli --good v1.0 --bad main --cmd "cargo test"
```

Direct program execution with arguments:

```bash
faultline-cli --good abc123 --bad def456 --program ./test.sh --arg --verbose --arg --fast
```

Custom probe budget and output directory:

```bash
faultline-cli --good v1.0 --bad main --cmd "make test" \
  --max-probes 20 --output-dir ./results
```

With environment variables and explicit shell:

```bash
faultline-cli --good v1.0 --bad main \
  --cmd "npm test" --shell sh \
  --env CI=true --env NODE_ENV=test
```
