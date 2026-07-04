use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::Tool;
use crate::result::ToolOutput;
use claw10_domain::SideEffectClass;

/// Tool untuk mendaftarkan file yang dihasilkan agen sebagai artifact resmi.
pub struct DeclareArtifactTool {
    kv_store: Arc<dyn claw10_store::Store>,
}

impl DeclareArtifactTool {
    #[must_use]
    pub fn new(kv_store: Arc<dyn claw10_store::Store>) -> Self {
        Self { kv_store }
    }
}

#[async_trait]
impl Tool for DeclareArtifactTool {
    fn name(&self) -> &'static str {
        "declare_artifact"
    }

    fn description(&self) -> &'static str {
        "Declare a generated file as an official, integrity-hashed artifact in the workspace"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file to register"
                },
                "name": {
                    "type": "string",
                    "description": "Descriptive name for the artifact (e.g., 'final_report.md')"
                },
                "mime_type": {
                    "type": "string",
                    "description": "MIME type of the file (default: 'text/plain')"
                }
            },
            "required": ["path", "name"]
        })
    }

    fn categories(&self) -> Vec<&str> {
        vec!["artifacts", "storage"]
    }

    fn side_effect_class(&self) -> SideEffectClass {
        SideEffectClass::ControlledWrite
    }

    async fn execute(
        &self,
        context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("path is required".into()))?;
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("name is required".into()))?;
        let mime_type = args
            .get("mime_type")
            .and_then(|v| v.as_str())
            .unwrap_or("text/plain");

        // 1. Baca isi file secara asinkron
        let content = tokio::fs::read(path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("cannot read file '{path}': {e}")))?;

        // 2. Simpan sebagai Artifact menggunakan ArtifactService
        let artifact_service = claw10_artifact::ArtifactService::new(Arc::clone(&self.kv_store));
        let artifact = artifact_service
            .store_artifact(
                context.task_id.clone(),
                context.agent_id.clone(),
                name.to_string(),
                mime_type.to_string(),
                content,
            )
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("cannot store artifact: {e}")))?;

        Ok(ToolOutput::ok(json!({
            "artifact_id": artifact.id.0.to_string(),
            "name": artifact.name,
            "size_bytes": artifact.size_bytes,
            "content_hash": artifact.content_hash,
            "storage_path": artifact.storage_path,
            "status": "RegisteredSuccessfully"
        })))
    }
}
