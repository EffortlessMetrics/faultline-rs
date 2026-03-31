# Your First Faultline Run

This tutorial walks you through installing faultline, running it against a Git repository, and reading the output artifacts.

## Prerequisites

- Rust stable toolchain (install via [rustup](https://rustup.rs/))
- Git installed and on your PATH
- A Git repository with a known regression (a good commit and a bad commit)

## Step 1: Install faultline

Clone the repository and build the CLI:

```bash
git clone https://github.com/your-org/faultline.git
cd faultline
cargo build --release
```

The binary is at `target/release/faultline-cli`. You can add it to your PATH or invoke it directly.

## Step 2: Identify your regression boundaries

You need two pieces of information:

1. A **good** commit — a revision where your test or build passes
2. A **bad** commit — a revision where it fails

For example, if your tests pass on `v1.0.0` but fail on `main`:

```bash
git log --oneline v1.0.0..main
```

## Step 3: Write a predicate

A predicate is a command that faultline runs at each candidate revision. It must follow the [predicate contract](../reference/predicate-contract.md):

- Exit `0` → pass
- Exit `125` → skip (untestable revision)
- Any other non-zero exit → fail
- Timeout → indeterminate

For example, to test whether `cargo test` passes:

```bash
# Using --cmd (shell command)
faultline-cli --repo . --good v1.0.0 --bad main --cmd "cargo test"
```

Or using a standalone script:

```bash
# Using --program (direct exec)
faultline-cli --repo . --good v1.0.0 --bad main --program ./test.sh
```

## Step 4: Run faultline

Run the full command:

```bash
cargo run -p faultline-cli -- \
  --repo /path/to/your/repo \
  --good abc1234 \
  --bad def5678 \
  --cmd "cargo test --lib" \
  --output-dir faultline-report
```

Faultline will:

1. Enumerate the commits between `--good` and `--bad`
2. Probe boundary commits first (the good and bad endpoints)
3. Use binary narrowing to select the next candidate
4. Run your predicate at each candidate revision using a disposable worktree
5. Converge on the narrowest credible regression window

You'll see output like:

```
run-id       a1b2c3d4
observations 7
output-dir   faultline-report
artifacts    faultline-report/analysis.json
             faultline-report/index.html
history      ancestry-path
outcome      FirstBad  last_good=abc1234 first_bad=def5678 confidence=90(high)
```

## Step 5: Read the output artifacts

Faultline produces two artifacts in the output directory:

### analysis.json

A machine-readable JSON report containing the full analysis: the request parameters, the revision sequence, all probe observations, the localization outcome, changed paths, and surface summary. This file conforms to the [artifact schema](../reference/artifact-schema.md).

```bash
cat faultline-report/analysis.json | jq '.outcome'
```

### index.html

A human-readable HTML report you can open in any browser. It shows the same information in a visual layout with the commit timeline, observation results, and outcome summary.

```bash
open faultline-report/index.html
```

## Step 6: Interpret the outcome

Faultline produces one of three outcome types:

| Outcome | Meaning |
|---------|---------|
| **FirstBad** | Exact regression commit identified with high confidence |
| **SuspectWindow** | A range of suspect commits, with reasons for ambiguity (skipped revisions, non-monotonic evidence, etc.) |
| **Inconclusive** | Not enough evidence to narrow the window (missing boundaries, max probes exhausted, etc.) |

If you get a `SuspectWindow` or `Inconclusive` result, check the `reasons` field in the JSON output for details on what prevented convergence.

## Next steps

- Learn about [CLI flags](../reference/cli-flags.md) for advanced options
- Read [How Localization Works](../explanations/localization.md) to understand the algorithm
- See [Adding a New Property Test](../howto/add-property-test.md) if you're contributing to faultline
