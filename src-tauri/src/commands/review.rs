use crate::local_store::{CommentSnapshot, CommentSnapshotStore};
use crate::models::*;
use crate::state::AppState;
use tauri::State;

use super::auth::build_platform;

fn value_id(value: &serde_json::Value) -> String {
    value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string())
}

fn validate_comment_body(body: &str) -> Result<String, String> {
    let normalized = body.trim();
    if normalized.is_empty() {
        return Err("评论内容不能为空".into());
    }
    if normalized.chars().count() > 64 * 1024 || normalized.contains('\0') {
        return Err("评论内容过长或包含非法字符".into());
    }
    Ok(normalized.to_string())
}

async fn ensure_owned_comment(
    platform: &dyn crate::platform::GitPlatform,
    owner: &str,
    repo: &str,
    pr_number: u64,
    thread_id: &str,
    comment_id: &str,
) -> Result<(), String> {
    let comments = platform.list_pr_comments(owner, repo, pr_number).await.map_err(|error| error.to_string())?;
    let current_user = platform.current_user().await.map_err(|error| error.to_string())?;
    let comment = comments
        .iter()
        .find(|comment| value_id(&comment.id) == comment_id && comment.thread_id == thread_id)
        .ok_or_else(|| "评论不存在，可能已被删除或线程已更新".to_string())?;
    if !comment.author.login.eq_ignore_ascii_case(&current_user.login) {
        return Err("只能编辑或删除自己的评论".into());
    }
    Ok(())
}

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

fn patch_matches_path(file: &PrFile, path: &str) -> bool {
    if file.filename == path {
        return true;
    }
    file.patch.lines().any(|line| {
        let Some(paths) = line.strip_prefix("diff --git a/") else {
            return false;
        };
        let Some((old_path, new_path)) = paths.split_once(" b/") else {
            return false;
        };
        old_path == path || new_path == path
    })
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
    p.create_review(&owner, &repo, pr_number, &body, &review_event, &comments).await.map_err(|e| e.to_string())
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
    p.list_reviews(&owner, &repo, pr_number).await.map_err(|e| e.to_string())
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
    let mut comments = p.list_pr_comments(&owner, &repo, pr_number).await.map_err(|e| e.to_string())?;
    let current_user = p.current_user().await.ok();
    for comment in &mut comments {
        let is_owned = current_user.as_ref().is_some_and(|user| comment.author.login.eq_ignore_ascii_case(&user.login));
        comment.can_edit = is_owned;
        comment.can_delete = is_owned;
    }
    // For platforms that don't return diff_hunk (e.g. GitLab and Gitee), supplement from SQLite.
    let snapshots = comment_store.get_snapshots_for_pr(&platform, &owner, &repo, pr_number).unwrap_or_default();
    let snapshot_map: std::collections::HashMap<String, CommentSnapshot> =
        snapshots.into_iter().map(|s| (s.comment_id.clone(), s)).collect();
    for c in &mut comments {
        let cid = value_id(&c.id);
        if c.diff_hunk.is_none() {
            if let Some(snap) = snapshot_map.get(&cid) {
                c.diff_hunk = snap.diff_hunk.clone();
            }
        }
    }

    // For comments still missing diff_hunk, extract the current hunk and cache it so
    // that outdated views retain context after the code changes.
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
                    (value_id(&c.id), c.path.clone(), c.line.unwrap())
                };
                if let Some(file) = files.iter().find(|f| patch_matches_path(f, &path)) {
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
    Ok(comments)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn review_thread_reply(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
    thread_id: String,
    reply_to_id: String,
    body: String,
) -> Result<(), String> {
    if thread_id.trim().is_empty() || reply_to_id.trim().is_empty() {
        return Err("评审线程和回复目标不能为空".into());
    }
    let body = validate_comment_body(&body)?;
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let comments = p.list_pr_comments(&owner, &repo, pr_number).await.map_err(|e| e.to_string())?;
    if !comments.iter().any(|comment| comment.thread_id == thread_id && value_id(&comment.id) == reply_to_id) {
        return Err("回复目标不存在，可能已被删除或线程已更新".into());
    }
    p.reply_to_review_thread(&owner, &repo, pr_number, &thread_id, &reply_to_id, &body).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn review_comment_update(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
    thread_id: String,
    comment_id: String,
    body: String,
) -> Result<(), String> {
    if thread_id.trim().is_empty() || comment_id.trim().is_empty() {
        return Err("评审线程和评论 ID 不能为空".into());
    }
    let body = validate_comment_body(&body)?;
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    ensure_owned_comment(&*p, &owner, &repo, pr_number, &thread_id, &comment_id).await?;
    p.update_review_comment(&owner, &repo, pr_number, &thread_id, &comment_id, &body).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn review_comment_delete(
    state: State<'_, AppState>,
    comment_store: State<'_, CommentSnapshotStore>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
    thread_id: String,
    comment_id: String,
) -> Result<(), String> {
    if thread_id.trim().is_empty() || comment_id.trim().is_empty() {
        return Err("评审线程和评论 ID 不能为空".into());
    }
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    ensure_owned_comment(&*p, &owner, &repo, pr_number, &thread_id, &comment_id).await?;
    p.delete_review_comment(&owner, &repo, pr_number, &thread_id, &comment_id).await.map_err(|e| e.to_string())?;
    let _ = comment_store.delete_snapshot(&comment_id, &platform);
    Ok(())
}

#[tauri::command]
pub async fn review_thread_set_resolved(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
    thread_id: String,
    resolved: bool,
) -> Result<(), String> {
    if thread_id.trim().is_empty() {
        return Err("评审线程 ID 不能为空".to_string());
    }
    let capabilities =
        crate::platform::capabilities_for(&platform).ok_or_else(|| format!("不支持的平台：{platform}"))?;
    if !capabilities.supports_review_thread_resolution {
        return Err(format!("{platform} 不支持解决或重新打开评审线程"));
    }
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.set_review_thread_resolved(&owner, &repo, pr_number, &thread_id, resolved).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn review_viewed_files_list(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
) -> Result<Vec<String>, String> {
    let capabilities =
        crate::platform::capabilities_for(&platform).ok_or_else(|| format!("不支持的平台：{platform}"))?;
    if !capabilities.supports_remote_file_viewed_state {
        return Err(format!("{platform} 不支持同步文件已查看状态"));
    }
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.list_viewed_pr_files(&owner, &repo, pr_number).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn review_file_set_viewed(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    pr_number: u64,
    path: String,
    viewed: bool,
) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    let capabilities =
        crate::platform::capabilities_for(&platform).ok_or_else(|| format!("不支持的平台：{platform}"))?;
    if !capabilities.supports_remote_file_viewed_state {
        return Err(format!("{platform} 不支持同步文件已查看状态"));
    }
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.set_pr_file_viewed(&owner, &repo, pr_number, &path, viewed).await.map_err(|e| e.to_string())
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
        .create_pr_comment(&owner, &repo, pr_number, &commit_id, &path, start_line, line, &side, &body)
        .await
        .map_err(|e| e.to_string())?;
    // If the platform didn't return diff_hunk but the caller provided it, write to SQLite
    if comment.diff_hunk.is_none() {
        if let Some(dh) = diff_hunk {
            comment.diff_hunk = Some(dh.clone());
            let snapshot = CommentSnapshot {
                comment_id: value_id(&comment.id),
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
            comment_store.save_snapshot(&snapshot).map_err(|e| e.to_string())?;
        }
    }
    Ok(comment)
}

#[cfg(test)]
mod tests {
    use super::{extract_hunk_from_patch, patch_matches_path, validate_comment_body};
    use crate::models::{FileStatus, PrFile};

    #[test]
    fn validates_thread_comment_body() {
        assert_eq!(validate_comment_body("  回复内容  ").unwrap(), "回复内容");
        assert!(validate_comment_body("   ").is_err());
        assert!(validate_comment_body("bad\0body").is_err());
    }

    #[test]
    fn matches_renamed_diff_paths_and_extracts_original_hunk() {
        let file = PrFile {
            filename: "src/new.rs".into(),
            status: FileStatus::Renamed,
            patch: "diff --git a/src/old.rs b/src/new.rs\n@@ -4 +4 @@\n-old\n+new".into(),
            additions: 1,
            deletions: 1,
        };
        assert!(patch_matches_path(&file, "src/old.rs"));
        assert!(patch_matches_path(&file, "src/new.rs"));
        assert!(extract_hunk_from_patch(&file.patch, 4).is_some());
    }
}
