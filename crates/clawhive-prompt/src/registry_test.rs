use super::*;

#[test]
fn test_new_registry_has_kernel() {
    let registry = IcvsPromptRegistry::new();
    let kernel = registry.get_kernel();
    assert!(!kernel.is_empty(), "kernel should have prompts");
    assert!(kernel.iter().any(|p| p.content.contains("logical agent")));
}

#[test]
fn test_new_registry_has_injection_prompt() {
    let registry = IcvsPromptRegistry::new();
    let injection = registry.get_injection_prompt();
    assert!(!injection.is_empty(), "injection prompt should be loaded");
}

#[test]
fn test_get_root_role_prompt() {
    let mut registry = IcvsPromptRegistry::new();
    let prompts = registry.get_role_prompt("root").unwrap();
    assert!(!prompts.is_empty());
    assert!(prompts.iter().any(|p| p.content.contains("Root Agent")));
}

#[test]
fn test_get_coding_role_prompt() {
    let mut registry = IcvsPromptRegistry::new();
    let prompts = registry.get_role_prompt("coding").unwrap();
    assert!(!prompts.is_empty());
    assert!(prompts.iter().any(|p| p.content.contains("Coding Agent")));
}

#[test]
fn test_role_prompt_caching() {
    let mut registry = IcvsPromptRegistry::new();
    let first = registry.get_role_prompt("root").unwrap();
    // Call again — should return cached version
    let second = registry.get_role_prompt("root").unwrap();
    assert_eq!(first.len(), second.len());
    assert_eq!(first[0].content, second[0].content);
}

#[test]
fn test_get_lifecycle_ephemeral_prompt() {
    let mut registry = IcvsPromptRegistry::new();
    let prompts = registry.get_lifecycle_prompt("ephemeral").unwrap();
    assert!(!prompts.is_empty());
    assert!(prompts.iter().any(|p| p.content.contains("ephemeral")));
}

#[test]
fn test_get_lifecycle_persistent_prompt() {
    let mut registry = IcvsPromptRegistry::new();
    let prompts = registry.get_lifecycle_prompt("persistent").unwrap();
    assert!(!prompts.is_empty());
    assert!(prompts.iter().any(|p| p.content.contains("persistent")));
}

#[test]
fn test_lifecycle_prompt_caching() {
    let mut registry = IcvsPromptRegistry::new();
    let first = registry.get_lifecycle_prompt("ephemeral").unwrap();
    let second = registry.get_lifecycle_prompt("ephemeral").unwrap();
    assert_eq!(first.len(), second.len());
    assert_eq!(first[0].content, second[0].content);
}

#[test]
fn test_get_multiple_roles() {
    let mut registry = IcvsPromptRegistry::new();
    for role in &["root", "director", "planner", "coding", "security_guardian"] {
        let prompts = registry.get_role_prompt(role).unwrap();
        assert!(!prompts.is_empty(), "role {} should have prompts", role);
    }
}

#[test]
fn test_get_invalid_role_returns_error() {
    let mut registry = IcvsPromptRegistry::new();
    let result = registry.get_role_prompt("nonexistent_role");
    assert!(result.is_err());
}

#[test]
fn test_get_invalid_lifecycle_returns_error() {
    let mut registry = IcvsPromptRegistry::new();
    let result = registry.get_lifecycle_prompt("invalid_mode");
    assert!(result.is_err());
}
