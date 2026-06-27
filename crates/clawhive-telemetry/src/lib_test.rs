use super::*;

#[test]
fn test_telemetry_event_builder() {
    let event = TelemetryEvent::new("agent.spawned", "success")
        .with_tenant_id("tenant-1".into())
        .with_agent_id("agent-1".into())
        .with_mission_id("mission-1".into())
        .with_cost(0.05);
    assert_eq!(event.event_type, "agent.spawned");
    assert_eq!(event.tenant_id.as_deref(), Some("tenant-1"));
    assert_eq!(event.agent_id.as_deref(), Some("agent-1"));
    assert!((event.cost_usd - 0.05).abs() < f64::EPSILON);
}

#[test]
fn test_emit_disabled() {
    let svc = TelemetryService::new(false);
    let event = TelemetryEvent::new("test", "ok");
    assert!(svc.emit(&event).is_ok());
}

#[test]
fn test_emit_enabled() {
    let svc = TelemetryService::new(true);
    let event = TelemetryEvent::new("test", "ok");
    assert!(svc.emit(&event).is_ok());
}

#[test]
fn test_record_convenience() {
    let svc = TelemetryService::new(true);
    let result = svc.record("test.event", "ok", |e| {
        e.with_tenant_id("t1".into())
    });
    assert!(result.is_ok());
}

#[test]
fn test_additional_fields() {
    let event = TelemetryEvent::new("test", "ok")
        .with_additional("reason".into(), "policy_denied".into());
    assert_eq!(event.additional.get("reason").unwrap(), "policy_denied");
}

#[test]
fn test_default_enabled() {
    let svc = TelemetryService::default();
    assert!(svc.enabled);
}
