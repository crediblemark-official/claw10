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

    fn side_effect_class(&self) -> clawhive_domain::SideEffectClass;

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
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        tracing::info!("registering tool: {}", tool.name());
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Result<&Box<dyn Tool>, ToolError> {
        self.tools
            .get(name)
            .ok_or_else(|| ToolError::ToolNotFound(name.to_string()))
    }

    pub fn list(&self) -> Vec<&dyn Tool> {
        self.tools.values().map(|t| t.as_ref()).collect()
    }

    pub fn list_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    pub fn tools_for_categories(&self, categories: &[&str]) -> Vec<&dyn Tool> {
        self.tools
            .values()
            .filter(|t| {
                let tool_cats = t.categories();
                categories
                    .iter()
                    .any(|c| tool_cats.iter().any(|tc| *tc == *c))
            })
            .map(|t| t.as_ref())
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
