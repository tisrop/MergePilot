use crate::models::*;
use crate::state::AppState;
use tauri::State;

use super::auth::build_platform;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn review_submit(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
    body: String,
    event: String,
    _comments: Vec<ReviewCommentPosition>,
) -> Result<Review, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let review_event = match event.as_str() {
        "approve" => ReviewEvent::Approve,
        "request_changes" => ReviewEvent::RequestChanges,
        _ => ReviewEvent::Comment,
    };
    // TODO: handle inline comments (comments param) for platforms that support it
    p.create_review(&owner, &repo, pr_number, &body, &review_event)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn review_list(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
) -> Result<Vec<Review>, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.list_reviews(&owner, &repo, pr_number)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn review_comments_list(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
) -> Result<Vec<PrComment>, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.list_pr_comments(&owner, &repo, pr_number)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn review_comment_add(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
    commit_id: String,
    path: String,
    line: u32,
    body: String,
) -> Result<(), String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.create_pr_comment(&owner, &repo, pr_number, &commit_id, &path, line, &body)
        .await
        .map_err(|e| e.to_string())
}
