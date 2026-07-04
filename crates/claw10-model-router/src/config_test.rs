use super::*;

#[test]
fn test_parse_minimal_config() {
    let toml_str = r#"
[alias.gpt4]
slot = "openai"
model = "gpt-4o"
api_key = "$OPENAI_API_KEY"

[alias.haiku]
slot = "anthropic"
model = "claude-3.5-haiku"
api_key = "sk-ant-fake123"
"#;
    let config: Claw10Config = toml::from_str(toml_str).expect("should parse");
    assert_eq!(config.alias.len(), 2);
    assert!(config.custom.is_empty());

    let gpt4 = config.alias.get("gpt4").unwrap();
    assert_eq!(gpt4.slot, "openai");
    assert_eq!(gpt4.model, "gpt-4o");
    assert_eq!(gpt4.api_key, "$OPENAI_API_KEY");
}

#[test]
fn test_parse_custom_provider() {
    let toml_str = r#"
[custom.my-llm]
base_url = "https://my-llm.example.com/v1"
api_key = "$MY_LLM_KEY"
models = ["my-model-v1", "my-model-v2"]

[custom.my-llm.model_meta."my-model-v1"]
context_window = 128000
max_output_tokens = 16384
cost_per_1m_input = 1.0
cost_per_1m_output = 3.0
"#;
    let config: Claw10Config = toml::from_str(toml_str).expect("should parse");
    let custom = config.custom.get("my-llm").unwrap();
    assert_eq!(custom.base_url, "https://my-llm.example.com/v1");
    assert_eq!(custom.models.len(), 2);
    let meta = custom.model_meta.get("my-model-v1").unwrap();
    assert_eq!(meta.context_window, 128_000);
    assert_eq!(meta.cost_per_1m_input, 1.0);
}

#[test]
fn test_resolve_api_key_inline() {
    let kv = |_: &str| None;
    let key = resolve_api_key("sk-real-key", "", &kv);
    assert_eq!(key, Some("sk-real-key".to_string()));
}

#[test]
fn test_resolve_api_key_env_ref() {
    let kv = |k: &str| {
        if k == "config:test_openai_key_api_key" {
            Some("sk-test-env".to_string())
        } else {
            None
        }
    };
    let key = resolve_api_key("$TEST_OPENAI_KEY", "", &kv);
    assert_eq!(key, Some("sk-test-env".to_string()));
}

#[test]
fn test_resolve_api_key_empty_ref_falls_to_slot_env() {
    let kv = |k: &str| {
        if k == "config:test_slot_env_api_key" {
            Some("sk-slot-fallback".to_string())
        } else {
            None
        }
    };
    let key = resolve_api_key("", "TEST_SLOT_ENV", &kv);
    assert_eq!(key, Some("sk-slot-fallback".to_string()));
}

#[test]
fn test_config_discovery_candidates() {
    let candidates = config_file_candidates();
    assert!(!candidates.is_empty());
    // First candidate should be from CLAW10_CONFIG env var if set,
    // otherwise from cwd
    assert!(candidates.iter().any(|p| p.ends_with("claw10.toml")));
}

#[test]
fn test_resolve_providers_with_aliases() {
    use crate::providers;
    let builtin = providers::provider_configs();
    let config_toml = r#"
[alias.gpt4]
slot = "openai"
model = "gpt-4o"
api_key = "sk-test-openai"
"#;
    let config: Claw10Config = toml::from_str(config_toml).unwrap();
    let kv = |_: &str| None;
    let (resolved, errors) = resolve_providers(Some(&config), builtin, kv);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(!resolved.is_empty(), "should resolve at least the alias");
    let alias = resolved.iter().find(|r| r.name == "openai.gpt4");
    assert!(alias.is_some(), "should have openai.gpt4 alias");
    if let Some(a) = alias {
        assert_eq!(a.base_url, "https://api.openai.com/v1");
        assert_eq!(a.api_key, "sk-test-openai");
        assert!(!a.models.is_empty());
        assert_eq!(a.models[0].id, "gpt-4o");
    }
}

#[test]
fn test_resolve_providers_bare_slot() {
    use crate::providers;
    let builtin = providers::provider_configs();
    let config: Option<Claw10Config> = None;
    let kv = |k: &str| {
        if k == "config:openai_api_key_api_key" {
            Some("sk-bare-test".to_string())
        } else {
            None
        }
    };
    let (resolved, errors) = resolve_providers(config.as_ref(), builtin, kv);
    assert!(errors.is_empty());
    let openai = resolved.iter().find(|r| r.name == "openai");
    assert!(openai.is_some(), "openai should resolve from KV");
    if let Some(o) = openai {
        assert_eq!(o.base_url, "https://api.openai.com/v1");
        assert_eq!(o.api_key, "sk-bare-test");
    }
}

#[test]
fn test_resolve_providers_custom() {
    use crate::providers;
    let builtin = providers::provider_configs();
    let config_toml = r#"
[custom.my-llm]
base_url = "https://my-llm.example.com/v1"
api_key = "sk-custom"
models = ["my-model"]
"#;
    let config: Claw10Config = toml::from_str(config_toml).unwrap();
    let kv = |_: &str| None;
    let (resolved, errors) = resolve_providers(Some(&config), builtin, kv);
    assert!(errors.is_empty());
    let custom = resolved.iter().find(|r| r.name == "custom.my-llm");
    assert!(custom.is_some(), "should have custom.my-llm");
    if let Some(c) = custom {
        assert_eq!(c.base_url, "https://my-llm.example.com/v1");
        assert_eq!(c.models.len(), 1);
        assert_eq!(c.models[0].id, "my-model");
        assert_eq!(c.models[0].provider, "custom.my-llm");
    }
}
