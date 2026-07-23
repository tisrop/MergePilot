use crate::ai::client::AiClient;
use crate::error::{AppError, CommandError, CommandResult};
use crate::error_log::ErrorLogStore;
use crate::models::{AiConfig, AiReviewRequest, AiReviewResult, AiStreamEvent};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn ai_get_config(state: State<'_, AppState>) -> CommandResult<AiConfig> {
    let mut config = state.ai_config.get_config().map_err(CommandError::from)?;
    // Never expose encrypted key to frontend
    config.api_key_encrypted = None;
    Ok(config)
}

#[tauri::command]
pub async fn ai_save_config(state: State<'_, AppState>, config: AiConfig) -> CommandResult<()> {
    // Merge: preserve encrypted key from existing config
    let existing = state.ai_config.get_config().unwrap_or_default();
    let mut merged = config;
    if merged.api_key_encrypted.is_none() {
        let encrypted_key = existing.api_key_encrypted.clone();
        merged.api_key_encrypted = encrypted_key.clone();
        merged.api_key_configured = encrypted_key.is_some();
    }
    state.ai_config.save_config(&merged).map_err(CommandError::from)
}

#[tauri::command]
pub async fn ai_save_api_key(state: State<'_, AppState>, api_key: String) -> CommandResult<()> {
    state.ai_config.save_api_key(&api_key).map_err(CommandError::from).map(|_| ())
}

#[tauri::command]
pub async fn ai_review(state: State<'_, AppState>, request: AiReviewRequest) -> CommandResult<AiReviewResult> {
    let config = state.ai_config.get_config().map_err(CommandError::from)?;
    let api_key = state.ai_config.get_api_key().map_err(CommandError::from)?;
    let _operation = state.operations.begin_ai().await?;

    let client = AiClient::new(config.endpoint, config.model, api_key);

    client
        .review(
            &request.diff,
            request.context.as_ref(),
            request.focus.as_ref(),
            config.system_prompt.as_deref(),
            config.temperature.unwrap_or(0.3),
            config.max_tokens.unwrap_or(8192),
        )
        .await
        .map_err(CommandError::from)
}

/// Streaming AI review — registers a cancellable background task and emits request-scoped events.
#[tauri::command]
pub async fn ai_review_stream(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    error_logs: State<'_, ErrorLogStore>,
    request_id: String,
    request: AiReviewRequest,
) -> CommandResult<()> {
    let config = state.ai_config.get_config().map_err(CommandError::from)?;
    let api_key = state.ai_config.get_api_key().map_err(CommandError::from)?;
    let system_prompt = config.system_prompt.clone();
    let temperature = config.temperature.unwrap_or(0.3);
    let max_tokens = config.max_tokens.unwrap_or(8192);
    let operation = state.operations.begin_ai().await?;
    let registry = state.ai_tasks.clone();
    let generation = registry.next_generation();
    let task_request_id = request_id.clone();
    let task_registry = registry.clone();
    let task_error_logs = error_logs.inner().clone();
    let (start_tx, start_rx) = tokio::sync::oneshot::channel();

    let task = tokio::spawn(async move {
        let _operation = operation;
        if start_rx.await.is_err() {
            return;
        }
        let client = AiClient::new(config.endpoint, config.model, api_key);
        let chunk_request_id = task_request_id.clone();
        let chunk_handle = app_handle.clone();
        let result = client
            .review_stream(
                &request.diff,
                request.context.as_ref(),
                request.focus.as_ref(),
                system_prompt.as_deref(),
                temperature,
                max_tokens,
                move |token| {
                    chunk_handle
                        .emit(
                            "ai-review-chunk",
                            AiStreamEvent { request_id: chunk_request_id.clone(), payload: token.to_string() },
                        )
                        .map_err(|error| AppError::Ai(format!("发送 AI 流事件失败: {error}")))
                },
            )
            .await;

        match result {
            Ok(review_result) => {
                let _ = app_handle.emit(
                    "ai-review-done",
                    AiStreamEvent { request_id: task_request_id.clone(), payload: review_result },
                );
            }
            Err(error) => {
                let error = CommandError::from(error);
                let log_store = task_error_logs.clone();
                let log_error = error.clone();
                let log_failed = !matches!(
                    tokio::task::spawn_blocking(move || log_store.record_command_error("ai_review_stream", &log_error))
                        .await,
                    Ok(Ok(()))
                );
                if log_failed {
                    let event = serde_json::json!({
                        "event": "error_log_write_failed",
                        "command": "ai_review_stream",
                    });
                    eprintln!("{event}");
                }
                let _ = app_handle
                    .emit("ai-review-error", AiStreamEvent { request_id: task_request_id.clone(), payload: error });
            }
        }
        task_registry.remove_if_current(&task_request_id, generation).await;
    });

    registry.replace(request_id, generation, task.abort_handle()).await;
    let _ = start_tx.send(());
    Ok(())
}

#[tauri::command]
pub async fn ai_review_cancel(state: State<'_, AppState>, request_id: String) -> CommandResult<()> {
    state.ai_tasks.cancel(&request_id).await;
    Ok(())
}
#[tauri::command]
pub async fn ai_list_models(state: State<'_, AppState>, endpoint: String) -> CommandResult<Vec<String>> {
    let api_key = state.ai_config.get_api_key().map_err(CommandError::from)?;

    // Use a dummy model name — list_models doesn't need a model
    let client = AiClient::new(endpoint, "".to_string(), api_key);

    client.list_models().await.map_err(CommandError::from)
}

#[tauri::command]
pub async fn ai_test_connection(state: State<'_, AppState>) -> CommandResult<bool> {
    let config = state.ai_config.get_config().map_err(CommandError::from)?;
    let api_key = state.ai_config.get_api_key().map_err(CommandError::from)?;

    let client = AiClient::new(config.endpoint, config.model, api_key);

    client.test_connection().await.map_err(CommandError::from)
}
