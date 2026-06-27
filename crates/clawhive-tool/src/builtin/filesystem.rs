use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::Tool;
use crate::result::ToolOutput;
use clawhive_domain::SideEffectClass;

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file"
                }
            },
            "required": ["path"]
        })
    }

    fn categories(&self) -> Vec<&str> {
        vec!["filesystem"]
    }

    fn side_effect_class(&self) -> SideEffectClass {
        SideEffectClass::ReadOnly
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("path is required".into()))?;

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("cannot read file '{}': {}", path, e)))?;

        Ok(ToolOutput::ok(json!({
            "path": path,
            "content": content,
            "size_bytes": content.len(),
        })))
    }
}

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file at the given path"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write"
                },
                "append": {
                    "type": "boolean",
                    "description": "Append instead of overwrite"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn categories(&self) -> Vec<&str> {
        vec!["filesystem"]
    }

    fn side_effect_class(&self) -> SideEffectClass {
        SideEffectClass::ReversibleWrite
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("path is required".into()))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("content is required".into()))?;
        let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);

        let parent = std::path::Path::new(path).parent();
        if let Some(dir) = parent {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("cannot create directory: {}", e)))?;
        }

        if append {
            let mut file = tokio::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("cannot open file: {}", e)))?;
            tokio::io::AsyncWriteExt::write_all(&mut file, content.as_bytes())
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("cannot write file: {}", e)))?;
        } else {
            tokio::fs::write(path, content)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("cannot write file: {}", e)))?;
        }

        Ok(ToolOutput::ok(json!({
            "path": path,
            "size_bytes": content.len(),
            "append": append,
        })))
    }
}
