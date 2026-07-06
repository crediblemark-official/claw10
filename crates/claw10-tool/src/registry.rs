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
    use serde_json::json;

    struct DummyTool {
        name: String,
        categories: Vec<&'static str>,
    }

    impl DummyTool {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                categories: vec![],
            }
        }
    }

    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A dummy tool for testing"
        }

        fn input_schema(&self) -> serde_json::Value {
            json!({})
        }

        fn categories(&self) -> Vec<&str> {
            self.categories.clone()
        }

        fn side_effect_class(&self) -> claw10_domain::SideEffectClass {
            claw10_domain::SideEffectClass::ReadOnly
        }

        async fn execute(
            &self,
            _context: &ToolContext,
            _args: serde_json::Value,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::ok(json!({"success": true})))
        }
    }

    #[test]
    fn test_list_empty() {
        let registry = ToolRegistry::new();
        let tools = registry.list();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_list_multiple_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(DummyTool::new("tool1")));
        registry.register(Box::new(DummyTool::new("tool2")));

        let tools = registry.list();
        assert_eq!(tools.len(), 2);

        let mut tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        tool_names.sort_unstable();

        assert_eq!(tool_names, vec!["tool1", "tool2"]);
    }

    #[test]
    fn test_list_names_empty() {
        let registry = ToolRegistry::new();
        let names = registry.list_names();
        assert!(names.is_empty());
    }

    #[test]
    fn test_list_names_multiple_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(DummyTool::new("tool1")));
        registry.register(Box::new(DummyTool::new("tool2")));

        let mut names = registry.list_names();
        assert_eq!(names.len(), 2);

        names.sort_unstable();
        assert_eq!(names, vec!["tool1", "tool2"]);
    }
}
