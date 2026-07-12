use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
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

async fn consume_sse_stream<S, F>(stream: S, mut on_token: F) -> Result<String, AppError>
where
    S: Stream<Item = Result<Vec<u8>, String>>,
    F: FnMut(&str) -> Result<(), AppError>,
{
    // Appending an empty event terminator makes providers that omit the final blank line flush safely.
    let stream = stream.chain(futures::stream::once(async { Ok::<Vec<u8>, String>(b"\n\n".to_vec()) }));
    let events = stream.eventsource();
    futures::pin_mut!(events);
    let mut accumulated = String::new();

    while let Some(event) = events.next().await {
        let event = event.map_err(|error| AppError::Ai(format!("SSE 解析失败: {error}")))?;
        let data = event.data.trim();
        if data.is_empty() {
            continue;
        }
        if data == "[DONE]" {
            break;
        }
        let json: Value = serde_json::from_str(data)
            .map_err(|error| AppError::Ai(format!("AI SSE 数据不是有效 JSON: {error}; data={data}")))?;
        if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
            accumulated.push_str(content);
            on_token(content)?;
        }
    }
    Ok(accumulated)
}

impl AiClient {
    pub fn new(endpoint: String, model: String, api_key: String) -> Self {
        Self { endpoint: endpoint.trim_end_matches('/').to_string(), model, api_key, client: reqwest::Client::new() }
    }

    /// Send a chat completion request (non-streaming)
    async fn chat(&self, messages: &[Value], temperature: f32, max_tokens: u32) -> Result<String, AppError> {
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
            return Err(AppError::Ai(format!("AI API error ({}): {}", status, error_body)));
        }

        let json: Value = resp.json().await?;
        let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();

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
        on_token: F,
    ) -> Result<String, AppError>
    where
        F: FnMut(&str) -> Result<(), AppError> + Send,
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
            return Err(AppError::Ai(format!("AI API error ({}): {}", status, error_body)));
        }

        let stream =
            resp.bytes_stream().map(|chunk| chunk.map(|bytes| bytes.to_vec()).map_err(|error| error.to_string()));
        consume_sse_stream(stream, on_token).await
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
    #[allow(clippy::too_many_arguments)]
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
        F: FnMut(&str) -> Result<(), AppError> + Send,
    {
        let system_prompt = prompt::build_system_prompt(focus, custom_prompt);
        let user_message = prompt::build_user_message(diff, context);

        let messages = vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": user_message}),
        ];

        let response = self.chat_stream(&messages, temperature, max_tokens, on_token).await?;

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
        let result: AiReviewResult = serde_json::from_str(trimmed)
            .map_err(|e| AppError::Ai(format!("Failed to parse AI response: {}\n\nRaw response: {}", e, response)))?;

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
            return Err(AppError::Ai(format!("Failed to list models ({}): {}", status, error_body)));
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
        let messages = vec![serde_json::json!({"role": "user", "content": "Hello, respond with just 'ok'."})];

        match self.chat(&messages, 0.0, 50).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::stream;

    use super::consume_sse_stream;

    fn delta(content: &str) -> String {
        format!(r#"{{"choices":[{{"delta":{{"content":"{content}"}}}}]}}"#)
    }

    #[tokio::test]
    async fn parses_lf_crlf_chunks_multiline_and_done() {
        let first = delta("你");
        let second = delta("好");
        let body = format!(
            ": keepalive\r\ndata: {first}\r\n\r\ndata: {}\ndata: {}\n\ndata: [DONE]\n\n",
            &second[..second.len() / 2],
            &second[second.len() / 2..]
        );
        let chunks = body.as_bytes().chunks(7).map(|chunk| Ok::<_, String>(chunk.to_vec())).collect::<Vec<_>>();
        let mut received = String::new();
        let result = consume_sse_stream(stream::iter(chunks), |token| {
            received.push_str(token);
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(result, "你好");
        assert_eq!(received, "你好");
    }

    #[tokio::test]
    async fn flushes_final_event_without_blank_line() {
        let body = format!("data: {}", delta("尾"));
        let result = consume_sse_stream(stream::iter(vec![Ok(body.into_bytes())]), |_| Ok(())).await.unwrap();
        assert_eq!(result, "尾");
    }

    #[tokio::test]
    async fn rejects_invalid_nonempty_json() {
        let error =
            consume_sse_stream(stream::iter(vec![Ok(b"data: not-json\n\n".to_vec())]), |_| Ok(())).await.unwrap_err();
        assert!(error.to_string().contains("not-json"));
    }
}
