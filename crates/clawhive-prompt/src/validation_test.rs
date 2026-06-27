use super::*;
use serde_json::json;

#[test]
fn test_validate_object_missing_field() {
    let value = json!({"name": "test"});
    let schema = json!({
        "type": "object",
        "required": ["name", "id"],
        "properties": {
            "name": {"type": "string"},
            "id": {"type": "string"}
        }
    });
    let outcome = SchemaValidator::validate(&value, &schema);
    assert!(!outcome.valid);
    assert!(outcome.errors.iter().any(|e| e.contains("id")));
}

#[test]
fn test_validate_object_valid() {
    let value = json!({"name": "test", "id": "123"});
    let schema = json!({
        "type": "object",
        "required": ["name", "id"],
        "properties": {
            "name": {"type": "string"},
            "id": {"type": "string"}
        }
    });
    let outcome = SchemaValidator::validate(&value, &schema);
    assert!(outcome.valid);
}

#[test]
fn test_validate_enum() {
    let value = json!("accepted");
    let schema = json!({
        "type": "string",
        "enum": ["accepted", "rejected"]
    });
    let outcome = SchemaValidator::validate(&value, &schema);
    assert!(outcome.valid);

    let value = json!("pending");
    let outcome = SchemaValidator::validate(&value, &schema);
    assert!(!outcome.valid);
}
