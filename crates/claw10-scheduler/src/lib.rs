use std::sync::Arc;

use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use uuid::Uuid;

use claw10_domain::{AgentId, Schedule};
use claw10_store::{Store, StoreError, StoreExt};

// ── Lightweight Cron Parser ───────────────────────────────────────
// Replaces the heavy `cron` crate with a minimal RFC 5988 parser.
// Supports: seconds, minutes, hours, day-of-month, month, day-of-week
// Fields: * , - / (wildcard, list, range, step)

#[derive(Debug, Clone)]
struct CronField {
    values: Vec<u32>,
}

impl CronField {
    fn matches(&self, value: u32) -> bool {
        self.values.contains(&value)
    }
}

#[derive(Debug, Clone)]
struct CronExpression {
    second: CronField,
    minute: CronField,
    hour: CronField,
    day_of_month: CronField,
    month: CronField,
    day_of_week: CronField,
}

/// Parse a single cron field (e.g. "*/5", "1,3,5", "10-20", "0")
fn parse_field(expr: &str, min: u32, max: u32) -> Result<CronField, String> {
    let mut values = Vec::new();

    for part in expr.split(',') {
        if let Some((start_s, rest)) = part.split_once('/') {
            let start = if start_s == "*" {
                min
            } else {
                parse_field_value(start_s, min, max)?
            };
            let step: u32 = rest.parse().map_err(|_| format!("invalid step: {rest}"))?;
            if step == 0 {
                return Err("step cannot be 0".into());
            }
            let mut val = start;
            while val <= max {
                values.push(val);
                val += step;
            }
        } else if let Some((start_s, end_s)) = part.split_once('-') {
            let start = parse_field_value(start_s, min, max)?;
            let end = parse_field_value(end_s, min, max)?;
            if start > end {
                return Err(format!("range {start}-{end} is invalid"));
            }
            for v in start..=end {
                values.push(v);
            }
        } else if part == "*" {
            for v in min..=max {
                values.push(v);
            }
        } else {
            values.push(parse_field_value(part, min, max)?);
        }
    }

    values.sort();
    values.dedup();
    Ok(CronField { values })
}

fn parse_field_value(s: &str, min: u32, max: u32) -> Result<u32, String> {
    let val: u32 = s.parse().map_err(|_| format!("invalid value: {s}"))?;
    if val < min || val > max {
        return Err(format!("value {val} out of range {min}-{max}"));
    }
    Ok(val)
}

impl CronExpression {
    fn parse(expr: &str) -> Result<Self, String> {
        let fields: Vec<&str> = expr.split_whitespace().collect();

        // Support both 5-field (no seconds) and 6-field (with seconds) cron
        let (second_expr, minute_expr, hour_expr, dom_expr, month_expr, dow_expr) =
            match fields.len() {
                5 => (
                    "0", // default second = 0
                    fields[0],
                    fields[1],
                    fields[2],
                    fields[3],
                    fields[4],
                ),
                6 => (
                    fields[0], fields[1], fields[2], fields[3], fields[4], fields[5],
                ),
                _ => {
                    return Err(format!(
                        "expected 5 or 6 fields, got {}",
                        fields.len()
                    ))
                }
            };

        Ok(Self {
            second: parse_field(second_expr, 0, 59)?,
            minute: parse_field(minute_expr, 0, 59)?,
            hour: parse_field(hour_expr, 0, 23)?,
            day_of_month: parse_field(dom_expr, 1, 31)?,
            month: parse_field(month_expr, 1, 12)?,
            day_of_week: parse_field(dow_expr, 0, 7)?, // 0=Sun, 7=Sun
        })
    }

    /// Check if a NaiveDateTime matches this cron expression.
    #[cfg(test)]
    fn matches_datetime(&self, dt: &chrono::NaiveDateTime) -> bool {
        self.second.matches(dt.second())
            && self.minute.matches(dt.minute())
            && self.hour.matches(dt.hour())
            && self.day_of_month.matches(dt.day())
            && self.month.matches(dt.month())
            && self.day_of_week.matches(dt.weekday().num_days_from_sunday())
    }
}

/// Find the next fire time for a cron expression, starting from the given time.
/// Uses a greedy approach: match from coarsest to finest field.
/// Returns None if no match found within ~366 days.
fn next_fire_time(cron: &CronExpression, from: chrono::NaiveDateTime) -> Option<chrono::NaiveDateTime> {
    let mut dt = from + Duration::seconds(1);
    dt = dt.with_second(0)?;

    // Ensure we start at second 0
    let start_second = *cron.second.values.first()?;

    // Try up to ~527040 minutes (366 days)
    for _ in 0..527_040u32 {
        // Match year (no choice, just check if month is in range)
        let year = dt.year();
        let first_month = *cron.month.values.first()?;
        let last_month = *cron.month.values.last()?;

        if dt.month() > last_month {
            // Past all valid months this year, jump to next year
            dt = chrono::NaiveDate::from_ymd_opt(year + 1, first_month, 1)?
                .and_hms_opt(0, 0, 0)?;
            continue;
        }

        if !cron.month.matches(dt.month()) {
            // Not a valid month, jump to next valid month
            let next_month = cron
                .month
                .values
                .iter()
                .find(|&&m| m >= dt.month())
                .copied()
                .unwrap_or(first_month);

            if next_month < dt.month() {
                // Wrapped around, next year
                dt = chrono::NaiveDate::from_ymd_opt(year + 1, first_month, 1)?
                    .and_hms_opt(0, 0, 0)?;
                continue;
            }

            dt = chrono::NaiveDate::from_ymd_opt(year, next_month, 1)?
                .and_hms_opt(0, 0, 0)?;
            continue;
        }

        // Match hour
        if !cron.hour.matches(dt.hour()) {
            let next_hour = cron
                .hour
                .values
                .iter()
                .find(|&&h| h >= dt.hour())
                .copied()
                .unwrap_or(cron.hour.values[0]);

            if next_hour <= dt.hour() {
                // Next day
                dt = dt + Duration::days(1);
                dt = dt.with_hour(0)?.with_minute(0)?.with_second(0)?;
                continue;
            }

            dt = dt.with_hour(next_hour)?.with_minute(0)?.with_second(0)?;
            continue;
        }

        // Match minute
        if !cron.minute.matches(dt.minute()) {
            let next_minute = cron
                .minute
                .values
                .iter()
                .find(|&&m| m >= dt.minute())
                .copied()
                .unwrap_or(cron.minute.values[0]);

            if next_minute <= dt.minute() {
                // Next hour
                dt = dt + Duration::hours(1);
                dt = dt.with_minute(0)?.with_second(0)?;
                continue;
            }

            dt = dt.with_minute(next_minute)?.with_second(0)?;
            continue;
        }

        // Match second
        if !cron.second.matches(dt.second()) {
            if start_second > dt.second() {
                dt = dt.with_second(start_second)?;
            } else {
                // Next minute
                dt = dt + Duration::minutes(1);
                dt = dt.with_second(start_second)?;
            }
            continue;
        }

        // Match day of month
        if !cron.day_of_month.matches(dt.day()) {
            let next_dom = cron
                .day_of_month
                .values
                .iter()
                .find(|&&d| d >= dt.day())
                .copied()
                .unwrap_or(cron.day_of_month.values[0]);

            if next_dom <= dt.day() {
                // Next month
                dt = dt + Duration::days(1);
                dt = dt.with_hour(0)?.with_minute(0)?.with_second(start_second)?;
                continue;
            }

            dt = dt.with_day(next_dom)?
                .with_hour(0)?
                .with_minute(0)?
                .with_second(start_second)?;
            continue;
        }

        // Match day of week
        let dow = dt.weekday().num_days_from_sunday();
        if !cron.day_of_week.matches(dow) {
            // Next day
            dt = dt + Duration::days(1);
            dt = dt.with_hour(0)?.with_minute(0)?.with_second(start_second)?;
            continue;
        }

        // All fields match!
        return Some(dt);
    }

    None // No match within 366 days
}

// ── Scheduler Service ─────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("agent not found: {0}")]
    AgentNotFound(String),
    #[error("schedule not found for agent {agent_id}: {schedule_id}")]
    ScheduleNotFound {
        agent_id: String,
        schedule_id: String,
    },
    #[error("timezone parsing failed: {0}")]
    InvalidTimezone(String),
    #[error("{0}")]
    Other(String),
}

impl From<StoreError> for SchedulerError {
    fn from(e: StoreError) -> Self {
        Self::Other(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct DueSchedule {
    pub agent_id: AgentId,
    pub schedule: Schedule,
}

const KEY_PREFIX: &str = "schedule:";

pub struct ScheduleService {
    store: Arc<dyn Store>,
}

impl ScheduleService {
    #[must_use]
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    fn agent_key(agent_id: &AgentId) -> String {
        format!("{KEY_PREFIX}{}", agent_id.0)
    }

    /// Add a schedule for an agent.
    ///
    /// # Errors
    /// Returns `SchedulerError::InvalidCron` if the cron expression is invalid.
    /// Returns `SchedulerError::InvalidTimezone` if the timezone is invalid.
    pub async fn add_schedule(
        &self,
        agent_id: &AgentId,
        schedule: Schedule,
    ) -> Result<(), SchedulerError> {
        // Validate cron expression using our lightweight parser
        CronExpression::parse(&schedule.cron)
            .map_err(|e| SchedulerError::InvalidCron(e))?;

        // Validate timezone
        schedule
            .timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|e| SchedulerError::InvalidTimezone(e.to_string()))?;

        let key = Self::agent_key(agent_id);
        let mut schedules: Vec<Schedule> = self.store.get(&key).await?.unwrap_or_default();
        schedules.push(schedule);
        self.store.set(&key, &schedules).await?;

        Ok(())
    }

    /// Remove a schedule from an agent by index.
    ///
    /// # Errors
    /// Returns `SchedulerError::AgentNotFound` if the agent has no schedules.
    /// Returns `SchedulerError::ScheduleNotFound` if the index is out of bounds.
    pub async fn remove_schedule(
        &self,
        agent_id: &AgentId,
        schedule_index: usize,
    ) -> Result<(), SchedulerError> {
        let key = Self::agent_key(agent_id);
        let mut schedules: Vec<Schedule> = self
            .store
            .get(&key)
            .await?
            .ok_or(SchedulerError::AgentNotFound(agent_id.0.to_string()))?;

        if schedule_index >= schedules.len() {
            return Err(SchedulerError::ScheduleNotFound {
                agent_id: agent_id.0.to_string(),
                schedule_id: schedule_index.to_string(),
            });
        }

        schedules.remove(schedule_index);
        if schedules.is_empty() {
            self.store.delete(&key).await?;
        } else {
            self.store.set(&key, &schedules).await?;
        }

        Ok(())
    }

    /// List all schedules for an agent.
    ///
    /// # Errors
    /// Returns `SchedulerError` if the store fails to read schedules.
    pub async fn list_schedules(&self, agent_id: &AgentId) -> Result<Vec<Schedule>, SchedulerError> {
        let key = Self::agent_key(agent_id);
        Ok(self.store.get::<Vec<Schedule>>(&key).await?.unwrap_or_default())
    }

    /// Get all schedules that are due at the given time.
    ///
    /// # Errors
    /// Returns `SchedulerError` if the store fails to read schedules.
    pub async fn get_due_schedules(
        &self,
        now: &DateTime<Utc>,
    ) -> Result<Vec<DueSchedule>, SchedulerError> {
        let all: Vec<(String, Vec<Schedule>)> = self.store.scan_prefix(KEY_PREFIX).await?;
        let mut due = Vec::new();

        for (agent_key, agent_schedules) in &all {
            let agent_id_str = agent_key
                .strip_prefix(KEY_PREFIX)
                .unwrap_or(agent_key);
            let Ok(agent_uuid) = Uuid::parse_str(agent_id_str) else {
                continue;
            };
            let agent_id = AgentId(agent_uuid);

            for schedule in agent_schedules {
                if Self::is_schedule_due(schedule, now) {
                    due.push(DueSchedule {
                        agent_id: agent_id.clone(),
                        schedule: schedule.clone(),
                    });
                }
            }
        }

        Ok(due)
    }

    /// Process all due schedules and return them for execution.
    ///
    /// # Errors
    /// Returns `SchedulerError` if the store fails to read schedules.
    pub async fn tick(&self, now: &DateTime<Utc>) -> Result<Vec<DueSchedule>, SchedulerError> {
        let due = self.get_due_schedules(now).await?;
        Ok(due)
    }

    /// Record that a schedule was last run at the given time.
    ///
    /// # Errors
    /// Returns `SchedulerError` if the store fails to write the last run time.
    pub async fn record_last_run(
        &self,
        agent_id: &AgentId,
        schedule_index: usize,
        ran_at: DateTime<Utc>,
    ) -> Result<(), SchedulerError> {
        let key = format!("{}:{}:last_run", Self::agent_key(agent_id), schedule_index);
        self.store.set(&key, &ran_at.to_rfc3339()).await?;
        Ok(())
    }

    /// Get the last run time for a schedule.
    ///
    /// # Errors
    /// Returns `SchedulerError` if the store fails to read the last run time.
    pub async fn get_last_run(
        &self,
        agent_id: &AgentId,
        schedule_index: usize,
    ) -> Result<Option<DateTime<Utc>>, SchedulerError> {
        let key = format!("{}:{}:last_run", Self::agent_key(agent_id), schedule_index);
        if let Some(rfc3339) = self.store.get::<String>(&key).await? {
            Ok(DateTime::parse_from_rfc3339(&rfc3339)
                .ok()
                .map(|dt| dt.with_timezone(&Utc)))
        } else {
            Ok(None)
        }
    }

    /// Check if a schedule is due at the given time.
    /// Uses our lightweight cron parser (replaces the `cron` crate).
    #[must_use]
    pub fn is_schedule_due(schedule: &Schedule, now: &DateTime<Utc>) -> bool {
        let Ok(cron) = CronExpression::parse(&schedule.cron) else {
            return false;
        };

        let Ok(tz) = schedule.timezone.parse::<chrono_tz::Tz>() else {
            return false;
        };

        let local_now = now.with_timezone(&tz);
        let naive_now = local_now.naive_local();

        let Some(next) = next_fire_time(&cron, naive_now) else {
            return false;
        };

        // The next fire time should be within 60 seconds of now
        let diff = (next - naive_now).num_seconds().abs();
        diff <= 60
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
