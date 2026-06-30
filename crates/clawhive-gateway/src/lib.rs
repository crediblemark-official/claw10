#![allow(clippy::pedantic)]

use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use clawhive_domain::{Channel, ChannelType, IdentityId, Session, SessionState};
use clawhive_store::{Store, StoreError, StoreExt};

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

const CHANNEL_PREFIX: &str = "gateway:channel:";
const SESSION_PREFIX: &str = "gateway:session:";

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

                self.http
                    .post(url)
                    .json(&payload)
                    .send()
                    .await
                    .map_err(|e| GatewayError::Other(format!("webhook request failed: {e}")))?
                    .text()
                    .await
                    .map_err(|e| GatewayError::Other(format!("webhook read body failed: {e}")))?
            }
            ChannelType::Telegram => {
                let bot_token = channel
                    .config
                    .get("bot_token")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        GatewayError::Other(format!("channel {} missing bot_token", channel_id))
                    })?;
                let chat_id = channel
                    .config
                    .get("chat_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        GatewayError::Other(format!("channel {} missing chat_id", channel_id))
                    })?;

                let tg_payload = serde_json::json!({
                    "chat_id": chat_id,
                    "text": format!(
                        "{}{}",
                        message
                            .subject
                            .as_ref()
                            .map(|s| format!("*{s}*\n\n"))
                            .unwrap_or_default(),
                        message.body
                    ),
                    "parse_mode": "MarkdownV2",
                });

                let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
                self.http
                    .post(&url)
                    .json(&tg_payload)
                    .send()
                    .await
                    .map_err(|e| GatewayError::Other(format!("telegram request failed: {e}")))?
                    .text()
                    .await
                    .map_err(|e| GatewayError::Other(format!("telegram read body failed: {e}")))?
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

                self.http
                    .post(webhook_url)
                    .json(&discord_payload)
                    .send()
                    .await
                    .map_err(|e| GatewayError::Other(format!("discord request failed: {e}")))?
                    .text()
                    .await
                    .map_err(|e| GatewayError::Other(format!("discord read body failed: {e}")))?
            }
            ChannelType::InternalBus => {
                tracing::debug!("internal bus dispatch: {payload}");
                "internal bus echo".into()
            }
            ChannelType::Mobile | ChannelType::Rest | ChannelType::Terminal => {
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
}
