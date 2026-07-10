use crate::models::*;
use crate::state::AppState;
use tauri::State;

use super::auth::build_platform;

#[tauri::command]
pub async fn pr_list(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    state_filter: Option<String>,
    page: Option<u32>,
    per_page: Option<u32>,
) -> Result<Paginated<PrSummary>, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let pr_state = match state_filter.as_deref() {
        Some("closed") => PrState::Closed,
        Some("merged") => PrState::Merged,
        Some("all") => PrState::All,
        _ => PrState::Open,
    };
    p.list_pull_requests(
        &owner,
        &repo,
        &pr_state,
        page.unwrap_or(1),
        per_page.unwrap_or(20),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pr_detail(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<PrDetail, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.get_pull_request(&owner, &repo, number)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pr_diff(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<DiffResult, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let (diff, files) = p
        .get_pr_diff(&owner, &repo, number)
        .await
        .map_err(|e| e.to_string())?;
    Ok(DiffResult { diff, files })
}
