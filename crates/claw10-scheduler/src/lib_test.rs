use super::*;
use chrono::TimeZone;
use claw10_store::InMemoryStore;
use std::sync::Arc;

fn setup() -> ScheduleService {
    ScheduleService::new(Arc::new(InMemoryStore::new()))
}

#[tokio::test]
async fn test_tick_returns_due_schedules() {
    let svc = setup();
    let agent_id = AgentId(Uuid::now_v7());
    let schedule = Schedule {
        cron: "0 * * * * *".to_string(),
        timezone: "UTC".to_string(),
        action: claw10_domain::ScheduleAction::Wake,
    };
    svc.add_schedule(&agent_id, schedule).await.unwrap();

    let now = Utc::now();
    let due = svc.tick(&now).await.unwrap();
    assert!(!due.is_empty());
    assert_eq!(due[0].agent_id, agent_id);
}

#[tokio::test]
async fn test_record_and_get_last_run() {
    let svc = setup();
    let agent_id = AgentId(Uuid::now_v7());

    let before = Utc.timestamp_opt(1_000_000, 0).unwrap();
    svc.record_last_run(&agent_id, 0, before).await.unwrap();

    let last_run = svc.get_last_run(&agent_id, 0).await.unwrap();
    assert_eq!(last_run, Some(before));
}

#[tokio::test]
async fn test_get_last_run_returns_none_when_empty() {
    let svc = setup();
    let agent_id = AgentId(Uuid::now_v7());
    let last_run = svc.get_last_run(&agent_id, 0).await.unwrap();
    assert_eq!(last_run, None);
}

// ── Cron parser tests ─────────────────────────────────────

#[test]
fn test_parse_wildcard() {
    let cron = CronExpression::parse("* * * * *").unwrap();
    let dt = chrono::NaiveDateTime::parse_from_str("2024-01-15 12:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
    assert!(cron.matches_datetime(&dt));
}

#[test]
fn test_parse_specific_values() {
    let cron = CronExpression::parse("0 12 * * *").unwrap();
    let matching = chrono::NaiveDateTime::parse_from_str("2024-01-15 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let non_matching = chrono::NaiveDateTime::parse_from_str("2024-01-15 13:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    assert!(cron.matches_datetime(&matching));
    assert!(!cron.matches_datetime(&non_matching));
}

#[test]
fn test_parse_range() {
    let cron = CronExpression::parse("0 9-17 * * *").unwrap();
    let morning = chrono::NaiveDateTime::parse_from_str("2024-01-15 09:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let evening = chrono::NaiveDateTime::parse_from_str("2024-01-15 18:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    assert!(cron.matches_datetime(&morning));
    assert!(!cron.matches_datetime(&evening));
}

#[test]
fn test_parse_step() {
    let cron = CronExpression::parse("*/5 * * * *").unwrap();
    let t0 = chrono::NaiveDateTime::parse_from_str("2024-01-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let t5 = chrono::NaiveDateTime::parse_from_str("2024-01-15 10:05:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let t3 = chrono::NaiveDateTime::parse_from_str("2024-01-15 10:03:00", "%Y-%m-%d %H:%M:%S").unwrap();
    assert!(cron.matches_datetime(&t0));
    assert!(cron.matches_datetime(&t5));
    assert!(!cron.matches_datetime(&t3));
}

#[test]
fn test_parse_list() {
    let cron = CronExpression::parse("0 0,12 * * *").unwrap();
    let midnight = chrono::NaiveDateTime::parse_from_str("2024-01-15 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let noon = chrono::NaiveDateTime::parse_from_str("2024-01-15 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let other = chrono::NaiveDateTime::parse_from_str("2024-01-15 06:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    assert!(cron.matches_datetime(&midnight));
    assert!(cron.matches_datetime(&noon));
    assert!(!cron.matches_datetime(&other));
}

#[test]
fn test_parse_6_field_with_seconds() {
    let cron = CronExpression::parse("0 */5 * * * *").unwrap();
    let t0 = chrono::NaiveDateTime::parse_from_str("2024-01-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let t5 = chrono::NaiveDateTime::parse_from_str("2024-01-15 10:05:00", "%Y-%m-%d %H:%M:%S").unwrap();
    assert!(cron.matches_datetime(&t0));
    assert!(cron.matches_datetime(&t5));
}

#[test]
fn test_next_fire_time() {
    let cron = CronExpression::parse("*/5 * * * *").unwrap();
    let from = chrono::NaiveDateTime::parse_from_str("2024-01-15 10:03:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let next = next_fire_time(&cron, from).unwrap();
    assert_eq!(next.format("%H:%M").to_string(), "10:05");
}

#[test]
fn test_next_fire_time_crosses_hour() {
    let cron = CronExpression::parse("*/5 * * * *").unwrap();
    let from = chrono::NaiveDateTime::parse_from_str("2024-01-15 10:58:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let next = next_fire_time(&cron, from).unwrap();
    assert_eq!(next.format("%H:%M").to_string(), "11:00");
}

#[test]
fn test_invalid_cron() {
    assert!(CronExpression::parse("invalid").is_err());
    assert!(CronExpression::parse("60 * * * *").is_err()); // second 60 invalid
    assert!(CronExpression::parse("0 25 * * *").is_err()); // hour 25 invalid
}
