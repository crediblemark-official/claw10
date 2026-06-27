use super::*;
use crate::bundle::PolicyIrInput;

#[test]
fn test_empty_policy() {
    let input = PolicyIrInput {
        id: "policy-1".to_string(),
        hash: "abc".to_string(),
        rules: String::new(),
    };
    let digest = PolicyDigestBuilder::build(&input);
    assert!(digest.contains("No active policy"));
}

#[test]
fn test_policy_with_rules() {
    let input = PolicyIrInput {
        id: "policy-1".to_string(),
        hash: "abc".to_string(),
        rules: r#"
[node: deny_external]
  type = blocklist
  content = "Deny external communication without approval"

[target: test]
  resolve = [deny_external]
"#
        .to_string(),
    };
    let digest = PolicyDigestBuilder::build(&input);
    assert!(digest.contains("deny"));
    assert!(digest.contains("Total rules"));
}
