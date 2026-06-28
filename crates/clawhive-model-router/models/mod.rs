pub mod nvidia;
pub mod openrouter;

pub fn resolve_static_model(name: &str, provider: &str) -> String {
    if name.contains('/') {
        return name.to_string();
    }
    
    if provider == "nvidia" {
        let name_lower = name.to_lowercase();
        let prefix = if name_lower.starts_with("llama") {
            "meta"
        } else if name_lower.starts_with("mistral") || name_lower.starts_with("ministral") || name_lower.starts_with("mixtral") {
            "mistralai"
        } else if name_lower.starts_with("kimi") {
            "moonshotai"
        } else if name_lower.starts_with("gemma") || name_lower.starts_with("diffusiongemma") {
            "google"
        } else if name_lower.starts_with("qwen") {
            "qwen"
        } else if name_lower.starts_with("minimax") {
            "minimaxai"
        } else if name_lower.starts_with("deepseek") {
            "deepseek-ai"
        } else if name_lower.starts_with("phi") {
            "microsoft"
        } else if name_lower.starts_with("cohere") {
            "cohere"
        } else if name_lower.starts_with("step") {
            "stepfun-ai"
        } else if name_lower.starts_with("glm") {
            "z-ai"
        } else if name_lower.starts_with("sarvam") {
            "sarvamai"
        } else if name_lower.starts_with("dracarys") {
            "abacusai"
        } else if name_lower.starts_with("stockmark") {
            "stockmark"
        } else if name_lower.starts_with("seed-oss") {
            "bytedance"
        } else {
            "nvidia"
        };
        format!("{}/{}", prefix, name)
    } else if provider == "openrouter" {
        let name_lower = name.to_lowercase();
        let prefix = if name_lower.starts_with("gpt-4") || name_lower.starts_with("gpt-3.5") || name_lower.starts_with("o1") || name_lower.starts_with("o3") {
            "openai"
        } else if name_lower.starts_with("claude") {
            "anthropic"
        } else if name_lower.starts_with("gemini") {
            "google"
        } else if name_lower.starts_with("deepseek") {
            "deepseek"
        } else if name_lower.starts_with("llama") {
            "meta-llama"
        } else if name_lower.starts_with("mistral") || name_lower.starts_with("ministral") {
            "mistralai"
        } else if name_lower.starts_with("qwen") {
            "qwen"
        } else if name_lower.starts_with("cohere") || name_lower.starts_with("command") {
            "cohere"
        } else {
            "openrouter"
        };
        format!("{}/{}", prefix, name)
    } else {
        name.to_string()
    }
}
