use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::Tool;
use crate::result::ToolOutput;
use clawhive_domain::SideEffectClass;

pub struct HttpTool;

#[async_trait]
impl Tool for HttpTool {
    fn name(&self) -> &str {
        "http"
    }

    fn description(&self) -> &str {
        "Make an HTTP request to a URL"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "DELETE", "PATCH"],
                    "default": "GET"
                },
                "url": {
                    "type": "string",
                    "description": "Target URL"
                },
                "headers": {
                    "type": "object",
                    "description": "HTTP headers"
                },
                "body": {
                    "type": "string",
                    "description": "Request body"
                },
                "timeout_seconds": {
                    "type": "integer",
                    "default": 30
                }
            },
            "required": ["url"]
        })
    }

    fn categories(&self) -> Vec<&str> {
        vec!["http"]
    }

    fn side_effect_class(&self) -> SideEffectClass {
        SideEffectClass::ExternalCommunication
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("url is required".into()))?;
        let method = args
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_uppercase();
        let timeout = args
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout))
            .build()
            .map_err(|e| ToolError::ExecutionFailed(format!("http client error: {}", e)))?;

        let req = match method.as_str() {
            "GET" => client.get(url),
            "POST" => {
                let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");
                client.post(url).body(body.to_string())
            }
            "PUT" => {
                let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");
                client.put(url).body(body.to_string())
            }
            "DELETE" => client.delete(url),
            "PATCH" => {
                let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");
                client.patch(url).body(body.to_string())
            }
            _ => return Err(ToolError::InvalidArguments(format!("unsupported method: {}", method))),
        };

        let response = req
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("http request failed: {}", e)))?;

        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<binary body>".into());

        let response_headers: serde_json::Map<String, serde_json::Value> = headers
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    json!(v.to_str().unwrap_or("<invalid>")),
                )
            })
            .collect();

        Ok(ToolOutput::ok(json!({
            "status": status,
            "headers": response_headers,
            "body": body,
        })))
    }
}
