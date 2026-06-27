use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("provider not found: {0}")]
    ProviderNotFound(String),

    #[error("model not available: {0}")]
    ModelNotAvailable(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("rate limited, retry after {0}s")]
    RateLimited(u64),

    #[error("context length exceeded: max {max}, requested {requested}")]
    ContextLengthExceeded { max: u32, requested: u32 },

    #[error("TOON parse failed: {0}")]
    ToonParseFailed(String),

    #[error("all fallback profiles exhausted")]
    AllFallbacksExhausted,

    #[error("budget exceeded for model call")]
    BudgetExceeded,

    #[error("model error: {0}")]
    Other(String),
}
