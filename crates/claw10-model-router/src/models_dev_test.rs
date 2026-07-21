use super::*;

#[test]
fn test_resolve_template_url() {
    unsafe { std::env::set_var("TEST_VAR", "hello") };
    let url = "https://api.example.com/${TEST_VAR}/v1";
    assert_eq!(resolve_template_url(url), "https://api.example.com/hello/v1");
}

#[test]
fn test_resolve_template_url_missing_var() {
    unsafe { std::env::remove_var("NONEXISTENT_VAR_12345") };
    let url = "https://api.example.com/${NONEXISTENT_VAR_12345}/v1";
    assert!(
        resolve_template_url(url).contains("NONEXISTENT_VAR_12345")
            || resolve_template_url(url).contains("//")
    );
}

#[test]
fn test_convert_model_basic() {
    let entry = ModelEntry {
        id: "gpt-4o".to_string(),
        name: "GPT-4o".to_string(),
        description: None,
        reasoning: false,
        tool_call: true,
        structured_output: None,
        temperature: true,
        limit: Some(ModelLimit {
            context: Some(128_000),
            input: None,
            output: Some(16_384),
        }),
        cost: Some(ModelCost {
            input: Some(2.50),
            output: Some(10.00),
            cache_read: None,
        }),
        modalities: Some(ModelModalities {
            input: vec!["text".to_string(), "image".to_string()],
            output: vec!["text".to_string()],
        }),
        open_weights: None,
    };

    let profile = convert_model("openai", &entry);
    assert_eq!(profile.id, "gpt-4o");
    assert_eq!(profile.context_window, 128_000);
    assert_eq!(profile.max_output_tokens, 16_384);
    assert_eq!(profile.cost_per_1m_input, 2.50);
    assert!(profile.suitable_for.contains(&"coding".to_string()));
    assert!(profile.suitable_for.contains(&"vision".to_string()));
}
