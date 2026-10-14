# Find where a regression began

Your test or build passed at an earlier commit and fails now. Use faultline to
find the smallest window the results support, then inspect those commits.

## Start with a check you trust

Choose a command you can run repeatedly at older commits. Confirm that it passes
at your good commit and fails at your bad commit. If `cargo test` captures the
problem, use that same command throughout the investigation.

You need Git and Rust 1.93 or newer. Install the initial release:

```bash
cargo install faultline-cli --version 0.0.1 --locked
```

If the release is not yet available, [build from source](../../../../BUILDING.md).
The executable is named `faultline-cli`.

## Run the investigation

From your project's directory, replace `GOOD_COMMIT` with a commit where the
check passes. Use `HEAD` as the bad commit only if the check fails there:

```bash
faultline-cli --good GOOD_COMMIT --bad HEAD --cmd "cargo test"
```

For a different check, replace `cargo test` with your command. Choose the shell
with `--shell` when needed. To run a program directly, use `--program` and supply
each argument with `--arg`.
The [command contract](../reference/predicate-contract.md) explains passing,
failing, skipped and timed-out checks.

## Read the evidence

Open `faultline-report/index.html` in your browser. It shows the checked commits,
their results and the reported regression window.
Keep `faultline-report/analysis.json` when you need to share or automate the run.

| Result | What to do next |
|--------|-----------------|
| FirstBad | Compare the last passing and first failing commits and investigate the relevant changes. |
| SuspectWindow | Inspect the reported range and the reasons that kept it from narrowing further. |
| Inconclusive | Read the reasons, correct the command or boundaries, and run again. |

The result points you toward changes to investigate. You still need to establish
the cause and fix it. A flaky command, a timeout or an untestable commit may leave
a range rather than one commit.

## Improve or share the run

- Use `--retries` for a command that sometimes changes its result.
- Repeated investigations can reuse previous results. Fresh rechecks are not
  currently available through the command; see the
  [rerun limitation](../reference/cli-flags.md#run-mode-flags).
- Use `--output-dir` to keep separate investigations in different folders.
- Review reports before sharing them, even though common secret patterns and
  environment values are scrubbed by default.

See [CLI options](../reference/cli-flags.md) for the full command reference.
For automated consumers, use the [report reference](../reference/artifact-schema.md).
