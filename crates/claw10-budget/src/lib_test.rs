use super::*;

// Helper untuk membuat objek Budget bawaan untuk pengujian
fn make_test_budget(allocated: f64, spent: f64, hard_limit: Option<f64>) -> Budget {
    Budget {
        allocated_usd: allocated,
        spent_usd: spent,
        soft_limit_usd: None,
        hard_limit_usd: hard_limit,
        recurring_monthly_usd: None,
    }
}

#[test]
fn test_reserve_success() {
    let mut budget = make_test_budget(10.0, 2.0, None);
    let svc = BudgetService;
    
    // Alokasi sejumlah 5.0 harus berhasil karena sisa anggaran adalah 8.0
    let res = svc.reserve(&mut budget, 5.0);
    assert!(res.is_ok());
    assert_eq!(budget.spent_usd, 7.0);
}

#[test]
fn test_reserve_exhausted() {
    let mut budget = make_test_budget(10.0, 8.0, None);
    let svc = BudgetService;
    
    // Alokasi sejumlah 3.0 harus gagal karena sisa anggaran hanya 2.0
    let res = svc.reserve(&mut budget, 3.0);
    assert!(res.is_err());
    if let Err(BudgetError::Exhausted { remaining, required }) = res {
        assert_eq!(remaining, 2.0);
        assert_eq!(required, 3.0);
    } else {
        panic!("Diharapkan error Exhausted");
    }
}

#[test]
fn test_reserve_hard_limit() {
    let mut budget = make_test_budget(10.0, 5.0, Some(8.0));
    let svc = BudgetService;
    
    // Alokasi sejumlah 4.0 harus gagal karena melampaui hard limit 8.0 (5.0 + 4.0 = 9.0)
    // Meskipun sisa alokasi total masih mencukupi (10.0 - 5.0 = 5.0)
    let res = svc.reserve(&mut budget, 4.0);
    assert!(res.is_err());
    assert!(matches!(res, Err(BudgetError::HardLimitReached)));
}

#[test]
fn test_can_allocate() {
    let budget_no_limit = make_test_budget(10.0, 5.0, None);
    let budget_with_limit = make_test_budget(10.0, 5.0, Some(8.0));
    let overspent_budget = make_test_budget(10.0, 12.0, None);
    
    // Uji dengan alokasi yang valid
    assert!(BudgetService::can_allocate(&budget_no_limit, 4.0));
    assert!(BudgetService::can_allocate(&budget_with_limit, 2.0));
    
    // Uji alokasi pas (exact) sisa anggaran
    assert!(BudgetService::can_allocate(&budget_no_limit, 5.0));

    // Uji alokasi pas (exact) hard limit
    assert!(BudgetService::can_allocate(&budget_with_limit, 3.0));

    // Uji alokasi yang melampaui total anggaran
    assert!(!BudgetService::can_allocate(&budget_no_limit, 6.0));
    
    // Uji alokasi yang melampaui hard limit
    assert!(!BudgetService::can_allocate(&budget_with_limit, 4.0));

    // Uji pada kondisi overspent (sudah melebihi alokasi)
    assert!(!BudgetService::can_allocate(&overspent_budget, 1.0));
}

#[test]
fn test_create_cost_record() {
    let record = BudgetService::create_cost_record(
        "mission_123".to_string(),
        "agent_abc".to_string(),
        0.005,
        CostCategory::ModelCall,
    );
    
    assert_eq!(record.mission_id, "mission_123");
    assert_eq!(record.agent_id, "agent_abc");
    assert_eq!(record.amount_usd, 0.005);
    assert!(matches!(record.category, CostCategory::ModelCall));
    assert!(record.task_id.is_none());
}
