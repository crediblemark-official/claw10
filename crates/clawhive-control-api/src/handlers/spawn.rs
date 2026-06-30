use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use clawhive_domain::{Agent, AgentId, ChildSpec, Lineage, Mission, MissionId, SpawnRequest, SpawnState};
use clawhive_lineage::LineageService;
use clawhive_spawn::broker::SpawnBroker;
use clawhive_store::StoreExt;

use clawhive_event::ClawHiveEvent;

use crate::error::ApiError;
use crate::handlers::audit::{self, build_audit_event};
use crate::state::AppState;
use crate::store::{AGENT_PREFIX, LINEAGE_PREFIX, MISSION_PREFIX, SPAWNREQ_PREFIX};

#[derive(Serialize)]
pub struct SpawnResponse {
    pub id: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct CreateSpawnRequest {
    pub mission_id: String,
    pub requested_by: String,
    pub reason: String,
    pub children: Vec<CreateChildSpec>,
}

#[derive(Deserialize)]
pub struct CreateChildSpec {
    pub role: String,
    pub objective: String,
    pub budget_usd: f64,
    pub model_profile: String,
    #[serde(default = "default_max_turns")]
    pub max_turns: u32,
}

fn default_max_turns() -> u32 {
    100
}

#[derive(Serialize)]
pub struct ChildResult {
    pub id: String,
    pub name: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct ApproveSpawnResponse {
    pub request_id: String,
    pub state: String,
    pub children: Vec<ChildResult>,
}

pub async fn list_spawn_requests(
    State(state): State<AppState>,
) -> Result<Json<Vec<SpawnResponse>>, ApiError> {
    let requests: Vec<SpawnResponse> = state
        .kv_store
        .scan_prefix::<SpawnRequest>(SPAWNREQ_PREFIX)
        .await?
        .into_iter()
        .map(|(_, r)| SpawnResponse {
            id: r.id.0.to_string(),
            state: format!("{:?}", r.state),
        })
        .collect();

    Ok(Json(requests))
}

pub async fn get_spawn_request(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SpawnResponse>, ApiError> {
    let key = format!("{SPAWNREQ_PREFIX}{id}");
    let request = state
        .kv_store
        .get::<SpawnRequest>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("spawn request {id}")))?;

    Ok(Json(SpawnResponse {
        id: request.id.0.to_string(),
        state: format!("{:?}", request.state),
    }))
}

pub async fn create_spawn_request(
    State(state): State<AppState>,
    Json(req): Json<CreateSpawnRequest>,
) -> Result<(StatusCode, Json<SpawnResponse>), ApiError> {
    let children: Vec<ChildSpec> = req
        .children
        .into_iter()
        .map(|c| ChildSpec {
            role: c.role,
            objective: c.objective,
            budget_usd: c.budget_usd,
            model_profile: c.model_profile,
            max_turns: c.max_turns,
            custom_permissions: None,
        })
        .collect();

    let spawn_request = SpawnBroker::create_request(
        MissionId(
            Uuid::parse_str(&req.mission_id)
                .map_err(|e| ApiError::Validation(format!("invalid mission_id: {e}")))?,
        ),
        AgentId(
            Uuid::parse_str(&req.requested_by)
                .map_err(|e| ApiError::Validation(format!("invalid agent_id: {e}")))?,
        ),
        req.reason,
        children,
    );

    let response = SpawnResponse {
        id: spawn_request.id.0.to_string(),
        state: format!("{:?}", spawn_request.state),
    };

    let key = format!("{SPAWNREQ_PREFIX}{}", spawn_request.id.0);
    state.kv_store.set(&key, &spawn_request).await?;

    let _ = state.telemetry.record("spawn.created", "pending", |e| {
        e.with_mission_id(spawn_request.mission_id.0.to_string())
            .with_additional("reason".into(), spawn_request.reason.clone())
    });

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn approve_spawn(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApproveSpawnResponse>, ApiError> {
    // Load all agents for depth/validation
    let all_agents: Vec<Agent> = state
        .kv_store
        .scan_prefix_unsorted::<Agent>(AGENT_PREFIX)
        .await?
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    // Load spawn request
    let spawn_key = format!("{SPAWNREQ_PREFIX}{id}");
    let mut request: SpawnRequest = state
        .kv_store
        .get(&spawn_key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("spawn request {id}")))?;

    if !matches!(request.state, SpawnState::Pending) {
        return Err(ApiError::Validation(format!(
            "spawn request {id} is not pending"
        )));
    }

    // Load mission
    let mission_key = format!("{MISSION_PREFIX}{}", request.mission_id.0);
    let mission: Mission = state
        .kv_store
        .get(&mission_key)
        .await?
        .ok_or_else(|| ApiError::NotFound("mission not found".into()))?;

    // Calculate depth
    let requested_by = request.requested_by.clone();
    let current_depth = calculate_depth(&requested_by, &all_agents);

    // Load parent agent
    let parent_key = format!("{AGENT_PREFIX}{}", requested_by.0);
    let mut parent: Agent = state
        .kv_store
        .get(&parent_key)
        .await?
        .ok_or_else(|| ApiError::NotFound("parent agent not found".into()))?;

    // Process spawn
    let children = state
        .spawn_broker
        .process_spawn_request(
            &mut parent,
            &mission,
            &request,
            &all_agents,
            current_depth,
        )
        .await
        .map_err(|e| ApiError::SpawnFailed(e.to_string()))?;

    // Update and save spawn request
    request.state = SpawnState::Approved;
    request.updated_at = chrono::Utc::now();
    state.kv_store.set(&spawn_key, &request).await?;

    let _ = state.telemetry.record("spawn.approved", "success", |e| {
        e.with_agent_id(parent.id.0.to_string())
            .with_mission_id(parent.mission_id.0.to_string())
            .with_additional("num_children".into(), children.len().to_string())
    });

    // Save parent (budget was modified)
    state.kv_store.set(&parent_key, &parent).await?;

    // Build child results and save each child
    let child_results: Vec<ChildResult> = children
        .iter()
        .map(|c| ChildResult {
            id: c.id.0.to_string(),
            name: c.name.clone(),
            role: c.role.clone(),
        })
        .collect();

    for child in &children {
        let child_key = format!("{AGENT_PREFIX}{}", child.id.0);
        state.kv_store.set(&child_key, child).await?;
    }

    // Update lineage
    let lineage_key = format!("{LINEAGE_PREFIX}{}", parent.lineage_id.0);
    let mut lineage = match state.kv_store.get::<Lineage>(&lineage_key).await? {
        Some(l) => l,
        None => {
            let mut l = LineageService::create_lineage(
                parent.mission_id.clone(),
                parent.id.clone(),
            );
            l.id = parent.lineage_id.clone();
            l
        }
    };

    for child in &children {
        LineageService::add_entry(
            &mut lineage,
            child.id.clone(),
            Some(requested_by.clone()),
            child.role.clone(),
        );
    }
    state.kv_store.set(&lineage_key, &lineage).await?;

    let _ = state.event_bus.publish(ClawHiveEvent::SpawnRequestApproved {
        spawn_request_id: request.id.0,
        parent_agent_id: requested_by.0,
        child_count: children.len(),
        timestamp: chrono::Utc::now(),
    }).await;

    audit::emit_event(
        Arc::clone(&state.audit_service),
        build_audit_event(
            "spawn_approved",
            Some(requested_by.0.to_string()),
            Some(request.mission_id.0.to_string()),
            None,
            serde_json::json!({"spawn_request_id": request.id.0.to_string(), "child_count": children.len()}),
        ),
    );

    Ok(Json(ApproveSpawnResponse {
        request_id: request.id.0.to_string(),
        state: "approved".into(),
        children: child_results,
    }))
}

pub async fn deny_spawn(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SpawnResponse>, ApiError> {
    let key = format!("{SPAWNREQ_PREFIX}{id}");
    let mut request = state
        .kv_store
        .get::<SpawnRequest>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("spawn request {id}")))?;

    if !matches!(request.state, SpawnState::Pending) {
        return Err(ApiError::Validation(format!(
            "spawn request {id} is not pending"
        )));
    }

    request.state = SpawnState::Denied;
    request.updated_at = chrono::Utc::now();
    state.kv_store.set(&key, &request).await?;

    let _ = state.telemetry.record("spawn.denied", "success", |e| {
        e.with_mission_id(request.mission_id.0.to_string())
    });

    let _ = state.event_bus.publish(ClawHiveEvent::SpawnRequestDenied {
        spawn_request_id: request.id.0,
        parent_agent_id: request.requested_by.0,
        reason: "denied by user".into(),
        timestamp: chrono::Utc::now(),
    }).await;

    audit::emit_event(
        Arc::clone(&state.audit_service),
        build_audit_event(
            "spawn_denied",
            Some(request.requested_by.0.to_string()),
            Some(request.mission_id.0.to_string()),
            None,
            serde_json::json!({"spawn_request_id": request.id.0.to_string(), "reason": "denied by user"}),
        ),
    );

    Ok(Json(SpawnResponse {
        id: request.id.0.to_string(),
        state: "denied".into(),
    }))
}

fn calculate_depth(agent_id: &AgentId, agents: &[Agent]) -> u32 {
    let mut depth = 0;
    let mut current = agent_id.clone();
    while let Some(agent) = agents.iter().find(|a: &&Agent| a.id == current) {
        match &agent.parent_agent_id {
            Some(pid) => {
                depth += 1;
                current = pid.clone();
            }
            None => break,
        }
    }
    depth
}
