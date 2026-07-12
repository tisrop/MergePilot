use crate::models::*;
use crate::state::AppState;
use tauri::State;

use super::auth::build_platform;

fn extract_issue_refs(body: &str) -> Vec<u64> {
    let keywords = ["close", "closes", "closed", "fix", "fixes", "fixed", "resolve", "resolves", "resolved"];
    let mut issues = Vec::new();
    let words: Vec<&str> = body.split(|c: char| c.is_whitespace() || c == ',').collect();
    for (i, word) in words.iter().enumerate() {
        let lower = word.to_lowercase();
        if keywords.contains(&lower.as_str()) {
            if let Some(next) = words.get(i + 1) {
                if let Some(num_str) = next.strip_prefix('#') {
                    if let Ok(num) = num_str.parse::<u64>() {
                        issues.push(num);
                    }
                }
            }
        }
    }
    issues
}

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
    p.list_pull_requests(&owner, &repo, &pr_state, page.unwrap_or(1), per_page.unwrap_or(20))
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
    p.get_pull_request(&owner, &repo, number).await.map_err(|e| e.to_string())
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
    let (diff, files) = p.get_pr_diff(&owner, &repo, number).await.map_err(|e| e.to_string())?;
    Ok(DiffResult { diff, files })
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn pr_merge(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
    strategy: String,
    commit_title: Option<String>,
    commit_message: Option<String>,
    close_issues: Option<bool>,
) -> Result<PrMergeOutcome, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let pr_detail = p.get_pull_request(&owner, &repo, number).await.map_err(|e| e.to_string())?;
    let merge_strategy = match strategy.as_str() {
        "squash" => MergeStrategy::Squash,
        "rebase" => MergeStrategy::Rebase,
        _ => MergeStrategy::Merge,
    };
    let result = p
        .merge_pull_request(&owner, &repo, number, &merge_strategy, commit_title, commit_message, &pr_detail.head_sha)
        .await
        .map_err(|e| e.to_string())?;

    let mut closed_issues = Vec::new();
    let mut issue_close_failures = Vec::new();
    if close_issues.unwrap_or(false) {
        for issue_num in extract_issue_refs(&pr_detail.body) {
            match p.close_issue(&owner, &repo, issue_num).await {
                Ok(()) => closed_issues.push(issue_num),
                Err(error) => {
                    issue_close_failures.push(IssueCloseFailure { number: issue_num, error: error.to_string() })
                }
            }
        }
    }

    Ok(PrMergeOutcome { merge: result, closed_issues, issue_close_failures })
}

#[tauri::command]
pub async fn pr_close(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<PrState, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.close_pull_request(&owner, &repo, number).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pr_reopen(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<PrState, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.reopen_pull_request(&owner, &repo, number).await.map_err(|e| e.to_string())
}
