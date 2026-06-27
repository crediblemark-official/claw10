use super::*;

#[test]
fn test_record_trace() {
    let trace = PromptTracer::record(
        "bundle-1",
        "agent-1",
        "Root",
        "ephemeral",
        "mission-1",
        "task-1",
        "1.0.0",
        4,
        "TOON",
        "policy-1",
        "abc123",
        2048,
        "MissionProposal",
    );
    assert_eq!(trace.agent_id, "agent-1");
    assert_eq!(trace.trace_id.len(), 16);
}
