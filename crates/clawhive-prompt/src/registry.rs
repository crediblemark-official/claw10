use std::collections::HashMap;

use clawhive_icvs::{AgentPrompt, IcvsCompiler};

use crate::prompts::{base_kernel, injection, lifecycle, roles};

pub struct IcvsPromptRegistry {
    kernel_cache: Vec<AgentPrompt>,
    role_cache: HashMap<String, Vec<AgentPrompt>>,
    lifecycle_cache: HashMap<String, Vec<AgentPrompt>>,
    injection_cache: Vec<AgentPrompt>,
}

impl IcvsPromptRegistry {
    #[must_use]
    pub fn new() -> Self {
        let kernel_cache = IcvsCompiler::compile_prompt(
            base_kernel::BASE_KERNEL_SOURCE,
            "base_kernel",
        )
        .unwrap_or_default();

        let injection_cache = IcvsCompiler::compile_prompt(
            injection::INJECTION_SOURCE,
            "injection_safety",
        )
        .unwrap_or_default();

        Self {
            kernel_cache,
            role_cache: HashMap::new(),
            lifecycle_cache: HashMap::new(),
            injection_cache,
        }
    }

    pub fn get_kernel(&self) -> &[AgentPrompt] {
        &self.kernel_cache
    }

    pub fn get_role_prompt(&mut self, role: &str) -> Result<Vec<AgentPrompt>, clawhive_icvs::IcvsError> {
        if let Some(cached) = self.role_cache.get(role) {
            return Ok(cached.clone());
        }
        let prompts = IcvsCompiler::compile_prompt(roles::ROLES_SOURCE, role)?;
        let result = prompts.clone();
        self.role_cache.insert(role.to_string(), prompts);
        Ok(result)
    }

    pub fn get_lifecycle_prompt(
        &mut self,
        lifecycle: &str,
    ) -> Result<Vec<AgentPrompt>, clawhive_icvs::IcvsError> {
        if let Some(cached) = self.lifecycle_cache.get(lifecycle) {
            return Ok(cached.clone());
        }
        let prompts = IcvsCompiler::compile_prompt(lifecycle::LIFECYCLE_SOURCE, lifecycle)?;
        let result = prompts.clone();
        self.lifecycle_cache.insert(lifecycle.to_string(), prompts);
        Ok(result)
    }

    #[must_use]
    pub fn get_injection_prompt(&self) -> &[AgentPrompt] {
        &self.injection_cache
    }
}

impl Default for IcvsPromptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "registry_test.rs"]
mod tests;

