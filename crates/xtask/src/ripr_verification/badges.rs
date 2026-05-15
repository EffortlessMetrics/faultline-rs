use crate::run::run_output_owned;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;

const BADGE_JSON: &str = "badges/ripr.json";
const GENERATED_BADGE_JSON: &str = "target/xtask/badges/ripr.json";
const SHIELDS_SCHEMA: &str = "schemas/badges/shields-endpoint.schema.json";

pub(crate) fn badges(check: bool) -> Result<(), String> {
    let root = super::repo_root()?;
    let generated = generate_ripr_badge(&root)?;
    validate_shields_endpoint(&root, &generated)?;

    if check {
        check_badge(&root, &generated)
    } else {
        write_badge(&root, &generated)
    }
}

fn generate_ripr_badge(root: &Path) -> Result<String, String> {
    let output = run_output_owned(
        ripr_binary()?.as_str(),
        &[
            "check".to_string(),
            "--root".to_string(),
            root.display().to_string(),
            "--mode".to_string(),
            "ready".to_string(),
            "--format".to_string(),
            "repo-badge-shields".to_string(),
        ],
    )?;
    let value: Value = serde_json::from_str(&output)
        .map_err(|err| format!("ripr badge output was not valid JSON: {err}"))?;
    serde_json::to_string_pretty(&value).map_err(|err| format!("serialize badge JSON: {err}"))
}

fn ripr_binary() -> Result<String, String> {
    match env::var("RIPR_BIN") {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) => Err("RIPR_BIN is set but empty".to_string()),
        Err(_) => Ok("ripr".to_string()),
    }
}

fn check_badge(root: &Path, generated: &str) -> Result<(), String> {
    write_generated(root, generated)?;
    let expected = format!("{generated}\n");
    let actual_path = root.join(BADGE_JSON);
    let actual = fs::read_to_string(&actual_path)
        .map_err(|err| format!("missing or unreadable {BADGE_JSON}: {err}"))?;
    if actual == expected {
        println!("RIPR badge endpoint ok: {BADGE_JSON}");
        Ok(())
    } else {
        Err(format!("{BADGE_JSON} is stale; run `cargo xtask badges`"))
    }
}

fn write_badge(root: &Path, generated: &str) -> Result<(), String> {
    write_generated(root, generated)?;
    let path = root.join(BADGE_JSON);
    ensure_parent(&path)?;
    fs::write(&path, format!("{generated}\n"))
        .map_err(|err| format!("write {BADGE_JSON}: {err}"))?;
    println!("Wrote {BADGE_JSON}");
    println!("Wrote {GENERATED_BADGE_JSON}");
    Ok(())
}

fn write_generated(root: &Path, generated: &str) -> Result<(), String> {
    let path = root.join(GENERATED_BADGE_JSON);
    ensure_parent(&path)?;
    fs::write(&path, format!("{generated}\n"))
        .map_err(|err| format!("write {GENERATED_BADGE_JSON}: {err}"))
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("{} has no parent", path.display()));
    };
    fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))
}

fn validate_shields_endpoint(root: &Path, text: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|err| format!("generated badge is not valid JSON: {err}"))?;
    let schema: Value = serde_json::from_str(
        &fs::read_to_string(root.join(SHIELDS_SCHEMA))
            .map_err(|err| format!("read {SHIELDS_SCHEMA}: {err}"))?,
    )
    .map_err(|err| format!("parse {SHIELDS_SCHEMA}: {err}"))?;

    for field in ["schemaVersion", "label", "message", "color"] {
        if value.get(field).is_none() {
            return Err(format!("generated badge is missing `{field}`"));
        }
    }
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err("generated badge schemaVersion must be 1".to_string());
    }
    for field in ["label", "message", "color"] {
        if !value.get(field).is_some_and(Value::is_string) {
            return Err(format!("generated badge `{field}` must be a string"));
        }
    }
    if !schema.is_object() {
        return Err(format!("{SHIELDS_SCHEMA} must be a JSON object"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_minimal_shields_endpoint() -> Result<(), String> {
        let root = crate::ripr_verification::repo_root()?;
        validate_shields_endpoint(
            &root,
            r#"{
  "schemaVersion": 1,
  "label": "ripr",
  "message": "0",
  "color": "brightgreen"
}"#,
        )
    }

    #[test]
    fn rejects_missing_badge_field() -> Result<(), String> {
        let root = crate::ripr_verification::repo_root()?;
        let err = validate_shields_endpoint(
            &root,
            r#"{
  "schemaVersion": 1,
  "label": "ripr",
  "message": "0"
}"#,
        )
        .expect_err("missing color should fail");
        assert!(err.contains("color"));
        Ok(())
    }

    #[test]
    fn default_ripr_binary_is_path_binary() {
        assert_eq!(ripr_binary().as_deref(), Ok("ripr"));
    }
}
