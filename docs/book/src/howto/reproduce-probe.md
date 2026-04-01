# How to Use the Reproduce Subcommand

This guide explains how to use `faultline reproduce` to re-run a specific probe from a completed faultline run.

## When to use reproduce

Use the reproduce subcommand when:
- You want to manually verify a result at a specific commit
- You need to debug why a predicate passed or failed at a boundary commit
- You want to share exact reproduction steps with a colleague

## Step 1: Run faultline and note the output directory

```bash
faultline-cli --repo . --good v1.0 --bad main \
  --cmd "cargo test" --output-dir regression-report
```

## Step 2: Reproduce the boundary commits

By default, `reproduce` targets the boundary commits (the last-good and first-bad):

```bash
faultline reproduce --run-dir regression-report
```

This checks out the boundary commit, sets the environment, and runs the predicate.

## Step 3: Reproduce a specific commit

To reproduce the probe for a specific commit:

```bash
faultline reproduce --run-dir regression-report --commit abc1234
```

## Step 4: Generate a shell script instead

Use `--shell` to emit a POSIX shell script to stdout instead of executing the probe:

```bash
faultline reproduce --run-dir regression-report --commit abc1234 --shell
```

The script includes:
- `cd` to the working directory
- `git checkout` of the target commit
- Environment variable exports
- The predicate command with timeout

You can save this script, share it, or modify it for debugging:

```bash
faultline reproduce --run-dir regression-report --commit abc1234 --shell > repro.sh
chmod +x repro.sh
./repro.sh
```

## How it works

Each faultline run generates a `ReproductionCapsule` for every probed commit. The capsule captures:
- The exact commit SHA
- The predicate command (program or shell command)
- Environment variables
- Working directory
- Timeout in seconds

The `reproduce` subcommand reads the report from the run directory, finds the capsule for the requested commit, and either executes it or emits it as a script.

## Notes

- If no `--commit` is specified, faultline reproduces the boundary commits by default.
- Shell-special characters in predicate arguments and environment values are escaped in the generated script.
- The run directory must contain a valid `analysis.json` report.
