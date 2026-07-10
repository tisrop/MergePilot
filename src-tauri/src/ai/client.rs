use futures::StreamExt;
use serde_json::Value;

use crate::ai::prompt;
use crate::error::AppError;
use crate::models::{AiReviewFocus, AiReviewResult, PrContext};

/// OpenAI-compatible chat client
pub struct AiClient {
    endpoint: String,
    model: String,
    api_key: String,
    client: reqwest::Client,
}

impl AiClient {
    pub fn new(endpoint: String, model: String, api_key: String) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            model,
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Send a chat completion request (non-streaming)
    async fn chat(
        &self,
        messages: &[Value],
        temperature: f32,
        max_tokens: u32,
    ) -> Result<String, AppError> {
        let url = format!("{}/chat/completions", self.endpoint);
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("User-Agent", "mergepilot")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            return Err(AppError::Ai(format!(
                "AI API error ({}): {}",
                status, error_body
            )));
        }

        let json: Value = resp.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }

    /// Send a streaming chat completion request.
    /// Calls `on_token` with each text delta as it arrives.
    /// Returns the complete accumulated content.
    async fn chat_stream<F>(
        &self,
        messages: &[Value],
        temperature: f32,
        max_tokens: u32,
        mut on_token: F,
    ) -> Result<String, AppError>
    where
        F: FnMut(&str) + Send,
    {
        let url = format!("{}/chat/completions", self.endpoint);
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
            "stream": true,
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("User-Agent", "mergepilot")
            .header("Accept", "text/event-stream")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            return Err(AppError::Ai(format!(
                "AI API error ({}): {}",
                status, error_body
            )));
        }

        let mut accumulated = String::new();
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            // Parse SSE lines: "data: {...}\n\n"
            while let Some(pos) = buffer.find("\n\n") {
                let event_block = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                for line in event_block.lines() {
                    let line = line.trim();
                    if line.is_empty() || !line.starts_with("data: ") {
                        continue;
                    }
                    let data = &line[6..]; // strip "data: "

                    if data == "[DONE]" {
                        break;
                    }

                    if let Ok(json) = serde_json::from_str::<Value>(data) {
                        if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                            accumulated.push_str(content);
                            on_token(content);
                        }
                    }
                }
            }
        }

        Ok(accumulated)
    }

    /// Perform a code review using the AI model (non-streaming)
    pub async fn review(
        &self,
        diff: &str,
        context: Option<&PrContext>,
        focus: Option<&AiReviewFocus>,
        custom_prompt: Option<&str>,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<AiReviewResult, AppError> {
        let system_prompt = prompt::build_system_prompt(focus, custom_prompt);
        let user_message = prompt::build_user_message(diff, context);

        let messages = vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": user_message}),
        ];

        let response = self.chat(&messages, temperature, max_tokens).await?;
        self.parse_review_response(&response)
    }

    /// Perform a streaming code review.
    /// Calls `on_token` with each text delta, and returns the final parsed result.
    pub async fn review_stream<F>(
        &self,
        diff: &str,
        context: Option<&PrContext>,
        focus: Option<&AiReviewFocus>,
        custom_prompt: Option<&str>,
        temperature: f32,
        max_tokens: u32,
        on_token: F,
    ) -> Result<AiReviewResult, AppError>
    where
        F: FnMut(&str) + Send,
    {
        let system_prompt = prompt::build_system_prompt(focus, custom_prompt);
        let user_message = prompt::build_user_message(diff, context);

        let messages = vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": user_message}),
        ];

        let response = self
            .chat_stream(&messages, temperature, max_tokens, on_token)
            .await?;

        self.parse_review_response(&response)
    }

    /// Parse the AI model's JSON response into AiReviewResult
    fn parse_review_response(&self, response: &str) -> Result<AiReviewResult, AppError> {
        let json_str = if let Some(start) = response.find("```json") {
            let after_start = &response[start + 7..];
            if let Some(end) = after_start.find("```") {
                &after_start[..end]
            } else {
                after_start
            }
        } else if let Some(start) = response.find('{') {
            &response[start..]
        } else {
            response
        };

        let trimmed = json_str.trim();
        let result: AiReviewResult = serde_json::from_str(trimmed).map_err(|e| {
            AppError::Ai(format!(
                "Failed to parse AI response: {}\n\nRaw response: {}",
                e, response
            ))
        })?;

        Ok(result)
    }

    /// List available models from the API endpoint.
    /// Calls GET /v1/models (OpenAI-compatible).
    pub async fn list_models(&self) -> Result<Vec<String>, AppError> {
        let url = format!("{}/models", self.endpoint);

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("User-Agent", "mergepilot")
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            return Err(AppError::Ai(format!(
                "Failed to list models ({}): {}",
                status, error_body
            )));
        }

        let json: Value = resp.json().await?;

        // OpenAI format: { "object": "list", "data": [{ "id": "...", ... }] }
        let models: Vec<String> = json["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .filter(|id| {
                        !id.contains("dall-e")
                            && !id.contains("whisper")
                            && !id.contains("tts")
                            && !id.contains("embedding")
                            && !id.contains("moderation")
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }

    /// Test the API connection with a simple request
    pub async fn test_connection(&self) -> Result<bool, AppError> {
        let messages =
            vec![serde_json::json!({"role": "user", "content": "Hello, respond with just 'ok'."})];

        match self.chat(&messages, 0.0, 50).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}
