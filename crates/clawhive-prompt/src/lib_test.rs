use super::*;

#[test]
fn test_all_roles_have_temperature() {
    for role in AgentRole::all_variants() {
        let t = role.temperature();
        assert!(
            (0.0..=1.0).contains(&t),
            "role {:?} has out-of-range temperature {}",
            role,
            t
        );
    }
}

#[test]
fn test_all_roles_have_max_output_tokens() {
    for role in AgentRole::all_variants() {
        let t = role.max_output_tokens();
        assert!(t > 0 && t <= 16384, "role {:?} has out-of-range max_output_tokens {}", role, t);
    }
}

#[test]
fn test_all_roles_have_primary_output() {
    for role in AgentRole::all_variants() {
        let o = role.primary_output();
        assert!(!o.is_empty(), "role {:?} has empty primary_output", role);
    }
}

#[test]
fn test_agent_role_display() {
    assert_eq!(format!("{}", AgentRole::Root), "Root");
    assert_eq!(format!("{}", AgentRole::Coding), "Coding");
    assert_eq!(format!("{}", AgentRole::SecurityGuardian), "SecurityGuardian");
}

#[test]
fn test_agent_role_parse() {
    use std::str::FromStr;
    assert_eq!(AgentRole::from_str("Root").unwrap(), AgentRole::Root);
    assert_eq!(AgentRole::from_str("Coding").unwrap(), AgentRole::Coding);
    assert_eq!(AgentRole::from_str("MemoryCurator").unwrap(), AgentRole::MemoryCurator);
    assert!(AgentRole::from_str("Unknown").is_err());
}

#[test]
fn test_specific_role_properties() {
    // Root: low temperature, high output
    assert_eq!(AgentRole::Root.temperature(), 0.3);
    assert_eq!(AgentRole::Root.max_output_tokens(), 4096);
    assert_eq!(AgentRole::Root.primary_output(), "MissionProposal");

    // Coding: low temperature, high output
    assert_eq!(AgentRole::Coding.temperature(), 0.1);
    assert_eq!(AgentRole::Coding.max_output_tokens(), 8192);
    assert_eq!(AgentRole::Coding.primary_output(), "CodeChangeResult");

    // Critic: very low temperature, medium output
    assert_eq!(AgentRole::Critic.temperature(), 0.2);
    assert_eq!(AgentRole::Critic.max_output_tokens(), 2048);
    assert_eq!(AgentRole::Critic.primary_output(), "CritiqueReport");

    // SecurityGuardian: low temperature
    assert_eq!(AgentRole::SecurityGuardian.temperature(), 0.1);
    assert_eq!(AgentRole::SecurityGuardian.primary_output(), "SecurityAssessment");
}
