use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct SchemaValidationOutcome {
    pub valid: bool,
    pub errors: Vec<String>,
}

pub struct SchemaValidator;

impl SchemaValidator {
    #[must_use]
    pub fn validate(value: &Value, schema: &Value) -> SchemaValidationOutcome {
        match validate_value(value, schema) {
            Ok(()) => SchemaValidationOutcome {
                valid: true,
                errors: Vec::new(),
            },
            Err(errors) => SchemaValidationOutcome {
                valid: false,
                errors,
            },
        }
    }
}

fn validate_value(value: &Value, schema: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if let Some(schema_type) = schema.get("type").and_then(Value::as_str) {
        match schema_type {
            "object" => {
                if !value.is_object() {
                    errors.push(format!("expected object, got {value:?}"));
                    return Err(errors);
                }
                if let Some(required) = schema.get("required").and_then(Value::as_array) {
                    let obj = value.as_object().unwrap();
                    for field in required {
                        let field_name = field.as_str().unwrap_or("");
                        if !obj.contains_key(field_name) {
                            errors.push(format!("missing required field: {field_name}"));
                        }
                    }
                }
                if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                    for (key, prop_schema) in properties {
                        if let Some(val) = value.get(key) {
                            validate_value(val, prop_schema).unwrap_or_else(|e| errors.extend(e));
                        }
                    }
                }
            }
            "array" => {
                if !value.is_array() {
                    errors.push(format!("expected array, got {value:?}"));
                    return Err(errors);
                }
                if let Some(items) = schema.get("items") {
                    let arr = value.as_array().unwrap();
                    for (i, item) in arr.iter().enumerate() {
                        validate_value(item, items).unwrap_or_else(|e| {
                            errors.extend(e.into_iter().map(|msg| format!("[{i}] {msg}")));
                        });
                    }
                }
            }
            "string" => {
                if !value.is_string() {
                    errors.push(format!("expected string, got {value:?}"));
                }
                if let Some(enum_values) = schema.get("enum").and_then(Value::as_array)
                    && !enum_values.iter().any(|e| e == value) {
                        errors.push(format!(
                            "expected one of {enum_values:?}, got {value:?}"
                        ));
                    }
            }
            "number" | "integer" => {
                if !value.is_number() {
                    errors.push(format!("expected number, got {value:?}"));
                }
            }
            "boolean" => {
                if !value.is_boolean() {
                    errors.push(format!("expected boolean, got {value:?}"));
                }
            }
            _ => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[async_trait]
pub trait SemanticValidator: Send + Sync {
    async fn validate(&self, response: &Value, context: &SemanticValidationContext) -> SemanticValidationOutcome;
}

pub struct SemanticValidationContext {
    pub agent_role: String,
    pub objective: String,
    pub mission_context: String,
}

pub struct SemanticValidationOutcome {
    pub valid: bool,
    pub risks: Vec<String>,
}

pub struct DefaultSemanticValidator;

#[async_trait]
impl SemanticValidator for DefaultSemanticValidator {
    async fn validate(
        &self,
        response: &Value,
        _context: &SemanticValidationContext,
    ) -> SemanticValidationOutcome {
        let mut risks = Vec::new();

        if let Some(confidence) = response.get("confidence").and_then(Value::as_f64)
            && confidence < 0.3 {
                risks.push(format!("low confidence: {confidence}"));
            }

        SemanticValidationOutcome {
            valid: risks.is_empty(),
            risks,
        }
    }
}

#[cfg(test)]
#[path = "validation_test.rs"]
mod tests;

