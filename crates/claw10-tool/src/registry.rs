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
    use claw10_domain::SideEffectClass;

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
            serde_json::json!({})
        }

        fn categories(&self) -> Vec<&str> {
            vec!["test"]
        }

        fn side_effect_class(&self) -> SideEffectClass {
            SideEffectClass::ReadOnly
        }

        async fn execute(
            &self,
            _context: &ToolContext,
            _args: serde_json::Value,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::ok(serde_json::json!("success")))
        }
    }

    #[test]
    fn test_list_names_empty() {
        let registry = ToolRegistry::new();
        let names = registry.list_names();
        assert!(names.is_empty());
    }

    #[test]
    fn test_list_names_with_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool { name: "tool1".to_string() }));
        registry.register(Box::new(MockTool { name: "tool2".to_string() }));
        registry.register(Box::new(MockTool { name: "tool3".to_string() }));

        let mut names = registry.list_names();
        names.sort_unstable(); // Sort for deterministic comparison

        assert_eq!(names, vec!["tool1", "tool2", "tool3"]);
    }
}
