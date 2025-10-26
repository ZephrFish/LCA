use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tracing::{debug, error};

use super::types::{ChatRequest, ChatResponse, LmStudioResponse, Message};

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<String>;
    async fn chat_with_history(&self, messages: Vec<Message>, model: &str) -> Result<String>;
}

pub struct OllamaClient {
    client: Client,
    base_url: String,
}

impl OllamaClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
        }
    }

    pub fn default() -> Self {
        Self::new("http://localhost:11434")
    }
}

#[async_trait]
impl LlmClient for OllamaClient {
    async fn chat(&self, request: ChatRequest) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);

        debug!("Sending chat request to Ollama: {:?}", request.model);

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("Ollama API error {}: {}", status, error_text);
            anyhow::bail!("Ollama API error: {} - {}", status, error_text);
        }

        let chat_response: ChatResponse = response.json().await?;
        Ok(chat_response.message.content)
    }

    async fn chat_with_history(&self, messages: Vec<Message>, model: &str) -> Result<String> {
        let request = ChatRequest::new(model, messages);
        self.chat(request).await
    }
}

pub struct LmStudioClient {
    client: Client,
    base_url: String,
}

impl LmStudioClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
        }
    }

    pub fn default() -> Self {
        Self::new("http://localhost:1234/v1")
    }
}

#[async_trait]
impl LlmClient for LmStudioClient {
    async fn chat(&self, request: ChatRequest) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);

        debug!("Sending chat request to LM Studio: {:?}", request.model);

        // Convert system messages to user messages for compatibility
        // Many LM Studio models only support user/assistant roles
        let messages: Vec<_> = request
            .messages
            .into_iter()
            .map(|mut msg| {
                if msg.role == super::types::Role::System {
                    msg.role = super::types::Role::User;
                    msg.content = format!("System Instructions: {}", msg.content);
                }
                msg
            })
            .collect();

        let body = json!({
            "model": request.model,
            "messages": messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(2000),
        });

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("LM Studio API error {}: {}", status, error_text);
            anyhow::bail!("LM Studio API error: {} - {}", status, error_text);
        }

        let lm_response: LmStudioResponse = response.json().await?;

        let content = lm_response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("No response from LM Studio"))?;

        debug!("LM Studio response: {}", content);
        Ok(content)
    }

    async fn chat_with_history(&self, messages: Vec<Message>, model: &str) -> Result<String> {
        let request = ChatRequest::new(model, messages);
        self.chat(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let ollama = OllamaClient::default();
        assert_eq!(ollama.base_url, "http://localhost:11434");

        let lm_studio = LmStudioClient::default();
        assert_eq!(lm_studio.base_url, "http://localhost:1234/v1");
    }
}
