use super::*;
use clawhive_icvs::IcvsCompiler;

#[test]
fn test_base_kernel_compiles() {
    let prompts = IcvsCompiler::compile_prompt(BASE_KERNEL_SOURCE, "base_kernel").unwrap();
    assert!(!prompts.is_empty());
    assert!(prompts[0].content.contains("logical agent"));
}

#[test]
fn test_base_kernel_parses() {
    let doc = IcvsCompiler::parse(BASE_KERNEL_SOURCE).unwrap();
    assert_eq!(doc.nodes.len(), 1);
}
