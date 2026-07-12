use crate::local_store::{CommentSnapshot, CommentSnapshotStore};
use crate::models::*;
use crate::state::AppState;
use tauri::State;

use super::auth::build_platform;

/// Parse the new-file start line from a unified-diff hunk header like `@@ -1,3 +10,4 @@`.
fn parse_hunk_new_start(line: &str) -> Option<i64> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "@@" {
        return None;
    }
    let _old = parts.next()?;
    let new_part = parts.next()?;
    let new_part = new_part.strip_prefix('+')?;
    let num_str = new_part.split(',').next()?;
    num_str.parse::<i64>().ok()
}

/// Extract the diff hunk containing `line` from a unified-diff patch.
/// Mirrors the frontend `extractHunkFromPatch` logic in ReviewList.vue, so that
/// platforms whose API does not return `diff_hunk` (e.g. Gitee) can still show
/// the commented code context.
fn extract_hunk_from_patch(patch: &str, line: u32) -> Option<String> {
    let mut current_line: i64 = 0;
    let mut result: Vec<&str> = Vec::new();
    let mut in_range = false;
    let target = line as i64;

    for pl in patch.lines() {
        if let Some(new_start) = parse_hunk_new_start(pl) {
            if in_range {
                break;
            }
            current_line = new_start - 1;
            result.clear();
            result.push(pl);
            continue;
        }
        if result.is_empty() {
            continue;
        }
        result.push(pl);
        if !pl.starts_with('-') {
            current_line += 1;
        }
        if current_line >= target && !in_range {
            in_range = true;
        }
        if in_range && current_line > target + 8 {
            break;
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result.join("\n"))
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn review_submit(
    state: State<'_, AppState>,
    _comment_store: State<'_, CommentSnapshotStore>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
    body: String,
    event: String,
    comments: Vec<ReviewCommentPosition>,
) -> Result<Review, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let review_event = match event.as_str() {
        "approve" => ReviewEvent::Approve,
        "request_changes" => ReviewEvent::RequestChanges,
        _ => ReviewEvent::Comment,
    };
    p.create_review(&owner, &repo, pr_number, &body, &review_event, &comments)
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
    comment_store: State<'_, CommentSnapshotStore>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
) -> Result<Vec<PrComment>, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let mut comments = p
        .list_pr_comments(&owner, &repo, pr_number)
        .await
        .map_err(|e| e.to_string())?;
    // For platforms that don't return diff_hunk (e.g. Gitee), supplement from SQLite
    if platform == "gitee" {
        let snapshots = comment_store
            .get_snapshots_for_pr(&platform, &owner, &repo, pr_number)
            .unwrap_or_default();
        let snapshot_map: std::collections::HashMap<String, CommentSnapshot> = snapshots
            .into_iter()
            .map(|s| (s.comment_id.clone(), s))
            .collect();
        for c in &mut comments {
            let cid = format!("{}", c.id);
            if c.diff_hunk.is_none() {
                if let Some(snap) = snapshot_map.get(&cid) {
                    c.diff_hunk = snap.diff_hunk.clone();
                }
            }
        }

        // For comments still missing diff_hunk (e.g. created via Gitee web UI),
        // extract the hunk from the current PR diff and cache it to SQLite so
        // that outdated views (after the code is updated) still have context.
        let missing_indices: Vec<usize> = comments
            .iter()
            .enumerate()
            .filter(|(_, c)| c.diff_hunk.is_none() && !c.path.is_empty() && c.line.is_some())
            .map(|(i, _)| i)
            .collect();
        if !missing_indices.is_empty() {
            if let Ok((_, files)) = p.get_pr_diff(&owner, &repo, pr_number).await {
                for i in missing_indices {
                    let (cid, path, line) = {
                        let c = &comments[i];
                        (format!("{}", c.id), c.path.clone(), c.line.unwrap())
                    };
                    if let Some(file) = files.iter().find(|f| f.filename == path) {
                        if let Some(hunk) = extract_hunk_from_patch(&file.patch, line) {
                            let snapshot = CommentSnapshot {
                                comment_id: cid,
                                platform: platform.clone(),
                                owner: owner.clone(),
                                repo: repo.clone(),
                                pr_number,
                                commit_id: comments[i].commit_id.clone(),
                                original_commit_id: comments[i].original_commit_id.clone(),
                                diff_hunk: Some(hunk.clone()),
                                original_line: comments[i].original_line,
                                original_start_line: comments[i].original_start_line,
                            };
                            let _ = comment_store.save_snapshot(&snapshot);
                            comments[i].diff_hunk = Some(hunk);
                        }
                    }
                }
            }
        }
    }
    Ok(comments)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn review_comment_add(
    state: State<'_, AppState>,
    comment_store: State<'_, CommentSnapshotStore>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
    commit_id: String,
    path: String,
    start_line: Option<u32>,
    line: u32,
    side: String,
    body: String,
    diff_hunk: Option<String>,
) -> Result<PrComment, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let mut comment = p
        .create_pr_comment(
            &owner, &repo, pr_number, &commit_id, &path, start_line, line, &side, &body,
        )
        .await
        .map_err(|e| e.to_string())?;
    // If the platform didn't return diff_hunk but the caller provided it, write to SQLite
    if comment.diff_hunk.is_none() {
        if let Some(dh) = diff_hunk {
            comment.diff_hunk = Some(dh.clone());
            let snapshot = CommentSnapshot {
                comment_id: format!("{}", comment.id),
                platform: platform.clone(),
                owner: owner.clone(),
                repo: repo.clone(),
                pr_number,
                commit_id: comment.commit_id.clone(),
                original_commit_id: comment.original_commit_id.clone(),
                diff_hunk: Some(dh),
                original_line: comment.original_line,
                original_start_line: comment.original_start_line,
            };
            comment_store
                .save_snapshot(&snapshot)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(comment)
}
