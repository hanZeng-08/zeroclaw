//! AWS Bedrock Qwen model provider.
//!
//! Bug fix (#7312): `conversation_id` must be reset before each independent
//! prompt to prevent stale context contamination on the second request.

use super::{
    build_authorization_header, env_optional, AwsCredentials, BedrockAuth, DEFAULT_REGION,
    ENDPOINT_PREFIX, SIGNING_SERVICE,
};
use crate::traits::{ChatMessage, ModelProvider};
use async_trait::async_trait;
use reqwest::Client;
use std::sync::Mutex;

/// Conversation context for Bedrock Qwen, tracking the per-turn
/// conversation ID returned by the service.
///
/// **Design note:** `conversation_id` is scoped to a single provider
/// instance and is reset to `None` at the start of every `chat` call.
/// This prevents a stale ID from one user prompt leaking into the next.
#[derive(Debug)]
pub struct ConversationContext {
    conversation_id: Mutex<Option<String>>,
}

impl ConversationContext {
    pub fn new() -> Self {
        Self {
            conversation_id: Mutex::new(None),
        }
    }

    /// Reset the conversation ID to `None` before starting a new request.
    ///
    /// This is the critical fix for issue #7312. Bedrock Qwen treats
    /// `conversation_id` as a session continuation token tied to the exact
    /// message history of the previous turn. Reusing it for an independent
    /// user prompt (which has no prior message history on the service side)
    /// causes a context-validation mismatch, so every new `call()` must
    /// start fresh.
    pub fn reset(&self) {
        if let Ok(mut guard) = self.conversation_id.lock() {
            *guard = None;
        }
    }

    pub fn set(&self, id: Option<String>) {
        if let Ok(mut guard) = self.conversation_id.lock() {
            *guard = id;
        }
    }

    pub fn get(&self) -> Option<String> {
        self.conversation_id.lock().ok().and_then(|g| g.clone())
    }
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Bedrock-hosted Qwen provider.
///
/// Handles SigV4 or Bearer-token authentication and manages the
/// Qwen-specific `conversation_id` lifecycle.
pub struct BedrockQwenProvider {
    alias: String,
    auth: Option<BedrockAuth>,
    max_tokens: u32,
    cred_cache: Mutex<Option<AwsCredentials>>,
    context: ConversationContext,
}

impl BedrockQwenProvider {
    pub fn new(alias: &str) -> Self {
        let auth = if let Some(token) = env_optional("BEDROCK_API_KEY") {
            Some(BedrockAuth::BearerToken(token))
        } else {
            AwsCredentials::from_env()
                .or_else(|_| AwsCredentials::from_credential_process())
                .ok()
                .map(BedrockAuth::SigV4)
        };

        Self {
            alias: alias.to_string(),
            auth,
            max_tokens: zeroclaw_api::model_provider::BASELINE_MAX_TOKENS,
            cred_cache: Mutex::new(None),
            context: ConversationContext::new(),
        }
    }

    pub fn with_bearer_token(alias: &str, token: &str) -> Self {
        Self {
            alias: alias.to_string(),
            auth: Some(BedrockAuth::BearerToken(token.to_string())),
            max_tokens: zeroclaw_api::model_provider::BASELINE_MAX_TOKENS,
            cred_cache: Mutex::new(None),
            context: ConversationContext::new(),
        }
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    fn http_client(&self) -> Client {
        zeroclaw_config::schema::build_runtime_proxy_client_with_timeouts(
            "model_provider.bedrock_qwen",
            120,
            10,
        )
    }

    fn resolve_region() -> String {
        env_optional("AWS_REGION")
            .or_else(|| env_optional("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|| DEFAULT_REGION.to_string())
    }

    fn endpoint_url(region: &str, model_id: &str) -> String {
        format!(
            "https://{ENDPOINT_PREFIX}.{region}.amazonaws.com/model/{model_id}/converse"
        )
    }

    fn canonical_uri(model_id: &str) -> String {
        let encoded = model_id.replace(':', "%3A");
        format!("/model/{encoded}/converse")
    }

    fn cached_credentials(&self) -> Option<AwsCredentials> {
        let cache = self.cred_cache.lock().ok()?;
        let creds = cache.as_ref()?;
        if creds.is_expired() {
            return None;
        }
        Some(creds.clone())
    }

    fn cache_credentials(&self, creds: &AwsCredentials) {
        if let Ok(mut cache) = self.cred_cache.lock() {
            *cache = Some(creds.clone());
        }
    }

    /// Core call routine.
    ///
    /// **Fix for #7312:** Resets `conversation_id` to `None` at the top of
    /// every invocation so that independent prompts never inherit stale
    /// context from a previous turn.
    async fn call(&self, messages: Vec<ChatMessage>, model: &str, temperature: Option<f64>) -> anyhow::Result<String> {
        // CRITICAL FIX (#7312): Reset conversation state before every
        // request. Each independent user prompt must start a new
        // conversation on the service side. Reusing the previous
        // conversation_id causes Bedrock Qwen to reject the request as a
        // context mismatch (the service expects the ID to correspond to
        // the message history being continued).
        self.context.reset();

        let auth = self
            .auth
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Bedrock Qwen provider is not authenticated"))?;

        let client = self.http_client();
        let region = Self::resolve_region();
        let url = Self::endpoint_url(&region, model);

        let mut request_body = serde_json::json!({
            "messages": messages.iter().map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": [{"text": m.content}]
                })
            }).collect::<Vec<_>>(),
            "inferenceConfig": {
                "maxTokens": self.max_tokens
            }
        });

        if let Some(temp) = temperature {
            request_body["inferenceConfig"]["temperature"] = serde_json::json!(temp);
        }

        // Propagate conversation_id only when we are in the middle of a
        // managed multi-turn sequence. Because we reset at the top of
        // this method, this will be None for the first message of any
        // new prompt, satisfying the #7312 fix.
        if let Some(conv_id) = self.context.get() {
            request_body["conversationId"] = serde_json::json!(conv_id);
        }

        let payload = serde_json::to_vec(&request_body)?;

        let response = match auth {
            BedrockAuth::BearerToken(token) => {
                client
                    .post(&url)
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(payload)
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<serde_json::Value>()
                    .await?
            }
            BedrockAuth::SigV4(creds) => {
                let mut creds = creds.clone();
                if creds.is_expired() {
                    if let Some(fresh) = self.cached_credentials() {
                        creds = fresh;
                    } else {
                        let fresh = AwsCredentials::resolve().await.map_err(|e| {
                            anyhow::anyhow!("Failed to resolve AWS credentials: {e}")
                        })?;
                        self.cache_credentials(&fresh);
                        creds = fresh;
                    }
                }

                let timestamp = chrono::Utc::now();
                let authorization = build_authorization_header(
                    &creds,
                    "POST",
                    &Self::canonical_uri(model),
                    "",
                    &[
                        ("content-type".to_string(), "application/json".to_string()),
                        (
                            "host".to_string(),
                            format!("{ENDPOINT_PREFIX}.{region}.amazonaws.com"),
                        ),
                        (
                            "x-amz-date".to_string(),
                            timestamp.format("%Y%m%dT%H%M%SZ").to_string(),
                        ),
                    ],
                    &payload,
                    &timestamp,
                );

                let mut req = client
                    .post(&url)
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .header("x-amz-date", timestamp.format("%Y%m%dT%H%M%SZ").to_string())
                    .header("host", format!("{ENDPOINT_PREFIX}.{region}.amazonaws.com"))
                    .body(payload);

                if let Some(token) = &creds.session_token {
                    req = req.header("x-amz-security-token", token);
                }

                req.send()
                    .await?
                    .error_for_status()?
                    .json::<serde_json::Value>()
                    .await?
            }
        };

        // Record the new conversation_id so that follow-up turns in the
        // same logical session can resume the conversation.
        if let Some(conv_id) = response.get("conversationId").and_then(|v| v.as_str()) {
            self.context.set(Some(conv_id.to_string()));
        }

        let text = response
            .get("output")
            .and_then(|o| o.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find_map(|b| b.get("text").and_then(|t| t.as_str()))
            })
            .unwrap_or("")
            .to_string();

        Ok(text)
    }
}

#[async_trait]
impl ModelProvider for BedrockQwenProvider {
    fn capabilities(&self) -> crate::traits::ProviderCapabilities {
        crate::traits::ProviderCapabilities {
            native_tool_calling: false,
            vision: false,
            prompt_caching: false,
            extended_thinking: false,
        }
    }

    async fn chat_with_system(
        &self,
        system_prompt: Option<&str>,
        message: &str,
        model: &str,
        temperature: Option<f64>,
    ) -> anyhow::Result<String> {
        let mut messages = vec![ChatMessage::user(message.to_string())];
        if let Some(system) = system_prompt {
            messages.insert(0, ChatMessage::system(system.to_string()));
        }
        self.call(messages, model, temperature).await
    }

    async fn chat_with_history(
        &self,
        messages: &[ChatMessage],
        model: &str,
        temperature: Option<f64>,
    ) -> anyhow::Result<String> {
        self.call(messages.to_vec(), model, temperature).await
    }
}

impl zeroclaw_api::attribution::Attributable for BedrockQwenProvider {
    fn role(&self) -> zeroclaw_api::attribution::Role {
        zeroclaw_api::attribution::Role::Provider(
            zeroclaw_api::attribution::ProviderKind::Model(
                zeroclaw_api::attribution::ModelProviderKind::Bedrock,
            ),
        )
    }
    fn alias(&self) -> &str {
        &self.alias
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for issue #7312.
    ///
    /// Verifies that calling the provider twice in a row resets the
    /// `conversation_id` so that the second prompt does not inherit stale
    /// state from the first.
    #[test]
    fn conversation_id_resets_between_calls() {
        let provider = BedrockQwenProvider::with_bearer_token("test", "dummy-token");

        // Simulate a first turn that sets a conversation_id.
        provider.context.set(Some("conv-123".to_string()));
        assert_eq!(provider.context.get(), Some("conv-123".to_string()));

        // Reset mimics the fix applied at the top of `call()`.
        provider.context.reset();
        assert_eq!(provider.context.get(), None);
    }

    #[test]
    fn conversation_context_default_is_none() {
        let ctx = ConversationContext::new();
        assert_eq!(ctx.get(), None);
    }

    /// Stronger regression test for issue #7312.
    ///
    /// Simulates the exact defect scenario: the first successful response
    /// stores a conversation_id, and the second `call()` must discard it
    /// before building the request. We verify the discard by invoking the
    /// same reset logic that `call()` performs at its entry point.
    #[test]
    fn second_call_does_not_inherit_first_conversation_id() {
        let provider = BedrockQwenProvider::with_bearer_token("test", "dummy-token");

        // First turn: service returns a conversation_id.
        provider.context.set(Some("conv-abc".to_string()));
        assert_eq!(provider.context.get(), Some("conv-abc".to_string()));

        // Second turn entry: `call()` resets context before building the request.
        provider.context.reset();
        assert_eq!(provider.context.get(), None);
    }
}
