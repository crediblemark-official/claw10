use std::collections::HashMap;

use async_trait::async_trait;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::result::ToolOutput;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn input_schema(&self) -> serde_json::Value;

    fn categories(&self) -> Vec<&str>;

    fn side_effect_class(&self) -> claw10_domain::SideEffectClass;

    async fn execute(
        &self,
        context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        tracing::info!("registering tool: {}", tool.name());
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Result<&dyn Tool, ToolError> {
        self.tools
            .get(name)
            .map(Box::as_ref)
            .ok_or_else(|| ToolError::ToolNotFound(name.to_string()))
    }

    #[must_use]
    pub fn list(&self) -> Vec<&dyn Tool> {
        self.tools
            .values()
            .map(std::convert::AsRef::as_ref)
            .collect()
    }

    #[must_use]
    pub fn list_names(&self) -> Vec<&str> {
        self.tools.keys().map(std::string::String::as_str).collect()
    }

    #[must_use]
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    #[must_use]
    pub fn tools_for_categories(&self, categories: &[&str]) -> Vec<&dyn Tool> {
        self.tools
            .values()
            .filter(|t| {
                let tool_cats = t.categories();
                categories.iter().any(|c| tool_cats.contains(c))
            })
            .map(std::convert::AsRef::as_ref)
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use claw10_domain::SideEffectClass;
    use serde_json::json;

    struct MockTool {
        name: String,
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A mock tool for testing"
        }

        fn input_schema(&self) -> serde_json::Value {
            json!({})
        }

        fn categories(&self) -> Vec<&str> {
            vec![]
        }

        fn side_effect_class(&self) -> SideEffectClass {
            SideEffectClass::ReadOnly
        }

        async fn execute(
            &self,
            _context: &ToolContext,
            _args: serde_json::Value,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::ok(json!("success")))
        }
    }

    #[test]
    fn test_registry_new_is_empty() {
        let registry = ToolRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_register_tool() {
        let mut registry = ToolRegistry::new();
        let tool = MockTool {
            name: "test_tool".to_string(),
        };

        registry.register(Box::new(tool));

        assert!(registry.has_tool("test_tool"));
        let retrieved = registry.get("test_tool").unwrap();
        assert_eq!(retrieved.name(), "test_tool");
    }
}
