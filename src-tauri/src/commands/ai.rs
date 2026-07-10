use crate::ai::client::AiClient;
use crate::models::{AiConfig, AiReviewRequest, AiReviewResult};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn ai_get_config(state: State<'_, AppState>) -> Result<AiConfig, String> {
    let mut config = state.ai_config.get_config().map_err(|e| e.to_string())?;
    // Never expose encrypted key to frontend
    config.api_key_encrypted = None;
    Ok(config)
}

#[tauri::command]
pub async fn ai_save_config(state: State<'_, AppState>, config: AiConfig) -> Result<(), String> {
    // Merge: preserve encrypted key from existing config
    let existing = state.ai_config.get_config().unwrap_or_default();
    let mut merged = config;
    if merged.api_key_encrypted.is_none() {
        let encrypted_key = existing.api_key_encrypted.clone();
        merged.api_key_encrypted = encrypted_key.clone();
        merged.api_key_configured = encrypted_key.is_some();
    }
    state
        .ai_config
        .save_config(&merged)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_save_api_key(state: State<'_, AppState>, api_key: String) -> Result<(), String> {
    state
        .ai_config
        .save_api_key(&api_key)
        .map_err(|e| e.to_string())
        .map(|_| ())
}

#[tauri::command]
pub async fn ai_review(
    state: State<'_, AppState>,
    request: AiReviewRequest,
) -> Result<AiReviewResult, String> {
    let config = state.ai_config.get_config().map_err(|e| e.to_string())?;
    let api_key = state.ai_config.get_api_key().map_err(|e| e.to_string())?;

    let client = AiClient::new(config.endpoint, config.model, api_key);

    client
        .review(
            &request.diff,
            request.context.as_ref(),
            request.focus.as_ref(),
            config.system_prompt.as_deref(),
            config.temperature.unwrap_or(0.3),
            config.max_tokens.unwrap_or(4096),
        )
        .await
        .map_err(|e| e.to_string())
}

/// Streaming AI review — spawns a background task that emits events
#[tauri::command]
pub async fn ai_review_stream(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    request: AiReviewRequest,
) -> Result<(), String> {
    let config = state.ai_config.get_config().map_err(|e| e.to_string())?;
    let api_key = state.ai_config.get_api_key().map_err(|e| e.to_string())?;
    let system_prompt = config.system_prompt.clone();
    let temperature = config.temperature.unwrap_or(0.3);
    let max_tokens = config.max_tokens.unwrap_or(4096);

    // Spawn a background task for streaming
    tokio::spawn(async move {
        let client = AiClient::new(config.endpoint, config.model, api_key);

        let result = client
            .review_stream(
                &request.diff,
                request.context.as_ref(),
                request.focus.as_ref(),
                system_prompt.as_deref(),
                temperature,
                max_tokens,
                |token| {
                    // Emit each token chunk to the frontend
                    let _ = app_handle.emit("ai-review-chunk", token);
                },
            )
            .await;

        match result {
            Ok(review_result) => {
                let _ = app_handle.emit("ai-review-done", review_result);
            }
            Err(e) => {
                let _ = app_handle.emit("ai-review-error", e.to_string());
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn ai_list_models(
    state: State<'_, AppState>,
    endpoint: String,
) -> Result<Vec<String>, String> {
    let api_key = state.ai_config.get_api_key().map_err(|e| e.to_string())?;

    // Use a dummy model name — list_models doesn't need a model
    let client = AiClient::new(endpoint, "".to_string(), api_key);

    client.list_models().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_test_connection(state: State<'_, AppState>) -> Result<bool, String> {
    let config = state.ai_config.get_config().map_err(|e| e.to_string())?;
    let api_key = state.ai_config.get_api_key().map_err(|e| e.to_string())?;

    let client = AiClient::new(config.endpoint, config.model, api_key);

    client.test_connection().await.map_err(|e| e.to_string())
}
