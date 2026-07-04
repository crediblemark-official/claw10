//! Known AI provider configurations for OpenAI-compatible APIs.
//!
//! Each entry defines: provider name, base URL, API key env var, and supported models.
//! Use [`provider_configs`] to iterate all known providers, then call
//! [`OpenAiCompatibleProvider::with_config`] to create each one.
//!
//! Dedicated providers (Anthropic, Gemini, etc.) are in their own modules.

use crate::provider::ModelProvider;
use crate::types::ModelProfile;

/// A descriptor for creating an OpenAI-compatible provider.
pub struct ProviderConfig {
    pub name: &'static str,
    pub base_url: &'static str,
    pub api_key_env: &'static str,
    pub notes: &'static str,
    pub models: Vec<ModelProfile>,
    /// Optional native factory for providers that are not OpenAI-compatible.
    /// When set, callers should register the provider via this factory instead
    /// of instantiating an `OpenAiCompatibleProvider`.
    pub factory: Option<fn() -> Box<dyn ModelProvider>>,
}

/// Return configurations for every known OpenAI-compatible provider.
pub fn provider_configs() -> Vec<ProviderConfig> {
    all_configs()
}

/// Look up a single provider slot by name (e.g. "openai", "groq").
/// Returns `None` if the slot is unknown.
pub fn get_provider_slot(name: &str) -> Option<ProviderConfig> {
    all_configs().into_iter().find(|c| c.name == name)
}

fn all_configs() -> Vec<ProviderConfig> {
    let mut configs: Vec<ProviderConfig> = vec![
        // Note: when adding a new provider here, also add a
        // `get_provider_slot`-friendly single-producer fn if needed.
        // ─── Major API Providers ───────────────────────────
        openai(),
        anthropic_compat(),
        mistral(),
        cohere(),
        google_gemini_compat(),
        deepseek(),
        moonshot(),
        xai(),
        // ─── Aggregators / Gateways ────────────────────────
        openrouter(),
        litellm(),
        cloudflare_ai_gateway(),
        vercel_ai_gateway(),
        // ─── Hosted Inference ──────────────────────────────
        together(),
        fireworks(),
        groq(),
        perplexity(),
        cerebras(),
        chutes(),
        inferrs(),
        // ─── China / Asia-Pacific ──────────────────────────
        alibaba_model_studio(),
        qwen_cloud(),
        volcengine(),
        tencent_cloud(),
        baidu_qianfan(),
        stepfun(),
        minimax(),
        zhipu_glm(),
        xiaomi(),
        bytedance_byteplus(),
        // ─── Specialised / Niche ───────────────────────────
        novita(),
        kilocode(),
        venice(),
        synthex(),
        gradium(),
        gmi_cloud(),
        arcee(),
        chutes_alt(),
        // ─── Local / Self-hosted ───────────────────────────
        ollama(),
        lm_studio(),
        sglang(),
        vllm(),
        ds4_local(),
        // ─── Image / Audio / Video ─────────────────────────
        fal_ai(),
        runway(),
        comfyui(),
        elevenlabs(),
        // ─── Other ─────────────────────────────────────────
        github_copilot(),
        huggingface(),
        sense_audio(),
        vydra(),
        azure_speech(),
        nvidia(),
        // ─── OpenCode variants ─────────────────────────────
        opencode(),
        opencode_go(),
    ];

    // Fill in the provider field for every model so routing works
    for cfg in &mut configs {
        for m in &mut cfg.models {
            m.provider = cfg.name.to_string();
        }
    }

    configs
}

// ── Helpers ──────────────────────────────────────────

fn model(
    id: &str,
    ctx: u32,
    max_out: u32,
    cost_in: f64,
    cost_out: f64,
    suitable: Vec<&str>,
) -> ModelProfile {
    ModelProfile {
        id: id.to_string(),
        provider: String::new(), // filled when registered
        model_name: id.to_string(),
        context_window: ctx,
        max_output_tokens: max_out,
        cost_per_1m_input: cost_in,
        cost_per_1m_output: cost_out,
        suitable_for: suitable.into_iter().map(|s| s.to_string()).collect(),
    }
}

fn config(
    name: &'static str,
    base_url: &'static str,
    api_key_env: &'static str,
    notes: &'static str,
    models: Vec<ModelProfile>,
) -> ProviderConfig {
    ProviderConfig {
        name,
        base_url,
        api_key_env,
        notes,
        models,
        factory: None,
    }
}

// ── Provider Definitions ─────────────────────────────

fn openai() -> ProviderConfig {
    config(
        "openai",
        "https://api.openai.com/v1",
        "OPENAI_API_KEY",
        "OpenAI API — GPT-4o, GPT-4o-mini, o-series",
        vec![
            model(
                "gpt-4o",
                128_000,
                16_384,
                2.50,
                10.00,
                vec!["general", "reasoning", "coding"],
            ),
            model(
                "gpt-4o-mini",
                128_000,
                16_384,
                0.15,
                0.60,
                vec!["general", "coding", "fast"],
            ),
            model(
                "gpt-4.1",
                1_047_576,
                16_384,
                2.00,
                8.00,
                vec!["general", "reasoning", "coding"],
            ),
            model(
                "gpt-4.1-mini",
                1_047_576,
                16_384,
                0.40,
                1.60,
                vec!["general", "coding"],
            ),
            model(
                "gpt-4.1-nano",
                1_047_576,
                16_384,
                0.10,
                0.40,
                vec!["general", "fast"],
            ),
            model(
                "o3-mini",
                200_000,
                100_000,
                1.10,
                4.40,
                vec!["reasoning", "coding"],
            ),
            model(
                "o4-mini",
                200_000,
                100_000,
                1.10,
                4.40,
                vec!["reasoning", "coding"],
            ),
            model(
                "gpt-4o-audio-preview",
                128_000,
                16_384,
                2.50,
                10.00,
                vec!["audio"],
            ),
        ],
    )
}

fn anthropic_compat() -> ProviderConfig {
    config(
        "anthropic",
        "https://api.anthropic.com/v1",
        "ANTHROPIC_API_KEY",
        "Anthropic — Claude 4 Sonnet, Opus, Haiku (OpenAI-compatible endpoint)",
        vec![
            model(
                "claude-sonnet-4-20250514",
                200_000,
                8_192,
                3.00,
                15.00,
                vec!["general", "reasoning", "coding"],
            ),
            model(
                "claude-3.5-sonnet",
                200_000,
                8_192,
                3.00,
                15.00,
                vec!["general", "reasoning", "coding"],
            ),
            model(
                "claude-3.5-haiku",
                200_000,
                8_192,
                0.80,
                4.00,
                vec!["general", "fast", "coding"],
            ),
            model(
                "claude-3-opus",
                200_000,
                4_096,
                15.00,
                75.00,
                vec!["reasoning"],
            ),
        ],
    )
}

fn openrouter() -> ProviderConfig {
    config(
        "openrouter",
        "https://openrouter.ai/api/v1",
        "OPENROUTER_API_KEY",
        "OpenRouter — unified access to 300+ models",
        vec![
            model(
                "openai/gpt-4o",
                128_000,
                16_384,
                2.50,
                10.00,
                vec!["general", "coding"],
            ),
            model(
                "openai/gpt-4o-mini",
                128_000,
                16_384,
                0.15,
                0.60,
                vec!["general", "fast"],
            ),
            model(
                "openai/o3-mini",
                200_000,
                100_000,
                1.10,
                4.40,
                vec!["reasoning"],
            ),
            model(
                "anthropic/claude-sonnet-4",
                200_000,
                8_192,
                3.00,
                15.00,
                vec!["general", "reasoning"],
            ),
            model(
                "google/gemini-2.0-flash-001",
                1_048_576,
                8_192,
                0.10,
                0.40,
                vec!["general", "fast"],
            ),
            model(
                "google/gemini-2.5-pro-preview-03-25",
                1_048_576,
                64_000,
                1.25,
                5.00,
                vec!["reasoning"],
            ),
            model(
                "deepseek/deepseek-chat",
                64_000,
                8_192,
                0.27,
                1.10,
                vec!["general", "coding"],
            ),
            model(
                "deepseek/deepseek-r1",
                64_000,
                8_192,
                0.55,
                2.19,
                vec!["reasoning", "coding"],
            ),
            model(
                "mistralai/mistral-large-2411",
                128_000,
                8_192,
                2.00,
                6.00,
                vec!["general", "reasoning"],
            ),
            model(
                "cohere/command-r7b-12-2024",
                128_000,
                4_096,
                0.15,
                0.60,
                vec!["general"],
            ),
            model(
                "meta-llama/llama-3.3-70b-instruct",
                128_000,
                8_192,
                0.25,
                1.00,
                vec!["general"],
            ),
            model(
                "qwen/qwen-2.5-72b-instruct",
                32_768,
                8_192,
                0.35,
                1.40,
                vec!["general", "coding"],
            ),
        ],
    )
}

fn deepseek() -> ProviderConfig {
    config(
        "deepseek",
        "https://api.deepseek.com",
        "DEEPSEEK_API_KEY",
        "DeepSeek — V3, R1 reasoning model",
        vec![
            model(
                "deepseek-chat",
                64_000,
                8_192,
                0.27,
                1.10,
                vec!["general", "coding"],
            ),
            model(
                "deepseek-reasoner",
                64_000,
                8_192,
                0.55,
                2.19,
                vec!["reasoning", "coding"],
            ),
            model(
                "deepseek-v3",
                64_000,
                8_192,
                0.27,
                1.10,
                vec!["general", "coding"],
            ),
            model("deepseek-r1", 64_000, 8_192, 0.55, 2.19, vec!["reasoning"]),
        ],
    )
}

fn moonshot() -> ProviderConfig {
    config(
        "moonshot",
        "https://api.moonshot.cn/v1",
        "MOONSHOT_API_KEY",
        "Moonshot AI — Kimi K2, K2.6",
        vec![
            model(
                "moonshotai/kimi-k2.6",
                128_000,
                16_384,
                1.00,
                3.00,
                vec!["general", "reasoning", "coding", "planning"],
            ),
            model(
                "moonshotai/kimi-k2",
                128_000,
                16_384,
                0.80,
                2.50,
                vec!["general", "coding"],
            ),
        ],
    )
}

fn mistral() -> ProviderConfig {
    config(
        "mistral",
        "https://api.mistral.ai/v1",
        "MISTRAL_API_KEY",
        "Mistral AI — Mistral Large, Small, Codestral",
        vec![
            model(
                "mistral-large-2411",
                128_000,
                8_192,
                2.00,
                6.00,
                vec!["general", "reasoning"],
            ),
            model(
                "mistral-small-2501",
                32_000,
                8_192,
                0.10,
                0.30,
                vec!["general", "fast"],
            ),
            model("codestral-2501", 256_000, 8_192, 1.00, 3.00, vec!["coding"]),
            model(
                "mistral-moderation-2411",
                32_000,
                4_096,
                0.01,
                0.01,
                vec!["moderation"],
            ),
        ],
    )
}

fn cohere() -> ProviderConfig {
    config(
        "cohere",
        "https://api.cohere.ai/v1",
        "COHERE_API_KEY",
        "Cohere — Command R7B, embed, rerank",
        vec![
            model(
                "command-r7b-12-2024",
                128_000,
                4_096,
                0.15,
                0.60,
                vec!["general", "rag"],
            ),
            model(
                "command-r-plus-08-2024",
                128_000,
                4_096,
                2.50,
                10.00,
                vec!["general", "reasoning"],
            ),
            model(
                "command-light",
                4_096,
                4_096,
                0.30,
                0.60,
                vec!["general", "fast"],
            ),
        ],
    )
}

fn together() -> ProviderConfig {
    config(
        "together",
        "https://api.together.xyz/v1",
        "TOGETHER_API_KEY",
        "Together AI — hosted open models",
        vec![
            model(
                "meta-llama/Llama-3.3-70B-Instruct-Turbo",
                128_000,
                8_192,
                0.40,
                1.60,
                vec!["general", "reasoning"],
            ),
            model(
                "meta-llama/Llama-3.2-90B-Vision-Instruct-Turbo",
                128_000,
                8_192,
                0.80,
                3.20,
                vec!["general", "vision"],
            ),
            model(
                "mistralai/Mixtral-8x22B-Instruct-v0.1",
                128_000,
                8_192,
                0.60,
                2.40,
                vec!["general"],
            ),
            model(
                "Qwen/Qwen2.5-72B-Instruct-Turbo",
                32_768,
                8_192,
                0.35,
                1.40,
                vec!["general", "coding"],
            ),
            model(
                "deepseek-ai/DeepSeek-R1",
                64_000,
                8_192,
                3.50,
                7.00,
                vec!["reasoning", "coding"],
            ),
        ],
    )
}

fn fireworks() -> ProviderConfig {
    config(
        "fireworks",
        "https://api.fireworks.ai/inference/v1",
        "FIREWORKS_API_KEY",
        "Fireworks AI — fast inference for open models",
        vec![
            model(
                "accounts/fireworks/models/llama-v3p3-70b-instruct",
                128_000,
                8_192,
                0.50,
                2.00,
                vec!["general"],
            ),
            model(
                "accounts/fireworks/models/deepseek-r1",
                64_000,
                8_192,
                4.00,
                8.00,
                vec!["reasoning"],
            ),
            model(
                "accounts/fireworks/models/qwen2p5-72b-instruct",
                32_768,
                8_192,
                0.40,
                1.60,
                vec!["general", "coding"],
            ),
            model(
                "accounts/fireworks/models/mixtral-8x22b-instruct",
                128_000,
                8_192,
                0.80,
                3.20,
                vec!["general"],
            ),
        ],
    )
}

fn groq() -> ProviderConfig {
    config(
        "groq",
        "https://api.groq.com/openai/v1",
        "GROQ_API_KEY",
        "Groq — LPU inference, extremely fast",
        vec![
            model(
                "llama-3.3-70b-versatile",
                128_000,
                32_768,
                0.59,
                0.79,
                vec!["general", "reasoning", "fast"],
            ),
            model(
                "llama-3.1-8b-instant",
                128_000,
                8_192,
                0.03,
                0.03,
                vec!["general", "fast"],
            ),
            model(
                "mixtral-8x7b-32768",
                32_768,
                8_192,
                0.27,
                0.27,
                vec!["general"],
            ),
            model(
                "gemma2-9b-it",
                8_192,
                8_192,
                0.10,
                0.10,
                vec!["general", "fast"],
            ),
            model(
                "deepseek-r1-distill-llama-70b",
                128_000,
                16_384,
                0.75,
                0.99,
                vec!["reasoning"],
            ),
        ],
    )
}

fn perplexity() -> ProviderConfig {
    config(
        "perplexity",
        "https://api.perplexity.ai",
        "PERPLEXITY_API_KEY",
        "Perplexity — LLM with web search capability",
        vec![
            model(
                "sonar-pro",
                128_000,
                4_096,
                1.00,
                1.00,
                vec!["general", "research", "search"],
            ),
            model(
                "sonar",
                128_000,
                4_096,
                0.50,
                0.50,
                vec!["general", "search"],
            ),
            model(
                "sonar-deep-research",
                128_000,
                8_192,
                2.00,
                2.00,
                vec!["research", "deep"],
            ),
        ],
    )
}

fn cerebras() -> ProviderConfig {
    config(
        "cerebras",
        "https://api.cerebras.ai/v1",
        "CEREBRAS_API_KEY",
        "Cerebras — wafer-scale inference",
        vec![model(
            "cerebras/llama-3.3-70b",
            128_000,
            8_192,
            0.50,
            2.00,
            vec!["general"],
        )],
    )
}

fn xai() -> ProviderConfig {
    config(
        "xai",
        "https://api.x.ai/v1",
        "XAI_API_KEY",
        "xAI — Grok models",
        vec![
            model(
                "grok-3-beta",
                128_000,
                8_192,
                3.00,
                15.00,
                vec!["general", "reasoning", "coding"],
            ),
            model(
                "grok-2-1212",
                128_000,
                8_192,
                2.00,
                10.00,
                vec!["general", "reasoning"],
            ),
            model(
                "grok-2-vision-1212",
                16_384,
                8_192,
                2.00,
                10.00,
                vec!["general", "vision"],
            ),
        ],
    )
}

fn google_gemini_compat() -> ProviderConfig {
    config(
        "google-gemini",
        "https://generativelanguage.googleapis.com/v1beta/openai",
        "GEMINI_API_KEY",
        "Google Gemini — OpenAI-compatible endpoint",
        vec![
            model(
                "gemini-2.5-pro-exp-03-25",
                1_048_576,
                64_000,
                1.25,
                5.00,
                vec!["reasoning", "coding", "general"],
            ),
            model(
                "gemini-2.0-flash",
                1_048_576,
                8_192,
                0.10,
                0.40,
                vec!["general", "fast", "vision"],
            ),
            model(
                "gemini-2.0-flash-lite",
                1_048_576,
                8_192,
                0.075,
                0.30,
                vec!["general", "fast"],
            ),
            model(
                "gemini-1.5-pro",
                2_097_152,
                8_192,
                1.25,
                5.00,
                vec!["general", "reasoning"],
            ),
            model(
                "gemini-1.5-flash",
                1_048_576,
                8_192,
                0.075,
                0.30,
                vec!["general", "fast"],
            ),
        ],
    )
}

fn alibaba_model_studio() -> ProviderConfig {
    config(
        "alibaba",
        "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "ALIBABA_API_KEY",
        "Alibaba Cloud Model Studio — Qwen models",
        vec![
            model(
                "qwen-max",
                32_768,
                8_192,
                1.60,
                6.40,
                vec!["general", "reasoning"],
            ),
            model(
                "qwen-plus",
                131_072,
                8_192,
                0.40,
                1.60,
                vec!["general", "coding"],
            ),
            model(
                "qwen-turbo",
                131_072,
                8_192,
                0.10,
                0.40,
                vec!["general", "fast"],
            ),
            model(
                "qwen2.5-72b-instruct",
                32_768,
                8_192,
                0.80,
                3.20,
                vec!["general"],
            ),
        ],
    )
}

fn qwen_cloud() -> ProviderConfig {
    config(
        "qwen-cloud",
        "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        "QWEN_API_KEY",
        "Qwen Cloud (international region)",
        vec![
            model(
                "qwen-max",
                32_768,
                8_192,
                1.60,
                6.40,
                vec!["general", "reasoning"],
            ),
            model(
                "qwen-plus",
                131_072,
                8_192,
                0.40,
                1.60,
                vec!["general", "coding"],
            ),
            model(
                "qwen-turbo",
                131_072,
                8_192,
                0.10,
                0.40,
                vec!["general", "fast"],
            ),
        ],
    )
}

fn volcengine() -> ProviderConfig {
    config(
        "volcengine",
        "https://ark.cn-beijing.volces.com/api/v3",
        "VOLCENGINE_API_KEY",
        "Volcengine — Doubao models (ByteDance)",
        vec![
            model("doubao-pro-32k", 32_768, 8_192, 0.40, 1.60, vec!["general"]),
            model(
                "doubao-pro-128k",
                128_000,
                8_192,
                1.20,
                4.80,
                vec!["general", "long"],
            ),
            model(
                "doubao-lite-32k",
                32_768,
                8_192,
                0.10,
                0.40,
                vec!["general", "fast"],
            ),
        ],
    )
}

fn tencent_cloud() -> ProviderConfig {
    config(
        "tencent",
        "https://api.hunyuan.cloud.tencent.com/v1",
        "TENCENT_API_KEY",
        "Tencent Cloud — Hunyuan (TokenHub)",
        vec![
            model(
                "hunyuan-turbo",
                256_000,
                8_192,
                1.20,
                4.80,
                vec!["general", "reasoning"],
            ),
            model(
                "hunyuan-standard",
                256_000,
                8_192,
                0.30,
                1.20,
                vec!["general"],
            ),
        ],
    )
}

fn baidu_qianfan() -> ProviderConfig {
    config(
        "qianfan",
        "https://aip.baidubce.com/rpc/2.0/ai_custom/v1/wenxinworkshop/chat",
        "QIANFAN_API_KEY",
        "Baidu Qianfan — ERNIE models",
        vec![
            model(
                "ernie-4.0-8k",
                8_192,
                4_096,
                1.20,
                4.80,
                vec!["general", "reasoning"],
            ),
            model("ernie-3.5-8k", 8_192, 4_096, 0.30, 1.20, vec!["general"]),
        ],
    )
}

fn stepfun() -> ProviderConfig {
    config(
        "stepfun",
        "https://api.stepfun.com/v1",
        "STEPFUN_API_KEY",
        "StepFun — Step-2, Step-1 models",
        vec![
            model("step-2-16k", 16_384, 8_192, 0.50, 2.00, vec!["general"]),
            model(
                "step-1-8k",
                8_192,
                8_192,
                0.20,
                0.80,
                vec!["general", "fast"],
            ),
        ],
    )
}

fn minimax() -> ProviderConfig {
    config(
        "minimax",
        "https://api.minimax.chat/v1",
        "MINIMAX_API_KEY",
        "MiniMax — MiniMax-Text, MiniMax-VL",
        vec![
            model(
                "minimax-text-01",
                128_000,
                8_192,
                0.40,
                1.60,
                vec!["general"],
            ),
            model(
                "minimax-vl-01",
                128_000,
                8_192,
                0.60,
                2.40,
                vec!["general", "vision"],
            ),
        ],
    )
}

fn zhipu_glm() -> ProviderConfig {
    config(
        "zhipu",
        "https://open.bigmodel.cn/api/paas/v4",
        "ZHIPU_API_KEY",
        "Zhipu AI — GLM-4 series",
        vec![
            model(
                "glm-4-plus",
                128_000,
                8_192,
                0.50,
                2.00,
                vec!["general", "reasoning"],
            ),
            model(
                "glm-4-flash",
                128_000,
                8_192,
                0.10,
                0.40,
                vec!["general", "fast"],
            ),
            model(
                "glm-4v-plus",
                128_000,
                8_192,
                0.80,
                3.20,
                vec!["general", "vision"],
            ),
        ],
    )
}

fn xiaomi() -> ProviderConfig {
    config(
        "xiaomi",
        "https://api.mi.com/v1",
        "XIAOMI_API_KEY",
        "Xiaomi AI — MiLM models",
        vec![model(
            "milv-2.0",
            32_768,
            8_192,
            0.30,
            1.20,
            vec!["general"],
        )],
    )
}

fn bytedance_byteplus() -> ProviderConfig {
    config(
        "byteplus",
        "https://ark.byteplus.com/api/v3",
        "BYTEPLUS_API_KEY",
        "BytePlus (International) — Doubao models",
        vec![
            model("doubao-pro-32k", 32_768, 8_192, 0.40, 1.60, vec!["general"]),
            model(
                "doubao-lite-32k",
                32_768,
                8_192,
                0.10,
                0.40,
                vec!["general", "fast"],
            ),
        ],
    )
}

fn nvidia() -> ProviderConfig {
    config(
        "nvidia",
        "https://integrate.api.nvidia.com/v1",
        "NVIDIA_API_KEY",
        "NVIDIA NIM — hosted GPU inference",
        vec![
            model(
                "moonshotai/kimi-k2.6",
                128_000,
                16_384,
                1.00,
                3.00,
                vec!["general", "reasoning", "coding", "planning"],
            ),
            model(
                "meta/llama-3.3-70b-instruct",
                128_000,
                16_384,
                0.50,
                1.50,
                vec!["general", "reasoning"],
            ),
            model(
                "mistralai/mistral-nemo-12b-instruct",
                128_000,
                16_384,
                0.20,
                0.60,
                vec!["general", "fast"],
            ),
            model(
                "nvidia/llama-3.1-nemotron-70b-instruct",
                128_000,
                16_384,
                0.50,
                1.50,
                vec!["reasoning", "coding"],
            ),
        ],
    )
}

fn ollama() -> ProviderConfig {
    config(
        "ollama",
        "http://localhost:11434/v1",
        "OLLAMA_API_KEY",
        "Ollama — local models (default http://localhost:11434)",
        vec![
            model(
                "llama3.3:70b",
                128_000,
                8_192,
                0.00,
                0.00,
                vec!["general", "local"],
            ),
            model(
                "llama3.2:3b",
                128_000,
                8_192,
                0.00,
                0.00,
                vec!["general", "fast", "local"],
            ),
            model(
                "qwen2.5:72b",
                32_768,
                8_192,
                0.00,
                0.00,
                vec!["general", "local"],
            ),
            model(
                "mistral:7b",
                32_768,
                8_192,
                0.00,
                0.00,
                vec!["general", "fast", "local"],
            ),
            model(
                "deepseek-r1:70b",
                128_000,
                8_192,
                0.00,
                0.00,
                vec!["reasoning", "local"],
            ),
            model(
                "phi4:14b",
                16_384,
                8_192,
                0.00,
                0.00,
                vec!["general", "local"],
            ),
        ],
    )
}

fn lm_studio() -> ProviderConfig {
    config(
        "lm-studio",
        "http://localhost:1234/v1",
        "LM_STUDIO_API_KEY",
        "LM Studio — local models (default http://localhost:1234)",
        vec![model(
            "local-model",
            32_768,
            8_192,
            0.00,
            0.00,
            vec!["general", "local"],
        )],
    )
}

fn sglang() -> ProviderConfig {
    config(
        "sglang",
        "http://localhost:30000/v1",
        "SGLANG_API_KEY",
        "SGLang — local inference server",
        vec![model(
            "local-model",
            32_768,
            8_192,
            0.00,
            0.00,
            vec!["general", "local"],
        )],
    )
}

fn vllm() -> ProviderConfig {
    config(
        "vllm",
        "http://localhost:8000/v1",
        "VLLM_API_KEY",
        "vLLM — local inference server",
        vec![model(
            "local-model",
            32_768,
            8_192,
            0.00,
            0.00,
            vec!["general", "local"],
        )],
    )
}

fn ds4_local() -> ProviderConfig {
    config(
        "ds4-local",
        "http://localhost:8080/v1",
        "DS4_API_KEY",
        "DeepSeek V4 locally hosted",
        vec![model(
            "deepseek-v4-local",
            128_000,
            16_384,
            0.00,
            0.00,
            vec!["general", "reasoning", "local"],
        )],
    )
}

fn novita() -> ProviderConfig {
    config(
        "novita",
        "https://api.novita.ai/v3/openai",
        "NOVITA_API_KEY",
        "Novita AI — hosted open models",
        vec![
            model(
                "meta-llama/llama-3.3-70b-instruct",
                128_000,
                8_192,
                0.45,
                1.80,
                vec!["general"],
            ),
            model(
                "deepseek/deepseek-r1",
                64_000,
                8_192,
                0.80,
                3.20,
                vec!["reasoning"],
            ),
        ],
    )
}

fn kilocode() -> ProviderConfig {
    config(
        "kilocode",
        "https://api.kilocode.ai/v1",
        "KILOCODE_API_KEY",
        "Kilocode — coding-focused models",
        vec![model(
            "kilocode-v1",
            128_000,
            16_384,
            0.50,
            2.00,
            vec!["coding"],
        )],
    )
}

fn venice() -> ProviderConfig {
    config(
        "venice",
        "https://api.venice.ai/api/v1",
        "VENICE_API_KEY",
        "Venice AI — privacy-focused inference",
        vec![model(
            "venice-v1",
            32_768,
            8_192,
            0.50,
            2.00,
            vec!["general"],
        )],
    )
}

fn synthex() -> ProviderConfig {
    config(
        "synthetic",
        "https://api.synthetic.ai/v1",
        "SYNTHETIC_API_KEY",
        "Synthetic AI — enterprise models",
        vec![model(
            "synthetic-v1",
            128_000,
            8_192,
            1.00,
            4.00,
            vec!["general"],
        )],
    )
}

fn gradium() -> ProviderConfig {
    config(
        "gradium",
        "https://api.gradium.ai/v1",
        "GRADIUM_API_KEY",
        "Gradium AI inference",
        vec![model(
            "gradium-v1",
            32_768,
            8_192,
            0.50,
            2.00,
            vec!["general"],
        )],
    )
}

fn gmi_cloud() -> ProviderConfig {
    config(
        "gmi-cloud",
        "https://api.gmicloud.ai/v1",
        "GMI_CLOUD_API_KEY",
        "GMI Cloud — GPU cloud inference",
        vec![model("gmi-v1", 32_768, 8_192, 0.50, 2.00, vec!["general"])],
    )
}

fn arcee() -> ProviderConfig {
    config(
        "arcee",
        "https://api.arcee.ai/v1",
        "ARCEE_API_KEY",
        "Arcee AI — Trinity, specialized models",
        vec![model(
            "trinity-v1",
            32_768,
            8_192,
            0.50,
            2.00,
            vec!["general", "coding"],
        )],
    )
}

fn chutes() -> ProviderConfig {
    config(
        "chutes",
        "https://api.chutes.ai/v1",
        "CHUTES_API_KEY",
        "Chutes AI — hosted open models",
        vec![model(
            "chutes-v1",
            32_768,
            8_192,
            0.40,
            1.60,
            vec!["general"],
        )],
    )
}

fn inferrs() -> ProviderConfig {
    config(
        "inferrs",
        "https://api.inferrs.ai/v1",
        "INFERRS_API_KEY",
        "Inferrs — local/cloud model hosting",
        vec![model(
            "inferrs-v1",
            32_768,
            8_192,
            0.30,
            1.20,
            vec!["general"],
        )],
    )
}

fn litellm() -> ProviderConfig {
    config(
        "litellm",
        "http://localhost:4000/v1",
        "LITELLM_API_KEY",
        "LiteLLM — unified proxy gateway (default http://localhost:4000)",
        vec![model(
            "litellm-proxy",
            128_000,
            16_384,
            0.00,
            0.00,
            vec!["general", "gateway"],
        )],
    )
}

fn cloudflare_ai_gateway() -> ProviderConfig {
    config(
        "cloudflare-ai-gateway",
        "https://gateway.ai.cloudflare.com/v1",
        "CLOUDFLARE_AI_GATEWAY_KEY",
        "Cloudflare AI Gateway",
        vec![model(
            "cf-gateway",
            128_000,
            16_384,
            0.00,
            0.00,
            vec!["general", "gateway"],
        )],
    )
}

fn vercel_ai_gateway() -> ProviderConfig {
    config(
        "vercel-ai-gateway",
        "https://gateway.vercel.ai/v1",
        "VERCEL_AI_GATEWAY_KEY",
        "Vercel AI Gateway",
        vec![model(
            "vercel-gateway",
            128_000,
            16_384,
            0.00,
            0.00,
            vec!["general", "gateway"],
        )],
    )
}

fn fal_ai() -> ProviderConfig {
    config(
        "fal",
        "https://api.fal.ai/v1",
        "FAL_API_KEY",
        "fal.ai — image, video, audio generation",
        vec![model(
            "fal-ai/default",
            4_096,
            4_096,
            0.50,
            0.50,
            vec!["image", "video"],
        )],
    )
}

fn runway() -> ProviderConfig {
    config(
        "runway",
        "https://api.runwayml.com/v1",
        "RUNWAY_API_KEY",
        "Runway ML — video generation models",
        vec![model(
            "runway/gen3",
            4_096,
            4_096,
            2.00,
            2.00,
            vec!["video"],
        )],
    )
}

fn comfyui() -> ProviderConfig {
    config(
        "comfyui",
        "http://localhost:8188",
        "COMFYUI_API_KEY",
        "ComfyUI — local image generation",
        vec![model(
            "comfyui/default",
            4_096,
            4_096,
            0.00,
            0.00,
            vec!["image", "local"],
        )],
    )
}

fn elevenlabs() -> ProviderConfig {
    config(
        "elevenlabs",
        "https://api.elevenlabs.io/v1",
        "ELEVENLABS_API_KEY",
        "ElevenLabs — text-to-speech, voice cloning",
        vec![model(
            "elevenlabs/default",
            4_096,
            4_096,
            0.50,
            0.50,
            vec!["audio", "tts"],
        )],
    )
}

fn github_copilot() -> ProviderConfig {
    config(
        "github-copilot",
        "https://api.githubcopilot.com",
        "GITHUB_TOKEN",
        "GitHub Copilot — code completion & chat",
        vec![
            model("copilot/gpt-4o", 128_000, 8_192, 0.00, 0.00, vec!["coding"]),
            model(
                "copilot/claude-sonnet",
                200_000,
                8_192,
                0.00,
                0.00,
                vec!["coding"],
            ),
        ],
    )
}

fn huggingface() -> ProviderConfig {
    config(
        "huggingface",
        "https://api-inference.huggingface.co/v1",
        "HUGGINGFACE_API_KEY",
        "Hugging Face Inference API",
        vec![
            model(
                "meta-llama/Llama-3.3-70B-Instruct",
                128_000,
                8_192,
                0.20,
                0.80,
                vec!["general"],
            ),
            model(
                "microsoft/Phi-4",
                16_384,
                8_192,
                0.10,
                0.40,
                vec!["general", "fast"],
            ),
            model(
                "google/gemma-2-27b-it",
                8_192,
                8_192,
                0.15,
                0.60,
                vec!["general"],
            ),
        ],
    )
}

fn sense_audio() -> ProviderConfig {
    config(
        "senseaudio",
        "https://api.senseaudio.ai/v1",
        "SENSEAUDIO_API_KEY",
        "SenseAudio — audio generation models",
        vec![model(
            "senseaudio/default",
            4_096,
            4_096,
            0.50,
            0.50,
            vec!["audio"],
        )],
    )
}

fn vydra() -> ProviderConfig {
    config(
        "vydra",
        "https://api.vydra.ai/v1",
        "VYDRA_API_KEY",
        "Vydra AI inference",
        vec![model(
            "vydra-v1",
            32_768,
            8_192,
            0.30,
            1.20,
            vec!["general"],
        )],
    )
}

fn opencode() -> ProviderConfig {
    config(
        "opencode",
        "https://api.opencode.ai/v1",
        "OPENCODE_API_KEY",
        "OpenCode AI agent platform",
        vec![model(
            "opencode-v1",
            128_000,
            16_384,
            0.50,
            2.00,
            vec!["coding", "general"],
        )],
    )
}

fn opencode_go() -> ProviderConfig {
    config(
        "opencode-go",
        "https://api.opencode.ai/v1",
        "OPENCODE_GO_API_KEY",
        "OpenCode Go — Golang-focused agent",
        vec![model(
            "opencode-go-v1",
            128_000,
            16_384,
            0.50,
            2.00,
            vec!["coding"],
        )],
    )
}

fn chutes_alt() -> ProviderConfig {
    config(
        "chutes-alt",
        "https://alt.chutes.ai/v1",
        "CHUTES_ALT_API_KEY",
        "Chutes (alternative endpoint)",
        vec![model(
            "chutes-alt-v1",
            32_768,
            8_192,
            0.40,
            1.60,
            vec!["general"],
        )],
    )
}

fn azure_speech() -> ProviderConfig {
    config(
        "azure-speech",
        "https://api.cognitive.microsoft.com/stt/v1",
        "AZURE_SPEECH_KEY",
        "Azure Speech Services — STT / TTS",
        vec![model(
            "azure-speech/default",
            4_096,
            4_096,
            0.00,
            0.00,
            vec!["audio", "stt", "tts"],
        )],
    )
}
