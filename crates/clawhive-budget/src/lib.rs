#![allow(clippy::pedantic)]

use clawhive_domain::{Budget, CostCategory, CostRecord};

#[derive(Debug, thiserror::Error)]
pub enum BudgetError {
    #[error("budget exhausted: remaining {remaining}, required {required}")]
    Exhausted { remaining: f64, required: f64 },
    #[error("hard limit reached")]
    HardLimitReached,
}

pub struct BudgetService;

impl BudgetService {
    pub fn reserve(&self, budget: &mut Budget, amount: f64) -> Result<(), BudgetError> {
        let remaining = budget.allocated_usd - budget.spent_usd;
        if remaining < amount {
            return Err(BudgetError::Exhausted {
                remaining,
                required: amount,
            });
        }

        if let Some(hard_limit) = budget.hard_limit_usd
            && budget.spent_usd + amount > hard_limit
        {
            return Err(BudgetError::HardLimitReached);
        }

        budget.spent_usd += amount;
        Ok(())
    }

    #[must_use]
    pub fn can_allocate(budget: &Budget, amount: f64) -> bool {
        let remaining = budget.allocated_usd - budget.spent_usd;
        if remaining < amount {
            return false;
        }
        if let Some(hard_limit) = budget.hard_limit_usd
            && budget.spent_usd + amount > hard_limit
        {
            return false;
        }
        true
    }

    #[must_use]
    pub fn create_cost_record(
        mission_id: String,
        agent_id: String,
        amount_usd: f64,
        category: CostCategory,
    ) -> CostRecord {
        CostRecord {
            mission_id,
            task_id: None,
            agent_id,
            amount_usd,
            category,
            timestamp: chrono::Utc::now(),
        }
    }
}
