use std::sync::Arc;

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use clawhive_domain::{Agent, PolicyBundleId, PolicyEffect, PolicyRule, PolicySubject};
use clawhive_icvs::IcvsCompiler;
use clawhive_policy::PolicyService;
use clawhive_store::StoreExt;

use crate::error::ApiError;
use crate::handlers::audit::{self, build_audit_event};
use crate::state::AppState;
use crate::store::AGENT_PREFIX;

#[derive(Serialize)]
pub struct PolicyResponse {
    pub result: String,
}

#[derive(Deserialize)]
pub struct CompilePolicyRequest {
    pub source: String,
}

#[derive(Deserialize)]
pub struct EvaluatePolicyRequest {
    pub bundle_id: String,
    pub subject: PolicySubject,
    pub action: String,
    pub resource: String,
    pub context: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct EvaluatePolicyResponse {
    pub allowed: bool,
    pub matched_rule: Option<String>,
    pub evaluation_time_ms: u64,
    pub reason: String,
}

/// POST /v1/policies/compile
///
/// Compiles a policy source into a `PolicyBundle` and persists it.
/// First tries the ICVS compiler; if the source is not valid ICVS it falls
/// back to the simple `allow`/`deny` line format for backward compatibility.
pub async fn compile_policy(
    State(state): State<AppState>,
    Json(req): Json<CompilePolicyRequest>,
) -> Json<PolicyResponse> {
    let rules: Vec<PolicyRule> =
        if let Ok(icvs_rules) = IcvsCompiler::compile_policy(&req.source) {
            icvs_rules
        } else {
            // Fallback naive parser for simple "allow role:admin read *" style lines.
            req.source
                .lines()
                .filter_map(|l| {
                    let t = l.trim();
                    if t.is_empty() || t.starts_with('#') {
                        return None;
                    }
                    let effect = if t.starts_with("allow") {
                        PolicyEffect::Allow
                    } else if t.starts_with("deny") {
                        PolicyEffect::ExplicitDeny
                    } else {
                        return None;
                    };
                    Some(PolicyRule {
                        id: clawhive_domain::PolicyRuleId(Uuid::now_v7()),
                        subject: PolicySubject::Role("*".into()),
                        action: "*".into(),
                        resource: "*".into(),
                        effect,
                        priority: 0,
                        condition: None,
                    })
                })
                .collect()
        };

    let rule_count = rules.len();
    let compiled = PolicyService::compile(rules);
    let bundle = PolicyService::create_bundle("compiled".into(), "1.0.0".into(), compiled);

    let bundle_id = bundle.id.0;
    let bundle_key = format!("policy:bundle:{bundle_id}");
    let _ = state.kv_store.set(&bundle_key, &bundle).await;

    Json(PolicyResponse {
        result: format!("compiled: {rule_count} rules (bundle_id: {bundle_id})"),
    })
}

/// POST /v1/policies/simulate
pub async fn simulate_policy(
    State(_state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Json<EvaluatePolicyResponse> {
    let subject = req
        .get("subject")
        .and_then(|v| serde_json::from_value::<PolicySubject>(v.clone()).ok())
        .unwrap_or(PolicySubject::Agent("simulated".into()));
    let action = req
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("read")
        .to_string();
    let resource = req
        .get("resource")
        .and_then(|v| v.as_str())
        .unwrap_or("resource")
        .to_string();
    let context = req.get("context");

    let rules: Vec<PolicyRule> = req
        .get("rules")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let result = PolicyService::simulate(&rules, &subject, &action, &resource, context);

    Json(EvaluatePolicyResponse {
        allowed: result.allowed,
        matched_rule: result.matched_rule.map(|r| r.id.0.to_string()),
        evaluation_time_ms: result.evaluation_time_ms,
        reason: result.reason,
    })
}

/// POST /v1/policies/evaluate
pub async fn evaluate_policy(
    State(state): State<AppState>,
    Json(req): Json<EvaluatePolicyRequest>,
) -> Result<Json<EvaluatePolicyResponse>, ApiError> {
    let bundle_id = PolicyBundleId(
        Uuid::parse_str(&req.bundle_id)
            .map_err(|e| ApiError::Validation(format!("invalid bundle_id: {e}")))?,
    );

    let bundle = state
        .kv_store
        .scan_prefix_unsorted::<Agent>(AGENT_PREFIX)
        .await?
        .into_iter()
        .find_map(|(_, a)| {
            if a.policy_bundle.id == bundle_id {
                Some(a.policy_bundle)
            } else {
                None
            }
        })
        .ok_or_else(|| ApiError::NotFound(format!("policy bundle {}", req.bundle_id)))?;

    let result = PolicyService::evaluate(
        &bundle,
        &req.subject,
        &req.action,
        &req.resource,
        req.context.as_ref(),
    )
    .map_err(|e| ApiError::Validation(e.to_string()))?;

    let _ = state.telemetry.record("policy.evaluated", if result.allowed { "allowed" } else { "denied" }, |e| {
        e.with_additional("action".into(), req.action.clone())
            .with_additional("resource".into(), req.resource.clone())
            .with_additional("subject".into(), format!("{:?}", req.subject))
    });

    audit::emit_event(
        Arc::clone(&state.audit_service),
        build_audit_event(
            "policy_evaluated",
            None,
            None,
            None,
            serde_json::json!({
                "bundle_id": req.bundle_id,
                "action": req.action,
                "resource": req.resource,
                "allowed": result.allowed,
            }),
        ),
    );

    Ok(Json(EvaluatePolicyResponse {
        allowed: result.allowed,
        matched_rule: result.matched_rule.map(|r| r.id.0.to_string()),
        evaluation_time_ms: result.evaluation_time_ms,
        reason: result.reason,
    }))
}
