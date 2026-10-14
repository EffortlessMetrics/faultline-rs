# faultline

[![CI Fast](https://github.com/EffortlessMetrics/faultline-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/EffortlessMetrics/faultline-rs/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

A test or build used to pass. Now it fails, and you are checking commits by hand
to work out where to start.

faultline runs the same command through your Git history and returns the
narrowest regression window supported by the results. You get commits to inspect
and a report showing the evidence behind that result.

## Get started

You need Git, Rust 1.93 or newer, a commit where your command passes, and a commit
where it fails.

Install the initial release:

```bash
cargo install faultline-cli --version 0.0.1 --locked
```

If 0.0.1 is not yet available on crates.io, [build from source](BUILDING.md).

From the repository you are investigating, replace `GOOD_COMMIT` with your
known passing commit. This example uses `HEAD` as the failing commit and
`cargo test` as the check:

```bash
faultline-cli --good GOOD_COMMIT --bad HEAD --kind test --program cargo --arg test
```

Use your own test or build command if the project needs a different check.
The [first-run guide](docs/book/src/tutorials/first-run.md) walks through choosing
the commits and reading the result.

The installed command is `faultline-cli`; its help text uses the product name
`faultline`.

## What you get

Open `faultline-report/index.html` in your browser. It shows which commits were
checked, what happened, and where to investigate next. Keep
`faultline-report/analysis.json` alongside it when sharing or automating a run.

| Result | What it means for your investigation |
|--------|--------------------------------------|
| FirstBad | The evidence supports a last passing commit and a first failing commit. |
| SuspectWindow | Start with the reported range; the report explains why it could not narrow further. |
| Inconclusive | Check the reported reasons and improve the command or boundaries before trying again. |

## What to expect

- You still need to explain and fix the regression after inspecting the reported commits.
- Flaky results, timeouts and commits that cannot be tested can leave a range or an inconclusive result.
- Your command needs to run at older commits with the tools and dependencies it requires.
- Shareable reports scrub common secret patterns and environment values by default. Review the files before sharing; scrubbing cannot recognize every secret.

## Where to go next

- [Run your first investigation](docs/book/src/tutorials/first-run.md)
- [Choose a reliable check](docs/book/src/reference/predicate-contract.md)
- [Use retries, cached results and other options](docs/book/src/reference/cli-flags.md)
- [Build from source](BUILDING.md)
- [Report a problem](https://github.com/EffortlessMetrics/faultline-rs/issues)
- [Contribute](CONTRIBUTING.md)
