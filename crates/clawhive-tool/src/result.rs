use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
    pub execution_time_ms: u64,
    pub cost_usd: f64,
}

impl ToolOutput {
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data,
            error: None,
            execution_time_ms: 0,
            cost_usd: 0.0,
        }
    }

    pub fn ok_with_cost(data: serde_json::Value, cost_usd: f64) -> Self {
        Self {
            success: true,
            data,
            error: None,
            execution_time_ms: 0,
            cost_usd,
        }
    }

    pub fn fail(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: serde_json::Value::Null,
            error: Some(error.into()),
            execution_time_ms: 0,
            cost_usd: 0.0,
        }
    }
}
