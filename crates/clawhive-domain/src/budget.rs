use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub allocated_usd: f64,
    pub spent_usd: f64,
    pub soft_limit_usd: Option<f64>,
    pub hard_limit_usd: Option<f64>,
    pub recurring_monthly_usd: Option<f64>,
}

impl Budget {
    pub fn remaining(&self) -> f64 {
        self.allocated_usd - self.spent_usd
    }

    pub fn is_exhausted(&self) -> bool {
        self.spent_usd >= self.hard_limit_usd.unwrap_or(self.allocated_usd)
    }

    pub fn is_soft_limit_reached(&self) -> bool {
        self.soft_limit_usd
            .map_or(false, |limit| self.spent_usd >= limit)
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            allocated_usd: 0.0,
            spent_usd: 0.0,
            soft_limit_usd: None,
            hard_limit_usd: None,
            recurring_monthly_usd: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRecord {
    pub mission_id: String,
    pub task_id: Option<String>,
    pub agent_id: String,
    pub amount_usd: f64,
    pub category: CostCategory,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CostCategory {
    ModelCall,
    ToolExecution,
    SpawnTax,
    Storage,
    Compute,
    Network,
    Other,
}
