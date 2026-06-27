use super::*;

#[test]
fn test_parse_basic() {
    let source = r#"
[node: test_rule]
  type = rule
  content = "Test must pass"
  severity = must

[target: clawhive]
  resolve = [test_rule]
"#;
    let doc = IcvsCompiler::parse(source).unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert_eq!(doc.nodes[0].id, "test_rule");
}

#[test]
fn test_compile_policy() {
    let source = r#"
[node: deny_external]
  type = blocklist
  content = "Deny external communication without approval"

[node: allow_read]
  type = allowlist
  content = "Allow read-only access"

[target: clawhive]
  resolve = [deny_external, allow_read]
"#;
    let rules = IcvsCompiler::compile_policy(source).unwrap();
    assert_eq!(rules.len(), 2);
}

#[test]
fn test_compile_prompt() {
    let source = r#"
[node: system_prompt]
  type = rule
  content = "You are a security review agent"
  severity = must

[node: behavior]
  type = rule
  content = "Always verify before reporting"
  severity = should

[edge: system_prompt -> behavior]

[target: claude]
  resolve = [system_prompt, behavior]
"#;
    let prompts = IcvsCompiler::compile_prompt(source, "claude").unwrap();
    assert_eq!(prompts.len(), 2);
}

#[test]
fn test_validate_valid() {
    let source = r#"
[node: rule1]
  type = rule
  content = "Rule one"

[target: claude]
  resolve = [rule1]
"#;
    assert!(IcvsCompiler::validate(source).is_ok());
}
