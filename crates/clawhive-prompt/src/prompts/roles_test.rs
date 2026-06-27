use super::*;
use clawhive_icvs::IcvsCompiler;

#[test]
fn test_all_role_prompts_parse() {
    let doc = IcvsCompiler::parse(ROLES_SOURCE).unwrap();
    assert!(doc.nodes.len() >= 22);
}

#[test]
fn test_compile_root_prompt() {
    let prompts = IcvsCompiler::compile_prompt(ROLES_SOURCE, "root").unwrap();
    assert!(!prompts.is_empty());
    assert!(prompts[0].content.contains("Root Agent"));
}

#[test]
fn test_compile_coding_prompt() {
    let prompts = IcvsCompiler::compile_prompt(ROLES_SOURCE, "coding").unwrap();
    assert!(!prompts.is_empty());
    assert!(prompts[0].content.contains("Coding Agent"));
}

#[test]
fn test_compile_security_guardian() {
    let prompts =
        IcvsCompiler::compile_prompt(ROLES_SOURCE, "security_guardian").unwrap();
    assert!(!prompts.is_empty());
    assert!(prompts[0].content.contains("Security Guardian"));
}

#[test]
fn test_all_role_targets_resolve() {
    let role_ids = [
        "root", "director", "planner", "orchestrator", "manager",
        "specialist", "research", "browser", "coding", "data",
        "communication", "device", "critic", "verifier", "judge",
        "security_guardian", "memory_curator", "skill_engineer",
        "cost_controller", "recovery", "watcher", "maintenance",
    ];
    for role in &role_ids {
        let prompts = IcvsCompiler::compile_prompt(ROLES_SOURCE, role).unwrap();
        assert!(!prompts.is_empty(), "role {role} should resolve");
    }
}
