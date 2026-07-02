use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::env;

/// Middleware Axum untuk memvalidasi static API key dari environment variable `CLAW10_API_KEY`.
/// Mendukung pengecekan header `X-Api-Key` atau `Authorization: Bearer <key>`.
/// Jika `CLAW10_API_KEY` tidak diset atau kosong, verifikasi di-bypass (dev/local mode).
pub async fn auth_middleware(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    if let Ok(expected_key) = env::var("CLAW10_API_KEY")
        && !expected_key.is_empty() {
            let actual_key = req
                .headers()
                .get("x-api-key")
                .or_else(|| req.headers().get("authorization"))
                .and_then(|value| value.to_str().ok())
                .map(|s| {
                    if s.starts_with("Bearer ") {
                        s[7..].trim()
                    } else {
                        s.trim()
                    }
                });

            match actual_key {
                Some(key) if key == expected_key => {
                    // Autentikasi berhasil
                }
                _ => {
                    tracing::warn!("Request unauthorized: API key tidak cocok atau tidak disediakan");
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
        }

    Ok(next.run(req).await)
}
