use anyhow::{Context, Result, bail};
use regex::Regex;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Workspace root discovery
// ---------------------------------------------------------------------------

/// Find the workspace root by searching upward for a Cargo.toml with [workspace].
pub fn workspace_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir().context("failed to get current directory")?;
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            let content = std::fs::read_to_string(&candidate)
                .with_context(|| format!("failed to read {}", candidate.display()))?;
            if content.contains("[workspace]") {
                return Ok(dir);
            }
        }
        if !dir.pop() {
            bail!("could not find workspace root (no Cargo.toml with [workspace] found)");
        }
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Validate that a crate name matches `faultline-[a-z][a-z0-9-]*`.
pub fn validate_crate_name(name: &str) -> Result<()> {
    let re = Regex::new(r"^faultline-[a-z][a-z0-9-]*$").unwrap();
    if !re.is_match(name) {
        bail!("error: crate name must match faultline-[a-z][a-z0-9-]*");
    }
    Ok(())
}

/// Validate that a string is non-empty.
pub fn validate_non_empty(value: &str, label: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("error: {label} must be non-empty");
    }
    Ok(())
}

/// Validate that a doc section is one of the four Diátaxis categories.
pub fn validate_section(section: &str) -> Result<()> {
    match section {
        "tutorial" | "howto" | "explanation" | "reference" => Ok(()),
        _ => bail!("error: section must be one of: tutorial, howto, explanation, reference"),
    }
}

// ---------------------------------------------------------------------------
// Scaffold: crate
// ---------------------------------------------------------------------------

/// Scaffold a new crate under `crates/<name>/`.
///
/// `root` is the workspace root directory.
pub fn scaffold_crate(root: &Path, name: &str, tier: &str) -> Result<()> {
    validate_crate_name(name)?;

    let crate_dir = root.join("crates").join(name);
    let src_dir = crate_dir.join("src");
    std::fs::create_dir_all(&src_dir)
        .with_context(|| format!("failed to create {}", src_dir.display()))?;

    // Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
"#
    );
    std::fs::write(crate_dir.join("Cargo.toml"), cargo_toml)
        .context("failed to write Cargo.toml")?;

    // src/lib.rs
    let lib_rs = format!("//! {name} — {tier} tier crate.\n");
    std::fs::write(src_dir.join("lib.rs"), lib_rs).context("failed to write src/lib.rs")?;

    // Append to workspace Cargo.toml members
    append_workspace_member(root, name)?;

    println!("scaffolded crate: crates/{name}/");
    Ok(())
}

/// Append a crate to the workspace `Cargo.toml` members list.
fn append_workspace_member(root: &Path, name: &str) -> Result<()> {
    let cargo_path = root.join("Cargo.toml");
    let content =
        std::fs::read_to_string(&cargo_path).context("failed to read workspace Cargo.toml")?;

    let entry = format!("\"crates/{name}\"");
    if content.contains(&entry) {
        return Ok(()); // already present
    }

    // Find the closing `]` of the members array and insert before it.
    let new_content = if let Some(members_start) = content.find("members = [") {
        // Find the matching `]`
        if let Some(bracket_pos) = content[members_start..].find(']') {
            let abs_bracket = members_start + bracket_pos;
            let before = &content[..abs_bracket];
            let after = &content[abs_bracket..];
            format!("{before}  {entry},\n{after}")
        } else {
            bail!("malformed workspace Cargo.toml: no closing ] for members");
        }
    } else {
        bail!("workspace Cargo.toml missing members = [...]");
    };

    std::fs::write(&cargo_path, new_content).context("failed to write workspace Cargo.toml")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Scaffold: ADR
// ---------------------------------------------------------------------------

/// Scaffold a new ADR under `docs/adr/`.
pub fn scaffold_adr(root: &Path, title: &str) -> Result<()> {
    validate_non_empty(title, "ADR title")?;

    let adr_dir = root.join("docs").join("adr");
    std::fs::create_dir_all(&adr_dir).context("failed to create docs/adr/")?;

    let next_num = next_adr_number(&adr_dir)?;
    let padded = format!("{:04}", next_num);
    let slug = slugify(title);
    let filename = format!("{padded}-{slug}.md");

    // Read template
    let template_path = adr_dir.join("TEMPLATE.md");
    let template = if template_path.exists() {
        std::fs::read_to_string(&template_path).context("failed to read ADR template")?
    } else {
        default_adr_template()
    };

    let content = template.replace("ADR-NNNN", &format!("ADR-{padded}"));
    let content = content.replace("<title>", title);

    std::fs::write(adr_dir.join(&filename), content)
        .with_context(|| format!("failed to write {filename}"))?;

    println!("scaffolded ADR: docs/adr/{filename}");
    Ok(())
}

/// Scan `docs/adr/` for the highest existing numeric prefix and return the next number.
pub fn next_adr_number(adr_dir: &Path) -> Result<u32> {
    let mut max: u32 = 0;
    if adr_dir.exists() {
        for entry in std::fs::read_dir(adr_dir).context("failed to read docs/adr/")? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // Match files like 0001-something.md
            if let Some(prefix) = name.split('-').next() {
                if let Ok(n) = prefix.parse::<u32>() {
                    if n > max {
                        max = n;
                    }
                }
            }
        }
    }
    Ok(max + 1)
}

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn default_adr_template() -> String {
    r#"# ADR-NNNN: <title>

## Status

Proposed

## Context

<!-- Why this decision is needed. -->

## Decision

<!-- What we decided. -->

## Consequences

<!-- What follows from this decision. -->

## Related Patterns

<!-- References to patterns from docs/patterns/catalog.md. -->
"#
    .to_string()
}

// ---------------------------------------------------------------------------
// Scaffold: scenario
// ---------------------------------------------------------------------------

/// Scaffold a new test scenario stub.
pub fn scaffold_scenario(root: &Path, name: &str, crate_name: &str) -> Result<()> {
    validate_non_empty(name, "scenario name")?;

    let tests_dir = root.join("crates").join(crate_name).join("tests");
    std::fs::create_dir_all(&tests_dir)
        .with_context(|| format!("failed to create {}", tests_dir.display()))?;

    let filename = format!("{}.rs", name.replace('-', "_"));
    let test_content = format!(
        r#"//! Scenario: {name}

#[test]
fn {fn_name}() {{
    // TODO: implement scenario
    todo!("implement {name} scenario");
}}
"#,
        fn_name = name.replace('-', "_")
    );

    std::fs::write(tests_dir.join(&filename), test_content)
        .with_context(|| format!("failed to write tests/{filename}"))?;

    // Add placeholder entry to scenario index
    append_scenario_index(root, name, crate_name)?;

    println!("scaffolded scenario: crates/{crate_name}/tests/{filename}");
    Ok(())
}

fn append_scenario_index(root: &Path, name: &str, crate_name: &str) -> Result<()> {
    let index_path = root.join("docs/scenarios/scenario_index.md");
    if !index_path.exists() {
        // Create a minimal index if it doesn't exist
        let dir = index_path.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        std::fs::write(&index_path, "# Scenario Index\n\n")?;
    }

    let entry = format!("\n| `{name}` | TODO | TODO | {crate_name} | — | TODO | — |\n");
    let mut content = std::fs::read_to_string(&index_path)?;
    content.push_str(&entry);
    std::fs::write(&index_path, content)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Scaffold: doc
// ---------------------------------------------------------------------------

/// Scaffold a new doc page in the mdBook site.
pub fn scaffold_doc(root: &Path, title: &str, section: &str) -> Result<()> {
    validate_non_empty(title, "doc title")?;
    validate_section(section)?;

    let section_dir = root.join("docs/book/src").join(section_to_dir(section));
    std::fs::create_dir_all(&section_dir)
        .with_context(|| format!("failed to create {}", section_dir.display()))?;

    let slug = slugify(title);
    let filename = format!("{slug}.md");
    let content = format!("# {title}\n\n<!-- TODO: write content -->\n");

    std::fs::write(section_dir.join(&filename), content)
        .with_context(|| format!("failed to write {filename}"))?;

    // Add entry to SUMMARY.md
    append_summary(root, title, section, &slug)?;

    println!(
        "scaffolded doc: docs/book/src/{}/{}",
        section_to_dir(section),
        filename
    );
    Ok(())
}

/// Map section name to directory name.
fn section_to_dir(section: &str) -> &str {
    match section {
        "tutorial" => "tutorials",
        "howto" => "howto",
        "explanation" => "explanations",
        "reference" => "reference",
        _ => section,
    }
}

/// Map section name to the heading used in SUMMARY.md.
fn section_to_heading(section: &str) -> &str {
    match section {
        "tutorial" => "Tutorials",
        "howto" => "How-To Guides",
        "explanation" => "Explanations",
        "reference" => "Reference",
        _ => section,
    }
}

fn append_summary(root: &Path, title: &str, section: &str, slug: &str) -> Result<()> {
    let summary_path = root.join("docs/book/src/SUMMARY.md");
    if !summary_path.exists() {
        bail!("SUMMARY.md not found at {}", summary_path.display());
    }

    let content = std::fs::read_to_string(&summary_path)?;
    let dir = section_to_dir(section);
    let entry = format!("- [{title}]({dir}/{slug}.md)");
    let heading = format!("## {}", section_to_heading(section));

    // Find the section heading and insert the entry after the last item in that section
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    let mut insert_pos = None;

    if let Some(heading_idx) = lines.iter().position(|l| l.trim() == heading) {
        // Find the last entry line in this section (before next ## or end)
        let mut last_entry = heading_idx;
        for (i, line) in lines.iter().enumerate().skip(heading_idx + 1) {
            if line.starts_with("## ") || line.starts_with("---") {
                break;
            }
            if line.starts_with("- [") {
                last_entry = i;
            }
        }
        insert_pos = Some(last_entry + 1);
    }

    if let Some(pos) = insert_pos {
        lines.insert(pos, entry);
    } else {
        // Section not found — append at end
        lines.push(String::new());
        lines.push(heading);
        lines.push(entry);
    }

    let new_content = lines.join("\n");
    // Ensure trailing newline
    let new_content = if new_content.ends_with('\n') {
        new_content
    } else {
        format!("{new_content}\n")
    };
    std::fs::write(&summary_path, new_content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_crate_names() {
        assert!(validate_crate_name("faultline-foo").is_ok());
        assert!(validate_crate_name("faultline-foo-bar").is_ok());
        assert!(validate_crate_name("faultline-a1").is_ok());
    }

    #[test]
    fn invalid_crate_names() {
        assert!(validate_crate_name("foo").is_err());
        assert!(validate_crate_name("faultline-").is_err());
        assert!(validate_crate_name("faultline-1bad").is_err());
        assert!(validate_crate_name("faultline-Foo").is_err());
        assert!(validate_crate_name("").is_err());
    }

    #[test]
    fn valid_sections() {
        for s in &["tutorial", "howto", "explanation", "reference"] {
            assert!(validate_section(s).is_ok());
        }
    }

    #[test]
    fn invalid_sections() {
        assert!(validate_section("tutorials").is_err());
        assert!(validate_section("").is_err());
        assert!(validate_section("blog").is_err());
    }

    #[test]
    fn empty_strings_rejected() {
        assert!(validate_non_empty("", "test").is_err());
        assert!(validate_non_empty("  ", "test").is_err());
        assert!(validate_non_empty("hello", "test").is_ok());
    }

    #[test]
    fn slugify_works() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("Use HTTP/2 Protocol"), "use-http-2-protocol");
    }
}
