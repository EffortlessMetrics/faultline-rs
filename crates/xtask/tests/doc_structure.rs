// Feature: repo-operating-system, Property 37: Pattern Entry Structural Completeness
// Feature: repo-operating-system, Property 38: Scenario Entry Structural Completeness

use std::path::Path;

/// Workspace root relative to the xtask crate directory.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .unwrap()
        .parent() // repo root
        .unwrap()
}

// ---------------------------------------------------------------------------
// Property 37 — Pattern Entry Structural Completeness
// **Validates: Requirements 1.2**
//
// For any pattern entry in docs/patterns/catalog.md, the entry shall contain
// all five required sections: a one-sentence definition, a "when to use"
// section, a concrete example, at least one anti-example, and cross-references
// to related ADRs and scenarios.
// ---------------------------------------------------------------------------

/// Split the catalog into individual pattern sections (delimited by `## N. `).
fn parse_pattern_sections(content: &str) -> Vec<(String, String)> {
    let mut patterns: Vec<(String, String)> = Vec::new();
    let mut current_title = String::new();
    let mut current_body = String::new();
    let mut in_pattern = false;

    for line in content.lines() {
        // Pattern headers look like: ## 1. Truth Core / Translation Edge
        if line.starts_with("## ") && line.len() > 3 {
            let after_hash = line[3..].trim();
            // Check if it starts with a digit followed by a dot (pattern entry)
            if after_hash
                .chars()
                .next()
                .map_or(false, |c| c.is_ascii_digit())
                && after_hash.contains(". ")
            {
                // Save previous pattern if any
                if in_pattern {
                    patterns.push((current_title.clone(), current_body.clone()));
                }
                current_title = after_hash.to_string();
                current_body = String::new();
                in_pattern = true;
                continue;
            }
        }

        if in_pattern {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    // Don't forget the last pattern
    if in_pattern {
        patterns.push((current_title, current_body));
    }

    patterns
}

#[test]
fn pattern_entry_structural_completeness() {
    // Feature: repo-operating-system, Property 37: Pattern Entry Structural Completeness
    // **Validates: Requirements 1.2**

    let catalog_path = workspace_root().join("docs/patterns/catalog.md");
    let content = std::fs::read_to_string(&catalog_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", catalog_path.display()));

    let patterns = parse_pattern_sections(&content);

    // The catalog must contain at least the 10 required patterns
    assert!(
        patterns.len() >= 10,
        "Expected at least 10 patterns in catalog, found {}",
        patterns.len()
    );

    let required_sections: &[(&str, &[&str])] = &[
        ("Definition", &["**Definition:**"]),
        ("When to use", &["**When to use:**"]),
        ("Example", &["**Example:**"]),
        ("Anti-example", &["**Anti-example:**"]),
        (
            "Cross-references",
            &["**Related ADRs:**", "**Related Scenarios:**"],
        ),
    ];

    for (title, body) in &patterns {
        for (section_name, markers) in required_sections {
            let found = markers.iter().any(|marker| body.contains(marker));
            assert!(
                found,
                "Pattern '{title}' is missing required section: {section_name}\n\
                 Expected one of: {markers:?}\n\
                 Pattern body excerpt: {}",
                &body[..body.len().min(200)]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Property 38 — Scenario Entry Structural Completeness
// **Validates: Requirements 2.2**
//
// For any scenario entry in docs/scenarios/scenario_index.md, the entry shall
// contain all seven required fields: scenario name, problem description,
// fixture/generator, crate(s), artifact(s), invariant/property, and related
// references.
// ---------------------------------------------------------------------------

/// Parse markdown table data rows from the scenario index.
/// Returns a vec of (line_number, columns) for each data row.
fn parse_scenario_table_rows(content: &str) -> Vec<(usize, Vec<String>)> {
    let mut rows: Vec<(usize, Vec<String>)> = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip non-table lines
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            continue;
        }

        // Skip separator rows (e.g., |---|---|---|)
        let inner = &trimmed[1..trimmed.len() - 1];
        if inner
            .chars()
            .all(|c| c == '-' || c == '|' || c == ':' || c == ' ')
        {
            continue;
        }

        // Split by pipe, trim each cell
        let cells: Vec<String> = inner
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect();

        // Only consider 14-column tables (the enriched scenario tables with metadata)
        if cells.len() != 14 {
            continue;
        }

        // Skip header rows — they contain "Scenario" as first cell
        if cells.first().map_or(false, |c| c == "Scenario") {
            continue;
        }

        // Only include rows that look like data (first cell is non-empty)
        if cells.first().map_or(true, |c| c.is_empty()) {
            continue;
        }

        rows.push((line_idx + 1, cells));
    }

    rows
}

#[test]
fn scenario_entry_structural_completeness() {
    // Feature: repo-operating-system, Property 38: Scenario Entry Structural Completeness
    // **Validates: Requirements 2.2**

    let index_path = workspace_root().join("docs/scenarios/scenario_index.md");
    let content = std::fs::read_to_string(&index_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", index_path.display()));

    let rows = parse_scenario_table_rows(&content);

    // There should be a substantial number of scenario entries
    assert!(
        !rows.is_empty(),
        "No scenario table rows found in scenario_index.md"
    );

    let expected_columns = 14; // Scenario, Problem, Fixture/Generator, Crate(s), Artifact, Invariant, Refs, Tier, Req IDs, Contract, Mutation Surface, Crit, Owner, Review

    for (line_num, cells) in &rows {
        assert_eq!(
            cells.len(),
            expected_columns,
            "Line {line_num}: expected {expected_columns} columns, found {}. Row: {cells:?}",
            cells.len()
        );

        // Each of the first 7 fields must be non-empty (using "—" for intentionally blank is OK)
        let field_names = [
            "Scenario",
            "Problem",
            "Fixture/Generator",
            "Crate(s)",
            "Artifact",
            "Invariant",
            "Refs",
        ];

        for (i, field_name) in field_names.iter().enumerate() {
            assert!(
                !cells[i].is_empty(),
                "Line {line_num}: field '{field_name}' (column {}) is empty. Row: {cells:?}",
                i + 1
            );
        }

        // Metadata columns (7..14) must also be non-empty
        let metadata_names = [
            "Tier",
            "Req IDs",
            "Contract",
            "Mutation Surface",
            "Crit",
            "Owner",
            "Review",
        ];

        for (j, meta_name) in metadata_names.iter().enumerate() {
            let col = 7 + j;
            assert!(
                !cells[col].is_empty(),
                "Line {line_num}: metadata field '{meta_name}' (column {}) is empty. Row: {cells:?}",
                col + 1
            );
        }
    }
}
