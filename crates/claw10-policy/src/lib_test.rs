use super::*;
use claw10_domain::PolicySubject;

#[test]
fn test_subject_matches_role_wildcard() {
    let rule = PolicySubject::Role("*".to_string());

    // Should match anything
    assert!(PolicyService::subject_matches(&rule, &PolicySubject::Role("admin".to_string())));
    assert!(PolicyService::subject_matches(&rule, &PolicySubject::Agent("agent-1".to_string())));
    assert!(PolicyService::subject_matches(&rule, &PolicySubject::Tenant("tenant-a".to_string())));
}

#[test]
fn test_subject_matches_exact_match() {
    assert!(PolicyService::subject_matches(
        &PolicySubject::Role("admin".to_string()),
        &PolicySubject::Role("admin".to_string())
    ));
    assert!(PolicyService::subject_matches(
        &PolicySubject::Agent("agent-1".to_string()),
        &PolicySubject::Agent("agent-1".to_string())
    ));
    assert!(PolicyService::subject_matches(
        &PolicySubject::Organization("org-a".to_string()),
        &PolicySubject::Organization("org-a".to_string())
    ));
    assert!(PolicyService::subject_matches(
        &PolicySubject::Department("dept-x".to_string()),
        &PolicySubject::Department("dept-x".to_string())
    ));
    assert!(PolicyService::subject_matches(
        &PolicySubject::Mission("mission-1".to_string()),
        &PolicySubject::Mission("mission-1".to_string())
    ));
    assert!(PolicyService::subject_matches(
        &PolicySubject::Task("task-1".to_string()),
        &PolicySubject::Task("task-1".to_string())
    ));
    assert!(PolicyService::subject_matches(
        &PolicySubject::Tool("tool-1".to_string()),
        &PolicySubject::Tool("tool-1".to_string())
    ));
    assert!(PolicyService::subject_matches(
        &PolicySubject::Worker("worker-1".to_string()),
        &PolicySubject::Worker("worker-1".to_string())
    ));
    assert!(PolicyService::subject_matches(
        &PolicySubject::Tenant("tenant-a".to_string()),
        &PolicySubject::Tenant("tenant-a".to_string())
    ));
    assert!(PolicyService::subject_matches(
        &PolicySubject::DataClass("class-1".to_string()),
        &PolicySubject::DataClass("class-1".to_string())
    ));
}

#[test]
fn test_subject_matches_different_value() {
    assert!(!PolicyService::subject_matches(
        &PolicySubject::Role("admin".to_string()),
        &PolicySubject::Role("user".to_string())
    ));
    assert!(!PolicyService::subject_matches(
        &PolicySubject::Agent("agent-1".to_string()),
        &PolicySubject::Agent("agent-2".to_string())
    ));
}

#[test]
fn test_subject_matches_different_type() {
    assert!(!PolicyService::subject_matches(
        &PolicySubject::Agent("admin".to_string()),
        &PolicySubject::Role("admin".to_string())
    ));
}
