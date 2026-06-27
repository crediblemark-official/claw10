use axum::Json;
use serde_json::{json, Value};

pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "clawhive-control-api",
        "version": "0.1.0"
    }))
}
