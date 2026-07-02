/// Resolve API key env var name for a provider.
/// First checks the built-in catalog, falls back to `{NAME}_API_KEY` convention.
pub(crate) fn provider_api_key_env(provider: &str) -> String {
    // Check built-in catalog
    if let Some(slot) = claw10_model_router::providers::get_provider_slot(provider) {
        return slot.api_key_env.to_string();
    }
    // Fallback to conventional naming
    format!("{}_API_KEY", provider.to_uppercase())
}

pub fn get_palette_items() -> Vec<(String, String, String, String)> {
    let mut items: Vec<(String, String, String, String)> = vec![
        ("Suggested".into(), "Switch session".into(), "ctrl+x l".into(), "/session_switch".into()),
        ("Suggested".into(), "New session".into(), "ctrl+x n".into(), "/session_new".into()),
        ("Suggested".into(), "Switch model".into(), "ctrl+x m".into(), "/model_switch".into()),
        ("Suggested".into(), "Clear Cache, History & Context".into(), String::new(), "/clear_all".into()),
        ("Suggested".into(), "Share session".into(), String::new(), "/session_share".into()),
    ];
    items.extend([
        ("Session".into(), "Switch session".into(), "ctrl+x l".into(), "/session_switch".into()),
        ("Session".into(), "New session".into(), "ctrl+x n".into(), "/session_new".into()),
        ("Session".into(), "Share session".into(), String::new(), "/session_share".into()),
        ("Session".into(), "Rename session".into(), "ctrl+r".into(), "/session_rename".into()),
        ("System".into(), "Clear Cache, History & Context".into(), String::new(), "/clear_all".into()),
    ]);
    items
}
