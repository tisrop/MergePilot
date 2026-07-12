use crate::models::*;
use crate::state::AppState;
use tauri::State;

use super::auth::build_platform;

#[tauri::command]
pub async fn issue_list(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    state_filter: Option<String>,
    page: Option<u32>,
) -> Result<Paginated<IssueSummary>, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let issue_state = match state_filter.as_deref() {
        Some("closed") => IssueState::Closed,
        Some("all") => IssueState::All,
        _ => IssueState::Open,
    };
    p.list_issues(&owner, &repo, &issue_state, page.unwrap_or(1)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn issue_create(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    title: String,
    body: String,
    labels: Vec<String>,
) -> Result<Issue, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.create_issue(&owner, &repo, &title, &body, &labels).await.map_err(|e| e.to_string())
}
