#![allow(clippy::pedantic)]

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use claw10_domain::{Channel, ChannelType, IdentityId, IncomingMessage, Session, SessionState};
use claw10_store::{Store, StoreError, StoreExt};

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("channel not found: {0}")]
    ChannelNotFound(String),
    #[error("channel {0} is inactive")]
    ChannelInactive(String),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("session {0} is expired")]
    SessionExpired(String),
    #[error("unsupported channel type for dispatch: {0:?}")]
    UnsupportedDispatch(ChannelType),
    #[error("{0}")]
    Other(String),
}

impl From<StoreError> for GatewayError {
    fn from(e: StoreError) -> Self {
        Self::Other(e.to_string())
    }
}

/// A message to be dispatched through a channel.
#[derive(Debug, Clone)]
pub struct Message {
    pub recipient: String,
    pub subject: Option<String>,
    pub body: String,
    pub metadata: Option<serde_json::Value>,
}

/// Result of a message dispatch.
#[derive(Debug, Clone)]
pub struct DispatchResult {
    pub channel_id: String,
    pub success: bool,
    pub response: Option<String>,
    pub dispatched_at: DateTime<Utc>,
}

/// Result of processing an incoming webhook.
#[derive(Debug, Clone)]
pub struct IncomingWebhookResult {
    pub message: IncomingMessage,
    pub response: Option<serde_json::Value>,
}

const CHANNEL_PREFIX: &str = "gateway:channel:";
const SESSION_PREFIX: &str = "gateway:session:";
const INCOMING_PREFIX: &str = "gateway:incoming:";

pub struct GatewayService {
    store: Arc<dyn Store>,
    http: reqwest::Client,
}

impl GatewayService {
    #[must_use]
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self {
            store,
            http: reqwest::Client::new(),
        }
    }

    #[must_use]
    pub fn with_client(store: Arc<dyn Store>, http: reqwest::Client) -> Self {
        Self { store, http }
    }

    // ── Channel Management ──────────────────────────────────────

    /// Register a new channel.
    pub async fn register_channel(
        &self,
        channel_type: ChannelType,
        config: serde_json::Value,
    ) -> Channel {
        let channel = Channel {
            id: Uuid::now_v7().to_string(),
            channel_type,
            config,
            is_active: true,
        };
        let key = format!("{CHANNEL_PREFIX}{}", channel.id);
        self.store
            .set(&key, &channel)
            .await
            .expect("GatewayService::register_channel: store set failed");
        channel
    }

    /// Get a channel by ID.
    pub async fn get_channel(&self, channel_id: &str) -> Result<Option<Channel>, GatewayError> {
        let key = format!("{CHANNEL_PREFIX}{channel_id}");
        Ok(self.store.get::<Channel>(&key).await?)
    }

    /// List all channels, optionally filtered by type.
    pub async fn list_channels(
        &self,
        type_filter: Option<&ChannelType>,
    ) -> Result<Vec<Channel>, GatewayError> {
        let all: Vec<(String, Channel)> = self.store.scan_prefix(CHANNEL_PREFIX).await?;
        Ok(all
            .into_iter()
            .map(|(_, c)| c)
            .filter(|c| match type_filter {
                Some(t) => &c.channel_type == t,
                None => true,
            })
            .collect())
    }

    /// Activate a channel.
    ///
    /// # Errors
    /// Returns `GatewayError::ChannelNotFound` if the channel does not exist.
    pub async fn activate_channel(&self, channel_id: &str) -> Result<(), GatewayError> {
        let key = format!("{CHANNEL_PREFIX}{channel_id}");
        let mut channel = self
            .store
            .get::<Channel>(&key)
            .await?
            .ok_or_else(|| GatewayError::ChannelNotFound(channel_id.into()))?;
        channel.is_active = true;
        self.store.set(&key, &channel).await?;
        Ok(())
    }

    /// Deactivate a channel.
    ///
    /// # Errors
    /// Returns `GatewayError::ChannelNotFound` if the channel does not exist.
    pub async fn deactivate_channel(&self, channel_id: &str) -> Result<(), GatewayError> {
        let key = format!("{CHANNEL_PREFIX}{channel_id}");
        let mut channel = self
            .store
            .get::<Channel>(&key)
            .await?
            .ok_or_else(|| GatewayError::ChannelNotFound(channel_id.into()))?;
        channel.is_active = false;
        self.store.set(&key, &channel).await?;
        Ok(())
    }

    // ── Session Management ──────────────────────────────────────

    /// Create a new session.
    pub async fn create_session(
        &self,
        identity_id: IdentityId,
        channel_id: String,
        ttl_seconds: i64,
    ) -> Result<Session, GatewayError> {
        let channel_key = format!("{CHANNEL_PREFIX}{channel_id}");
        if !self.store.exists(&channel_key).await? {
            return Err(GatewayError::ChannelNotFound(channel_id));
        }

        let now = Utc::now();
        let session = Session {
            id: Uuid::now_v7().to_string(),
            identity_id,
            channel_id,
            state: SessionState::Active,
            created_at: now,
            expires_at: now + chrono::Duration::seconds(ttl_seconds),
        };
        let key = format!("{SESSION_PREFIX}{}", session.id);
        self.store
            .set(&key, &session)
            .await
            .expect("GatewayService::create_session: store set failed");
        Ok(session)
    }

    /// Get a session by ID.
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>, GatewayError> {
        let key = format!("{SESSION_PREFIX}{session_id}");
        Ok(self.store.get::<Session>(&key).await?)
    }

    /// Terminate a session.
    ///
    /// # Errors
    /// Returns `GatewayError::SessionNotFound` if the session does not exist.
    pub async fn terminate_session(&self, session_id: &str) -> Result<(), GatewayError> {
        let key = format!("{SESSION_PREFIX}{session_id}");
        let mut session = self
            .store
            .get::<Session>(&key)
            .await?
            .ok_or_else(|| GatewayError::SessionNotFound(session_id.into()))?;
        session.state = SessionState::Terminated;
        self.store.set(&key, &session).await?;
        Ok(())
    }

    /// Clean up expired sessions.
    pub async fn expire_stale_sessions(&self) -> Result<usize, GatewayError> {
        let now = Utc::now();
        let all: Vec<(String, Session)> = self.store.scan_prefix(SESSION_PREFIX).await?;
        let mut expired_count = 0;

        for (key, mut session) in all {
            if session.expires_at < now && session.state == SessionState::Active {
                session.state = SessionState::Expired;
                self.store.set(&key, &session).await?;
                expired_count += 1;
            }
        }

        Ok(expired_count)
    }

    /// List active sessions for an identity.
    pub async fn list_sessions(
        &self,
        identity_id: &IdentityId,
    ) -> Result<Vec<Session>, GatewayError> {
        let all: Vec<(String, Session)> = self.store.scan_prefix(SESSION_PREFIX).await?;
        Ok(all
            .into_iter()
            .map(|(_, s)| s)
            .filter(|s| s.identity_id == *identity_id)
            .collect())
    }

    // ── Message Dispatch ────────────────────────────────────────

    /// Dispatch a message through a channel.
    ///
    /// Supported transports:
    /// - `Webhook`: POST JSON payload to `config.url`
    /// - `Telegram`: call `sendMessage` using `config.bot_token` and `config.chat_id`
    /// - `Discord`: POST to `config.webhook_url`
    /// - `WhatsApp`: POST JSON to `config.bridge_url` (user-provided bridge, e.g. WhatsApp Business API or Baileys)
    /// - `InternalBus`: no-op / local echo
    ///
    /// # Errors
    /// Returns `GatewayError::ChannelNotFound` if the channel does not exist.
    /// Returns `GatewayError::ChannelInactive` if the channel is inactive.
    /// Returns `GatewayError::UnsupportedDispatch` for non-dispatchable channel types.
    pub async fn dispatch(
        &self,
        channel_id: &str,
        message: &Message,
    ) -> Result<DispatchResult, GatewayError> {
        let key = format!("{CHANNEL_PREFIX}{channel_id}");
        let channel = self
            .store
            .get::<Channel>(&key)
            .await?
            .ok_or_else(|| GatewayError::ChannelNotFound(channel_id.into()))?;

        if !channel.is_active {
            return Err(GatewayError::ChannelInactive(channel_id.into()));
        }

        let payload = serde_json::json!({
            "recipient": message.recipient,
            "subject": message.subject,
            "body": message.body,
            "metadata": message.metadata,
            "channel_type": format!("{:?}", channel.channel_type),
            "dispatched_at": Utc::now().to_rfc3339(),
        });

        let response = match channel.channel_type {
            ChannelType::Webhook | ChannelType::Slack | ChannelType::WhatsApp => {
                let url = channel
                    .config
                    .get("url")
                    .or_else(|| channel.config.get("bridge_url"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        GatewayError::Other(format!(
                            "channel {} missing url/bridge_url config",
                            channel_id
                        ))
                    })?;

                let resp = self.http
                    .post(url)
                    .json(&payload)
                    .timeout(std::time::Duration::from_secs(30))
                    .send()
                    .await
                    .map_err(|e| GatewayError::Other(format!("webhook request failed for channel {}: {e}", channel_id)))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    tracing::warn!("Webhook dispatch failed for channel {}: HTTP {} - {}", channel_id, status, body);
                    return Err(GatewayError::Other(format!(
                        "Webhook dispatch failed for channel {}: HTTP {} - {}",
                        channel_id, status, body
                    )));
                }

                let body = resp.text().await
                    .map_err(|e| GatewayError::Other(format!("webhook read body failed for channel {}: {e}", channel_id)))?;
                tracing::info!("Webhook dispatch succeeded for channel {}", channel_id);
                body
            }
            ChannelType::Telegram => {
                let bot_token = channel
                    .config
                    .get("bot_token")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        GatewayError::Other(format!("channel {} missing bot_token", channel_id))
                    })?;
                // Prefer the recipient override (e.g. the sender/chat id from an incoming message)
                // so replies go back to the correct user. Fallback to the configured chat_id.
                let chat_id: String = if !message.recipient.is_empty()
                    && message.recipient.parse::<i64>().is_ok()
                {
                    message.recipient.clone()
                } else {
                    channel
                        .config
                        .get("chat_id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            GatewayError::Other(format!("channel {} missing chat_id", channel_id))
                        })?
                        .into()
                };

                let text_html = convert_markdown_to_html(&message.body);
                let tg_payload = serde_json::json!({
                    "chat_id": chat_id,
                    "text": format!(
                        "{}{}",
                        message
                            .subject
                            .as_ref()
                            .map(|s| format!("<b>{}</b>\n\n", s))
                            .unwrap_or_default(),
                        text_html
                    ),
                    "parse_mode": "HTML",
                });

                let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
                let resp = self.http
                    .post(&url)
                    .json(&tg_payload)
                    .timeout(std::time::Duration::from_secs(30))
                    .send()
                    .await
                    .map_err(|e| GatewayError::Other(format!(
                        "Telegram dispatch failed for channel {} (chat_id={}): {e}",
                        channel_id, chat_id
                    )))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    tracing::warn!(
                        "Telegram dispatch failed for channel {} (chat_id={}): HTTP {} - {}",
                        channel_id, chat_id, status, body
                    );
                    return Err(GatewayError::Other(format!(
                        "Telegram dispatch failed for channel {} (chat_id={}): HTTP {} - {}",
                        channel_id, chat_id, status, body
                    )));
                }

                let body = resp.text().await
                    .map_err(|e| GatewayError::Other(format!(
                        "Telegram read body failed for channel {} (chat_id={}): {e}",
                        channel_id, chat_id
                    )))?;
                tracing::info!("Telegram dispatch succeeded for channel {} (chat_id={})", channel_id, chat_id);
                body
            }
            ChannelType::Discord => {
                let webhook_url = channel
                    .config
                    .get("webhook_url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        GatewayError::Other(format!("channel {} missing webhook_url", channel_id))
                    })?;

                let discord_payload = serde_json::json!({
                    "content": message.body,
                    "username": message.recipient,
                    "embeds": message.subject.as_ref().map(|s| {
                        vec![serde_json::json!({
                            "title": s,
                            "description": message.body,
                        })]
                    }),
                });

                let url_domain = webhook_url
                    .split("://")
                    .nth(1)
                    .unwrap_or("unknown")
                    .split('/')
                    .next()
                    .unwrap_or("unknown");

                let resp = self.http
                    .post(webhook_url)
                    .json(&discord_payload)
                    .timeout(std::time::Duration::from_secs(30))
                    .send()
                    .await
                    .map_err(|e| GatewayError::Other(format!(
                        "Discord dispatch failed for channel {} (webhook_domain={}): {e}",
                        channel_id, url_domain
                    )))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    tracing::warn!(
                        "Discord dispatch failed for channel {} (webhook_domain={}): HTTP {} - {}",
                        channel_id, url_domain, status, body
                    );
                    return Err(GatewayError::Other(format!(
                        "Discord dispatch failed for channel {} (webhook_domain={}): HTTP {} - {}",
                        channel_id, url_domain, status, body
                    )));
                }

                let body = resp.text().await
                    .map_err(|e| GatewayError::Other(format!(
                        "Discord read body failed for channel {} (webhook_domain={}): {e}",
                        channel_id, url_domain
                    )))?;
                tracing::info!("Discord dispatch succeeded for channel {} (webhook_domain={})", channel_id, url_domain);
                body
            }
            ChannelType::InternalBus => {
                tracing::debug!("internal bus dispatch: {payload}");
                "internal bus echo".into()
            }
            ChannelType::Rest => {
                let url = channel
                    .config
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        GatewayError::Other(format!(
                            "channel {} missing url config",
                            channel_id
                        ))
                    })?;

                let resp = self.http
                    .post(url)
                    .json(&payload)
                    .timeout(std::time::Duration::from_secs(30))
                    .send()
                    .await
                    .map_err(|e| GatewayError::Other(format!("rest request failed for channel {}: {e}", channel_id)))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    tracing::warn!("REST dispatch failed for channel {}: HTTP {} - {}", channel_id, status, body);
                    return Err(GatewayError::Other(format!(
                        "REST dispatch failed for channel {}: HTTP {} - {}",
                        channel_id, status, body
                    )));
                }

                let body = resp.text().await
                    .map_err(|e| GatewayError::Other(format!("rest read body failed for channel {}: {e}", channel_id)))?;
                tracing::info!("REST dispatch succeeded for channel {}", channel_id);
                body
            }
            ChannelType::Terminal => {
                tracing::info!("terminal dispatch: {payload}");
                "terminal echo".into()
            }
            ChannelType::Mobile => {
                return Err(GatewayError::UnsupportedDispatch(channel.channel_type));
            }
        };

        Ok(DispatchResult {
            channel_id: channel_id.into(),
            success: true,
            response: Some(response),
            dispatched_at: Utc::now(),
        })
    }

    // ── Incoming Webhooks ───────────────────────────────────────

    /// Process an incoming webhook payload from an external service.
    ///
    /// Supported sources:
    /// - `Telegram`: [`Update`](https://core.telegram.org/bots/api#update)
    /// - `Discord`: [`Interaction`](https://discord.com/developers/docs/interactions/receiving-and-responding)
    /// - `WhatsApp`: [Meta Cloud API Webhook](https://developers.facebook.com/docs/whatsapp/cloud-api/guides/set-up-webhook)
    /// - `Slack`: [Events API](https://api.slack.com/apis/connections/events-api)
    /// - `Webhook` / `Rest`: generic `{"sender": ..., "text": ...}`
    ///
    /// # Errors
    /// Returns `GatewayError::ChannelNotFound` if the channel does not exist.
    /// Returns `GatewayError::ChannelInactive` if the channel is inactive.
    /// Returns `GatewayError::UnsupportedDispatch` for non-webhook channel types.
    pub async fn receive(
        &self,
        channel_id: &str,
        body: &serde_json::Value,
        headers: &HashMap<String, String>,
    ) -> Result<IncomingWebhookResult, GatewayError> {
        let key = format!("{CHANNEL_PREFIX}{channel_id}");
        let channel = self
            .store
            .get::<Channel>(&key)
            .await?
            .ok_or_else(|| GatewayError::ChannelNotFound(channel_id.into()))?;

        if !channel.is_active {
            return Err(GatewayError::ChannelInactive(channel_id.into()));
        }

        let now = Utc::now();

        match channel.channel_type {
            ChannelType::Telegram => {
                Self::verify_telegram_token(&channel, headers)?;
                let (sender_id, text) = parse_telegram_update(body)?;
                let msg = self.store_incoming(channel_id, &sender_id, &text, body, now).await;
                Ok(IncomingWebhookResult {
                    message: msg,
                    response: None,
                })
            }
            ChannelType::Discord => {
                Self::verify_discord_signature(&channel, body, headers)?;
                let (sender_id, text) = parse_discord_interaction(body)?;
                let msg = self.store_incoming(channel_id, &sender_id, &text, body, now).await;
                Ok(IncomingWebhookResult {
                    message: msg,
                    response: None,
                })
            }
            ChannelType::WhatsApp => {
                let (sender_id, text) = parse_whatsapp_webhook(body)?;
                let msg = self.store_incoming(channel_id, &sender_id, &text, body, now).await;
                Ok(IncomingWebhookResult {
                    message: msg,
                    response: None,
                })
            }
            ChannelType::Slack => {
                // Slack URL verification challenge (tidak perlu signature check)
                if let Some(challenge) = body.get("challenge").and_then(|v| v.as_str()) {
                    return Ok(IncomingWebhookResult {
                        message: IncomingMessage {
                            id: Uuid::now_v7().to_string(),
                            channel_id: channel_id.into(),
                            sender_id: String::new(),
                            text: String::new(),
                            raw_payload: body.clone(),
                            received_at: now,
                        },
                        response: Some(serde_json::json!({"challenge": challenge})),
                    });
                }
                // Verifikasi HMAC-SHA256 Slack signature
                Self::verify_slack_signature(&channel, body, headers)?;
                let (sender_id, text) = parse_slack_event(body)?;
                let msg = self.store_incoming(channel_id, &sender_id, &text, body, now).await;
                Ok(IncomingWebhookResult {
                    message: msg,
                    response: None,
                })
            }
            ChannelType::Webhook | ChannelType::Rest => {
                let sender_id = body
                    .get("sender")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let text = body
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let msg = self
                    .store_incoming(channel_id, sender_id, text, body, now)
                    .await;
                Ok(IncomingWebhookResult {
                    message: msg,
                    response: None,
                })
            }
            ChannelType::Terminal | ChannelType::InternalBus | ChannelType::Mobile => {
                Err(GatewayError::UnsupportedDispatch(channel.channel_type))
            }
        }
    }

    async fn store_incoming(
        &self,
        channel_id: &str,
        sender_id: &str,
        text: &str,
        raw_payload: &serde_json::Value,
        received_at: DateTime<Utc>,
    ) -> IncomingMessage {
        let msg = IncomingMessage {
            id: Uuid::now_v7().to_string(),
            channel_id: channel_id.into(),
            sender_id: sender_id.into(),
            text: text.into(),
            raw_payload: raw_payload.clone(),
            received_at,
        };
        let store_key = format!("{INCOMING_PREFIX}{}", msg.id);
        // best-effort store: parse failures are logged but not fatal
        if let Err(e) = self.store.set(&store_key, &msg).await {
            tracing::warn!("failed to store incoming message: {e}");
        }
        msg
    }

    fn verify_telegram_token(
        channel: &Channel,
        headers: &HashMap<String, String>,
    ) -> Result<(), GatewayError> {
        if let Some(expected) = channel.config.get("secret_token").and_then(|v| v.as_str()) {
            let actual = headers
                .get("x-telegram-bot-api-secret-token")
                .map(|s| s.as_str())
                .unwrap_or("");
            if actual != expected {
                return Err(GatewayError::Other("invalid telegram secret token".into()));
            }
        }
        Ok(())
    }

    fn verify_discord_signature(
        channel: &Channel,
        body: &serde_json::Value,
        headers: &HashMap<String, String>,
    ) -> Result<(), GatewayError> {
        let public_key_hex = match channel.config.get("public_key").and_then(|v| v.as_str()) {
            Some(k) => k.to_string(),
            // Jika public_key tidak dikonfigurasi, skip verifikasi (dev mode)
            None => {
                tracing::debug!("Discord: public_key tidak dikonfigurasi, skip signature verify");
                return Ok(());
            }
        };

        let signature_hex = headers
            .get("x-signature-ed25519")
            .ok_or_else(|| GatewayError::Other("discord: missing X-Signature-Ed25519 header".into()))?;

        let timestamp = headers
            .get("x-signature-timestamp")
            .ok_or_else(|| GatewayError::Other("discord: missing X-Signature-Timestamp header".into()))?;

        // Decode public key dari hex
        let public_key_bytes = hex::decode(&public_key_hex)
            .map_err(|e| GatewayError::Other(format!("discord: invalid public_key hex: {e}")))?;
        let public_key_array: [u8; 32] = public_key_bytes
            .try_into()
            .map_err(|_| GatewayError::Other("discord: public_key harus 32 bytes".into()))?;
        let verifying_key = VerifyingKey::from_bytes(&public_key_array)
            .map_err(|e| GatewayError::Other(format!("discord: invalid Ed25519 public key: {e}")))?;

        // Decode signature
        let signature_bytes = hex::decode(signature_hex)
            .map_err(|e| GatewayError::Other(format!("discord: invalid signature hex: {e}")))?;
        let signature_array: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| GatewayError::Other("discord: signature harus 64 bytes".into()))?;
        let signature = Signature::from_bytes(&signature_array);

        // Pesan yang diverifikasi = timestamp + raw body JSON
        let body_str = serde_json::to_string(body)
            .map_err(|e| GatewayError::Other(format!("discord: gagal serialize body: {e}")))?;
        let message = format!("{timestamp}{body_str}");

        verifying_key
            .verify(message.as_bytes(), &signature)
            .map_err(|_| GatewayError::Other("discord: invalid Ed25519 signature".into()))?;

        Ok(())
    }

    /// Verifikasi Slack request signature menggunakan HMAC-SHA256.
    /// Slack mengirim `X-Slack-Signature` dan `X-Slack-Request-Timestamp`.
    fn verify_slack_signature(
        channel: &Channel,
        body: &serde_json::Value,
        headers: &HashMap<String, String>,
    ) -> Result<(), GatewayError> {
        let signing_secret = match channel.config.get("signing_secret").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => {
                tracing::debug!("Slack: signing_secret tidak dikonfigurasi, skip signature verify");
                return Ok(());
            }
        };

        let timestamp = headers
            .get("x-slack-request-timestamp")
            .ok_or_else(|| GatewayError::Other("slack: missing X-Slack-Request-Timestamp".into()))?;

        let slack_signature = headers
            .get("x-slack-signature")
            .ok_or_else(|| GatewayError::Other("slack: missing X-Slack-Signature".into()))?;

        // Signature harus format "v0=<hex>"
        let expected_hash = slack_signature
            .strip_prefix("v0=")
            .ok_or_else(|| GatewayError::Other("slack: X-Slack-Signature format invalid".into()))?;

        // Compute HMAC-SHA256: v0:{timestamp}:{body}
        let body_str = serde_json::to_string(body)
            .map_err(|e| GatewayError::Other(format!("slack: gagal serialize body: {e}")))?;
        let base_string = format!("v0:{timestamp}:{body_str}");

        let mut mac = Hmac::<Sha256>::new_from_slice(signing_secret.as_bytes())
            .map_err(|_| GatewayError::Other("slack: invalid signing_secret".into()))?;
        mac.update(base_string.as_bytes());
        let computed = hex::encode(mac.finalize().into_bytes());

        if computed != expected_hash {
            return Err(GatewayError::Other("slack: invalid HMAC-SHA256 signature".into()));
        }

        Ok(())
    }
}

// ── Webhook Payload Parsers ─────────────────────────────────

fn parse_telegram_update(body: &serde_json::Value) -> Result<(String, String), GatewayError> {
    if body.get("message").is_none() {
        return Err(GatewayError::Other("telegram: missing 'message' field in update payload".into()));
    }

    let sender_id = body
        .pointer("/message/from/id")
        .and_then(|v| v.as_i64())
        .map(|n| n.to_string())
        .or_else(|| {
            body.pointer("/message/chat/id")
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string())
        })
        .ok_or_else(|| GatewayError::Other("telegram: missing message.from.id".into()))?;

    let text = body
        .pointer("/message/text")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    Ok((sender_id, text.into()))
}

fn parse_discord_interaction(body: &serde_json::Value) -> Result<(String, String), GatewayError> {
    if body.get("type").is_none() {
        return Err(GatewayError::Other("discord: missing 'type' field in interaction payload".into()));
    }

    // Discord sends a PING (type 1) on webhook setup
    if body.get("type").and_then(|v| v.as_i64()) == Some(1) {
        return Ok(("discord".into(), "ping".into()));
    }

    let sender_id = body
        .pointer("/member/user/id")
        .and_then(|v| v.as_str())
        .or_else(|| body.get("user").and_then(|u| u.get("id")).and_then(|v| v.as_str()))
        .ok_or_else(|| GatewayError::Other("discord: missing member.user.id".into()))?;

    let text = body
        .pointer("/data/name")
        .and_then(|v| v.as_str())
        .map(|s| format!("/{s}"))
        .or_else(|| {
            body.pointer("/data/options")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|opt| opt.get("value"))
                .and_then(|v| v.as_str())
                .map(|s| s.into())
        })
        .unwrap_or_default();

    Ok((sender_id.into(), text))
}

fn parse_whatsapp_webhook(body: &serde_json::Value) -> Result<(String, String), GatewayError> {
    if body.get("entry").is_none() {
        return Err(GatewayError::Other("whatsapp: missing 'entry' field in webhook payload".into()));
    }

    // WhatsApp verification challenge is handled at the HTTP handler level
    let sender_id = body
        .pointer("/entry/0/changes/0/value/messages/0/from")
        .and_then(|v| v.as_str())
        .ok_or_else(|| GatewayError::Other("whatsapp: missing messages[0].from".into()))?;

    let text = body
        .pointer("/entry/0/changes/0/value/messages/0/text/body")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    Ok((sender_id.into(), text.into()))
}

fn parse_slack_event(body: &serde_json::Value) -> Result<(String, String), GatewayError> {
    if body.get("event").is_none() {
        return Err(GatewayError::Other("slack: missing 'event' field in event payload".into()));
    }

    // Slack URL verification challenge is handled in `receive`
    let sender_id = body
        .pointer("/event/user")
        .and_then(|v| v.as_str())
        .ok_or_else(|| GatewayError::Other("slack: missing event.user".into()))?;

    let text = body
        .pointer("/event/text")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    Ok((sender_id.into(), text.into()))
}



/// Helper untuk mengonversi Markdown sederhana (**bold**, *italic*) ke format HTML Telegram.
fn convert_markdown_to_html(text: &str) -> String {
    let escaped = text
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;");

    let mut html = String::new();
    let mut in_bold = false;
    let mut in_italic = false;
    let mut chars = escaped.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '*' {
            if chars.peek() == Some(&'*') {
                chars.next(); // konsumsi '*' kedua
                if in_bold {
                    html.push_str("</b>");
                    in_bold = false;
                } else {
                    html.push_str("<b>");
                    in_bold = true;
                }
            } else {
                if in_italic {
                    html.push_str("</i>");
                    in_italic = false;
                } else {
                    html.push_str("<i>");
                    in_italic = true;
                }
            }
        } else {
            html.push(ch);
        }
    }

    if in_bold {
        html.push_str("</b>");
    }
    if in_italic {
        html.push_str("</i>");
    }

    html
}
