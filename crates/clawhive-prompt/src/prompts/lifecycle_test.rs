use super::*;
use clawhive_icvs::IcvsCompiler;

#[test]
fn test_ephemeral_compiles() {
    let prompts = IcvsCompiler::compile_prompt(LIFECYCLE_SOURCE, "ephemeral").unwrap();
    assert!(!prompts.is_empty());
    assert!(prompts[0].content.contains("ephemeral agent"));
}

#[test]
fn test_persistent_compiles() {
    let prompts = IcvsCompiler::compile_prompt(LIFECYCLE_SOURCE, "persistent").unwrap();
    assert!(!prompts.is_empty());
    assert!(prompts[0].content.contains("persistent logical agent"));
}

#[test]
fn test_lifecycle_parses() {
    let doc = IcvsCompiler::parse(LIFECYCLE_SOURCE).unwrap();
    assert_eq!(doc.nodes.len(), 2);
}
