use anyhow::Result;
use clap::{Parser, Subcommand};

use xtask::ci;
use xtask::scaffold;
use xtask::schema;
use xtask::tools;

#[derive(Parser)]
#[command(name = "xtask", about = "faultline repo operations")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run fmt + clippy + test (fast CI tier)
    CiFast,
    /// Run ci-fast + golden + schema-check (full CI tier)
    CiFull,
    /// Build CLI and run against fixture repo
    Smoke,
    /// Run and update golden/snapshot tests
    Golden,
    /// Run cargo-mutants on configured surfaces
    Mutants,
    /// Run fuzz targets (default 60s, --duration to override)
    Fuzz {
        #[arg(long, default_value_t = 60)]
        duration: u64,
    },
    /// Build docs and check links
    DocsCheck,
    /// Run cargo-deny + cargo-audit + cargo-semver-checks
    ReleaseCheck,
    /// Generate boilerplate for new repo artifacts
    Scaffold {
        #[command(subcommand)]
        kind: ScaffoldKind,
    },
    /// Regenerate JSON Schema files (internal)
    #[command(hide = true)]
    GenerateSchema,
}

#[derive(Subcommand)]
enum ScaffoldKind {
    /// Create a new crate under crates/
    Crate {
        name: String,
        #[arg(long)]
        tier: String,
    },
    /// Create a new ADR under docs/adr/
    Adr { title: String },
    /// Create a new test scenario stub
    Scenario {
        name: String,
        #[arg(long)]
        crate_name: String,
    },
    /// Create a new doc page in the mdBook site
    Doc {
        title: String,
        #[arg(long)]
        section: String,
    },
}

fn run_cmd(contract: &str, cmd: &str, args: &[&str]) -> Result<()> {
    println!("=> {cmd} {}", args.join(" "));
    let status = std::process::Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to execute {cmd}: {e}"))?;
    if !status.success() {
        anyhow::bail!("contract broken: {contract}");
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::CiFast => ci::ci_fast()?,
        Command::CiFull => ci::ci_full()?,

        Command::Smoke => {
            println!("=== smoke ===\n");
            run_cmd("CLI build", "cargo", &["build", "-p", "faultline-cli"])?;
            println!("\n=== smoke passed (fixture run is a placeholder) ===");
        }

        Command::Golden => {
            tools::ensure_tool("cargo-insta", "cargo install cargo-insta");
            println!("=== golden ===\n");
            run_cmd(
                "golden snapshot update",
                "cargo",
                &["insta", "test", "--review"],
            )?;
            println!("\n=== golden passed ===");
        }

        Command::Mutants => {
            tools::ensure_tool("cargo-mutants", "cargo install cargo-mutants");
            println!("=== mutants ===\n");
            run_cmd(
                "mutation testing",
                "cargo",
                &["mutants", "-p", "faultline-localization", "--", "--lib"],
            )?;
            println!("\n=== mutants passed ===");
        }

        Command::Fuzz { duration } => {
            tools::ensure_tool("cargo-fuzz", "cargo install cargo-fuzz");
            println!("=== fuzz (duration: {duration}s) ===\n");
            let dur = format!("{duration}");
            run_cmd(
                "fuzz testing",
                "cargo",
                &[
                    "fuzz",
                    "run",
                    "fuzz_analysis_report",
                    "--",
                    &format!("-max_total_time={dur}"),
                ],
            )?;
            println!("\n=== fuzz passed ===");
        }

        Command::DocsCheck => {
            tools::ensure_tool("mdbook", "cargo install mdbook");
            println!("=== docs-check ===\n");
            run_cmd("mdbook build", "mdbook", &["build", "docs/book"])?;
            println!("\n=== docs-check passed ===");
        }

        Command::ReleaseCheck => {
            tools::ensure_tool("cargo-deny", "cargo install cargo-deny");
            tools::ensure_tool("cargo-audit", "cargo install cargo-audit");
            tools::ensure_tool("cargo-semver-checks", "cargo install cargo-semver-checks");

            println!("=== release-check ===\n");
            run_cmd("supply-chain policy", "cargo", &["deny", "check"])?;
            run_cmd("security audit", "cargo", &["audit"])?;
            run_cmd("semver compatibility", "cargo", &["semver-checks"])?;
            println!("\n=== release-check passed ===");
        }

        Command::Scaffold { kind } => {
            let root = scaffold::workspace_root()?;
            match kind {
                ScaffoldKind::Crate { name, tier } => {
                    scaffold::scaffold_crate(&root, &name, &tier)?
                }
                ScaffoldKind::Adr { title } => scaffold::scaffold_adr(&root, &title)?,
                ScaffoldKind::Scenario { name, crate_name } => {
                    scaffold::scaffold_scenario(&root, &name, &crate_name)?
                }
                ScaffoldKind::Doc { title, section } => {
                    scaffold::scaffold_doc(&root, &title, &section)?
                }
            }
        }

        Command::GenerateSchema => schema::generate_schema()?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    // Feature: repo-operating-system, Property 43: Xtask Help Completeness
    // **Validates: Requirements 5.2, 5.5, 10.1**

    #[test]
    fn xtask_help_lists_all_subcommands() {
        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        cmd.write_long_help(&mut buf).unwrap();
        let help = String::from_utf8(buf).unwrap();

        let expected_subcommands = [
            "ci-fast",
            "ci-full",
            "smoke",
            "golden",
            "mutants",
            "fuzz",
            "docs-check",
            "release-check",
            "scaffold",
        ];

        for name in &expected_subcommands {
            assert!(
                help.contains(name),
                "xtask --help missing subcommand: {name}\n--- help output ---\n{help}"
            );
        }
    }

    #[test]
    fn scaffold_help_lists_all_kinds() {
        let cmd = Cli::command();
        let scaffold_cmd = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "scaffold")
            .expect("scaffold subcommand not found");

        let mut buf = Vec::new();
        scaffold_cmd.clone().write_long_help(&mut buf).unwrap();
        let help = String::from_utf8(buf).unwrap();

        let expected_kinds = ["crate", "adr", "scenario", "doc"];

        for kind in &expected_kinds {
            assert!(
                help.contains(kind),
                "scaffold --help missing kind: {kind}\n--- help output ---\n{help}"
            );
        }
    }
}
