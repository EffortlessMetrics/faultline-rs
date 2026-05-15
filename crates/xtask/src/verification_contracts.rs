use serde_json::Value;
use std::fs;
use std::path::Path;

pub(crate) fn validate_json_file_against_schema(
    root: &Path,
    value_path: &str,
    schema_path: &str,
) -> Result<(), String> {
    let schema = read_json(&root.join(schema_path))?;
    let value = read_json(&root.join(value_path))?;
    let mut violations = Vec::new();
    validate_value_against_schema(
        &value,
        &schema,
        &schema,
        value_path.to_string(),
        &mut violations,
    );
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{value_path} does not match {schema_path}:\n{}",
            violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

fn validate_value_against_schema(
    value: &Value,
    schema: &Value,
    root_schema: &Value,
    location: String,
    violations: &mut Vec<String>,
) {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        if let Some(resolved) = reference
            .strip_prefix('#')
            .and_then(|pointer| root_schema.pointer(pointer))
        {
            validate_value_against_schema(value, resolved, root_schema, location, violations);
        } else {
            violations.push(format!(
                "{location}: unresolved schema reference {reference}"
            ));
        }
        return;
    }

    if let Some(expected) = schema.get("const")
        && value != expected
    {
        violations.push(format!(
            "{location}: expected const {}, got {}",
            compact_json(expected),
            compact_json(value)
        ));
    }

    if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
        && !allowed.iter().any(|candidate| candidate == value)
    {
        violations.push(format!(
            "{location}: value {} is not in enum",
            compact_json(value)
        ));
    }

    if let Some(schema_type) = schema.get("type") {
        validate_type(value, schema_type, &location, violations);
    }

    if let (Some(object), Some(properties)) = (
        value.as_object(),
        schema.get("properties").and_then(Value::as_object),
    ) {
        for field in string_array(schema.get("required")) {
            if !object.contains_key(&field) {
                violations.push(format!("{location}: missing required field `{field}`"));
            }
        }
        if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
            for field in object.keys() {
                if !properties.contains_key(field) {
                    violations.push(format!("{location}: unexpected field `{field}`"));
                }
            }
        }
        for (field, field_schema) in properties {
            if let Some(field_value) = object.get(field) {
                validate_value_against_schema(
                    field_value,
                    field_schema,
                    root_schema,
                    format!("{location}.{field}"),
                    violations,
                );
            }
        }
    }

    if let (Some(items), Some(items_schema)) = (value.as_array(), schema.get("items")) {
        for (index, item) in items.iter().enumerate() {
            validate_value_against_schema(
                item,
                items_schema,
                root_schema,
                format!("{location}[{index}]"),
                violations,
            );
        }
    }
}

fn validate_type(value: &Value, schema_type: &Value, location: &str, violations: &mut Vec<String>) {
    let allowed = match schema_type {
        Value::String(text) => vec![text.as_str()],
        Value::Array(values) => values.iter().filter_map(Value::as_str).collect(),
        _ => {
            violations.push(format!("{location}: schema type must be string or array"));
            return;
        }
    };
    if !allowed
        .iter()
        .any(|allowed_type| value_matches_type(value, allowed_type))
    {
        violations.push(format!(
            "{location}: expected type {}, got {}",
            allowed.join("|"),
            value_type(value)
        ));
    }
}

fn value_matches_type(value: &Value, expected_type: &str) -> bool {
    match expected_type {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => {
            value.as_i64().is_some()
                || value
                    .as_u64()
                    .is_some_and(|number| i64::try_from(number).is_ok())
        }
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_required_fields_and_types() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["schema_version"],
            "additionalProperties": false,
            "properties": {
                "schema_version": {"type": "string"}
            }
        });
        let mut violations = Vec::new();
        validate_value_against_schema(
            &serde_json::json!({"schema_version": 1}),
            &schema,
            &schema,
            "sample".to_string(),
            &mut violations,
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("sample.schema_version"))
        );
    }
}
