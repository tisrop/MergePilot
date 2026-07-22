use async_trait::async_trait;
use serde_json::Value;
use sha1::{Digest, Sha1};

use super::GitPlatform;
use crate::error::AppError;
use crate::http_client::HttpClient;
use crate::models::*;

pub struct GitLabAdapter {
    client: HttpClient,
    token: String,
    base_url: String,
}

impl GitLabAdapter {
    pub fn new(client: HttpClient, token: String) -> Self {
        Self { client, token, base_url: "https://gitlab.com/api/v4".to_string() }
    }

    #[allow(dead_code)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = super::normalize_api_base("gitlab", &url);
        self
    }

    #[allow(dead_code)]
    fn auth_header(&self) -> String {
        format!("PRIVATE-TOKEN: {}", self.token)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, AppError> {
        let resp = self
            .client
            .get(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn list_commit_diffs(&self, project: &str, sha: &str) -> Result<(Vec<Value>, bool), AppError> {
        const PAGE_SIZE: usize = 100;

        let mut changes = Vec::new();
        let mut incomplete = false;
        let mut page = 1_u32;
        loop {
            let url = format!(
                "{}/projects/{}/repository/commits/{}/diff?per_page={}&page={}",
                self.base_url, project, sha, PAGE_SIZE, page
            );
            let response = self
                .client
                .get(&url)
                .header("PRIVATE-TOKEN", &self.token)
                .header("User-Agent", "mergebeacon")
                .send()
                .await?;
            let status = response.status();
            let next_page_header = response.headers().get("x-next-page");
            let next_page = next_page_header
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.is_empty())
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|next| *next > page);
            let total_pages = response
                .headers()
                .get("x-total-pages")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u32>().ok());
            let pagination_known = next_page_header.is_some() || total_pages.is_some();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::Api(format!("GitLab API {} ({}): {}", status, url, body)));
            }
            let page_changes: Vec<Value> = response.json().await?;
            let page_size = page_changes.len();
            incomplete |= page_changes.iter().any(|change| {
                change["collapsed"].as_bool() == Some(true) || change["too_large"].as_bool() == Some(true)
            });
            changes.extend(page_changes);

            if let Some(next) = next_page {
                page = next;
                continue;
            }
            if total_pages.is_some_and(|total| page < total) {
                page = page.saturating_add(1);
                continue;
            }
            if !pagination_known && page_size >= PAGE_SIZE {
                incomplete = true;
            }
            break;
        }

        Ok((changes, incomplete))
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let resp = self
            .client
            .post(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn put_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let resp = self
            .client
            .put(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn delete_empty(&self, url: &str) -> Result<(), AppError> {
        self.client
            .raw_client()
            .delete(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn list_all_inbox_merge_requests(&self, filter_name: &str, user_id: u64) -> Result<Vec<Value>, AppError> {
        const REMOTE_PAGE_SIZE: u64 = 100;
        let url = format!("{}/merge_requests", self.base_url);
        let mut page = 1_u64;
        let mut items = Vec::new();

        loop {
            let response = self
                .client
                .raw_client()
                .get(&url)
                .header("PRIVATE-TOKEN", &self.token)
                .header("User-Agent", "mergebeacon")
                .query(&[("scope", "all"), ("state", "opened"), ("order_by", "updated_at"), ("sort", "desc")])
                .query(&[(filter_name, user_id), ("page", page), ("per_page", REMOTE_PAGE_SIZE)])
                .send()
                .await?
                .error_for_status()?;
            let total_pages = response
                .headers()
                .get("x-total-pages")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let page_items: Vec<Value> = response.json().await?;
            let fetched = page_items.len() as u64;
            items.extend(page_items);

            let has_more = total_pages.map_or(fetched == REMOTE_PAGE_SIZE, |total| page < total);
            if !has_more {
                break;
            }
            page = page.saturating_add(1);
        }

        Ok(items)
    }

    fn repository_from_merge_request(json: &Value) -> Result<(String, String), AppError> {
        let full_reference = json["references"]["full"]
            .as_str()
            .ok_or_else(|| AppError::Api("GitLab 收件箱响应缺少 references.full".into()))?;
        let full_name = full_reference.rsplit_once('!').map(|(path, _)| path).unwrap_or(full_reference);
        let (owner, repo) = full_name
            .rsplit_once('/')
            .filter(|(owner, repo)| !owner.is_empty() && !repo.is_empty())
            .ok_or_else(|| AppError::Api("GitLab 收件箱响应中的项目路径无效".into()))?;
        Ok((owner.to_string(), repo.to_string()))
    }

    fn map_user(json: &Value) -> User {
        User {
            id: json["id"].clone(),
            login: json["username"].as_str().unwrap_or("").to_string(),
            name: json["name"].as_str().unwrap_or("").to_string(),
            avatar_url: json["avatar_url"].as_str().unwrap_or("").to_string(),
        }
    }

    fn metadata_milestone(json: &Value) -> Option<PrMilestone> {
        (!json.is_null()).then(|| PrMilestone {
            id: json["id"].clone(),
            number: json["iid"].as_u64().or_else(|| json["id"].as_u64()),
            title: json["title"].as_str().unwrap_or("").to_string(),
        })
    }

    fn map_merge_train_entry(entry: &Value, position: Option<u32>, total: Option<u32>) -> PrMergeQueueStatus {
        let raw_status = entry["status"].as_str().unwrap_or("");
        let pipeline_status = entry["pipeline"]["status"].as_str().map(str::to_string);
        let (state, failure_reason) = match pipeline_status.as_deref() {
            Some("failed" | "canceled" | "skipped") => (
                MergeQueueState::Failed,
                Some(format!("Merge Train Pipeline 状态为 {}", pipeline_status.as_deref().unwrap_or("unknown"))),
            ),
            Some("manual") => (MergeQueueState::Blocked, Some("Merge Train Pipeline 正在等待手动操作".into())),
            Some("running" | "pending" | "created" | "preparing" | "scheduled" | "waiting_for_resource") => {
                (MergeQueueState::Waiting, None)
            }
            Some("success") if raw_status == "merging" => (MergeQueueState::Merging, None),
            Some("success") if matches!(raw_status, "merged" | "skip_merged") => (MergeQueueState::Merged, None),
            Some("success") => (MergeQueueState::Ready, None),
            _ => match raw_status {
                "idle" => (MergeQueueState::Queued, None),
                "fresh" => (MergeQueueState::Waiting, None),
                "stale" => (MergeQueueState::Blocked, Some("Merge Train 条目已过期，等待重新生成".into())),
                "merging" => (MergeQueueState::Merging, None),
                "merged" | "skip_merged" => (MergeQueueState::Merged, None),
                _ => (MergeQueueState::Unknown, None),
            },
        };
        PrMergeQueueStatus {
            kind: MergeQueueKind::MergeTrain,
            available: true,
            state,
            position,
            total,
            target_branch: entry["target_branch"].as_str().map(str::to_string),
            enqueued_at: entry["created_at"].as_str().map(str::to_string),
            updated_at: entry["updated_at"].as_str().map(str::to_string),
            estimated_time_seconds: None,
            head_sha: entry["pipeline"]["sha"].as_str().map(str::to_string),
            pipeline_status,
            failure_reason,
        }
    }

    async fn merge_train_response(&self, url: &str) -> Result<Option<reqwest::Response>, AppError> {
        let response = self
            .client
            .get(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .send()
            .await?;
        if matches!(response.status(), reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND) {
            return Ok(None);
        }
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitLab Merge Train API {status}: {body}")));
        }
        Ok(Some(response))
    }

    fn known_or(left: Option<bool>, right: Option<bool>) -> Option<bool> {
        match (left, right) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        }
    }

    fn access_level(project: &Value) -> Option<u64> {
        [
            project["permissions"]["project_access"]["access_level"].as_u64(),
            project["permissions"]["group_access"]["access_level"].as_u64(),
        ]
        .into_iter()
        .flatten()
        .max()
    }

    async fn metadata_permissions(&self, owner: &str, repo: &str, author_login: &str) -> PrMetadataPermissions {
        let project_id = urlencoding(owner, repo);
        let user_url = format!("{}/user", self.base_url);
        let project_url = format!("{}/projects/{}", self.base_url, project_id);
        let (user, project) = tokio::join!(self.get_json::<Value>(&user_url), self.get_json::<Value>(&project_url));
        let is_author =
            user.ok().and_then(|user| user["username"].as_str().map(|login| login.eq_ignore_ascii_case(author_login)));
        let can_write = project.ok().and_then(|project| Self::access_level(&project).map(|level| level >= 30));
        let can_edit = Self::known_or(is_author, can_write);
        PrMetadataPermissions {
            can_edit_title_body: can_edit,
            can_toggle_draft: can_edit,
            can_manage_reviewers: can_write,
            can_manage_assignees: can_write,
            can_manage_labels: can_write,
            can_manage_milestone: can_write,
        }
    }

    fn title_for_draft(title: &str, draft: Option<bool>) -> String {
        let trimmed = title.trim();
        let lower = trimmed.to_ascii_lowercase();
        let without_prefix = ["draft:", "wip:"]
            .into_iter()
            .find(|prefix| lower.starts_with(prefix))
            .map(|prefix| trimmed[prefix.len()..].trim_start())
            .unwrap_or(trimmed);
        if draft == Some(true) {
            format!("Draft: {without_prefix}")
        } else {
            without_prefix.to_string()
        }
    }

    async fn resolve_user_ids(&self, logins: &[String]) -> Result<Vec<u64>, AppError> {
        let mut ids = Vec::with_capacity(logins.len());
        for login in logins {
            let encoded = urlencoding::encode(login);
            let url = format!("{}/users?username={}", self.base_url, encoded);
            let users = self.get_json::<Value>(&url).await?;
            let id = users
                .as_array()
                .and_then(|users| {
                    users
                        .iter()
                        .find(|user| user["username"].as_str() == Some(login))
                        .and_then(|user| user["id"].as_u64())
                })
                .ok_or_else(|| AppError::Api(format!("GitLab 中不存在用户：{login}")))?;
            ids.push(id);
        }
        Ok(ids)
    }

    async fn resolve_milestone_id(&self, owner: &str, repo: &str, title: &str) -> Result<u64, AppError> {
        let project_id = urlencoding(owner, repo);
        let encoded = urlencoding::encode(title);
        let url = format!("{}/projects/{}/milestones?state=all&title={}", self.base_url, project_id, encoded);
        let milestones = self.get_json::<Value>(&url).await?;
        milestones
            .as_array()
            .and_then(|items| {
                items.iter().find(|item| item["title"].as_str() == Some(title)).and_then(|item| item["id"].as_u64())
            })
            .ok_or_else(|| AppError::Api(format!("GitLab 项目中不存在 Milestone：{title}")))
    }

    fn required_string(json: &Value, field: &str, label: &str) -> Result<String, AppError> {
        json[field]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| AppError::Api(format!("GitLab {label} 缺少 {field}")))
    }

    fn inbox_status(mr: &Value) -> ReviewInboxStatusSummary {
        let draft = mr["draft"].as_bool().or_else(|| mr["work_in_progress"].as_bool());
        let merge_status = mr["detailed_merge_status"].as_str().or_else(|| mr["merge_status"].as_str()).unwrap_or("");
        let has_conflicts = mr["has_conflicts"].as_bool().or(match merge_status {
            "conflict" | "conflicts" | "cannot_be_merged" => Some(true),
            "mergeable" | "can_be_merged" => Some(false),
            _ => None,
        });
        let mut blocking_reasons = Vec::new();
        if draft == Some(true) {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Draft,
                message: "MR 仍处于 Draft 状态".into(),
            });
        }
        if has_conflicts == Some(true) {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Conflicts,
                message: "源分支存在合并冲突".into(),
            });
        }

        let checks_status = match mr["head_pipeline"]["status"].as_str() {
            Some("success") | Some("skipped") => ReadinessState::Ready,
            Some("failed") | Some("canceled") => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ChecksFailed,
                    message: "CI 检查未通过".into(),
                });
                ReadinessState::Blocked
            }
            Some("created")
            | Some("waiting_for_resource")
            | Some("preparing")
            | Some("pending")
            | Some("running")
            | Some("scheduled")
            | Some("manual") => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ChecksPending,
                    message: "CI 检查仍在进行中".into(),
                });
                ReadinessState::Pending
            }
            Some(_) => ReadinessState::Unknown,
            None if matches!(merge_status, "mergeable" | "can_be_merged") => ReadinessState::Ready,
            None if matches!(merge_status, "ci_still_running" | "checking" | "unchecked") => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ChecksPending,
                    message: "CI 或合并状态仍在检查中".into(),
                });
                ReadinessState::Pending
            }
            None => ReadinessState::Unknown,
        };

        let approvals_status = match merge_status {
            "requested_changes" => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ChangesRequested,
                    message: "已有评审请求修改".into(),
                });
                ReadinessState::Blocked
            }
            "not_approved" | "approvals_syncing" => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ApprovalsRequired,
                    message: "审批尚未满足合并要求".into(),
                });
                ReadinessState::Blocked
            }
            "mergeable" | "can_be_merged" => ReadinessState::Ready,
            _ => ReadinessState::Unknown,
        };

        if matches!(merge_status, "need_rebase" | "behind") {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::BranchBehind,
                message: "源分支落后于目标分支".into(),
            });
        }
        if merge_status == "discussions_not_resolved" || mr["blocking_discussions_resolved"].as_bool() == Some(false) {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::DiscussionsUnresolved,
                message: "仍有未解决的讨论线程".into(),
            });
        }
        if matches!(merge_status, "blocked_status" | "broken_status") {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::PlatformBlocked,
                message: "GitLab 报告当前 MR 被平台规则阻塞".into(),
            });
        }

        let has_hard_blocker =
            blocking_reasons.iter().any(|reason| reason.code != MergeBlockingReasonCode::ChecksPending);
        let status = if has_hard_blocker
            || checks_status == ReadinessState::Blocked
            || approvals_status == ReadinessState::Blocked
        {
            ReadinessState::Blocked
        } else if checks_status == ReadinessState::Pending {
            ReadinessState::Pending
        } else if matches!(merge_status, "mergeable" | "can_be_merged")
            && draft != Some(true)
            && has_conflicts != Some(true)
        {
            ReadinessState::Ready
        } else {
            ReadinessState::Unknown
        };

        ReviewInboxStatusSummary { status, draft, has_conflicts, checks_status, approvals_status, blocking_reasons }
    }

    fn unified_diff(change: &Value) -> String {
        let patch = change["diff"].as_str().unwrap_or("");
        let old_path = change["old_path"].as_str().unwrap_or("");
        let new_path = change["new_path"].as_str().unwrap_or("");
        if patch.trim().is_empty() {
            let is_metadata_only_rename = change["renamed_file"].as_bool() == Some(true)
                && change["collapsed"].as_bool() != Some(true)
                && change["too_large"].as_bool() != Some(true)
                && change["additions"].as_u64().unwrap_or(0) == 0
                && change["deletions"].as_u64().unwrap_or(0) == 0
                && !old_path.is_empty()
                && !new_path.is_empty()
                && old_path != new_path;
            return if is_metadata_only_rename {
                crate::patch::metadata_only_rename_patch(old_path, new_path)
            } else {
                String::new()
            };
        }

        let mut diff = if patch.starts_with("diff --git ") {
            patch.to_string()
        } else {
            let old_marker = if change["new_file"].as_bool() == Some(true) {
                "/dev/null".to_string()
            } else {
                format!("a/{old_path}")
            };
            let new_marker = if change["deleted_file"].as_bool() == Some(true) {
                "/dev/null".to_string()
            } else {
                format!("b/{new_path}")
            };

            format!("diff --git a/{old_path} b/{new_path}\n--- {old_marker}\n+++ {new_marker}\n{patch}")
        };
        if !diff.ends_with('\n') {
            diff.push('\n');
        }
        diff
    }

    fn line_code(path: &str, old_line: Option<u32>, new_line: Option<u32>) -> String {
        let path_hash = format!("{:x}", Sha1::digest(path.as_bytes()));
        format!("{path_hash}_{}_{}", old_line.unwrap_or(0), new_line.unwrap_or(0))
    }

    fn line_position(side: &str, line: u32) -> Result<(Option<u32>, Option<u32>, &'static str), AppError> {
        match side {
            "left" => Ok((Some(line), None, "old")),
            "right" => Ok((None, Some(line), "new")),
            _ => Err(AppError::Api("GitLab 行级评论 side 必须为 left 或 right".into())),
        }
    }

    fn value_id(value: &Value) -> String {
        value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string())
    }

    fn map_pr_comment(
        note: &Value,
        thread_id: &str,
        fallback_position: &Value,
        reply_to_id: Option<String>,
        resolved: Option<bool>,
        resolvable: bool,
    ) -> Option<PrComment> {
        let position =
            note.get("position").filter(|position| position.is_object()).unwrap_or(fallback_position).as_object()?;
        let position = Value::Object(position.clone());
        let new_line = position["new_line"].as_u64();
        let old_line = position["old_line"].as_u64();
        let side = if new_line.is_some() {
            Some("right")
        } else if old_line.is_some() {
            Some("left")
        } else {
            None
        };
        let path = match side {
            Some("left") => position["old_path"].as_str().or_else(|| position["new_path"].as_str()),
            _ => position["new_path"].as_str().or_else(|| position["old_path"].as_str()),
        }?
        .to_string();
        let line = new_line.or(old_line).map(|line| line as u32);
        let start_position = &position["line_range"]["start"];
        let start_line = match side {
            Some("left") => start_position["old_line"].as_u64().or_else(|| start_position["new_line"].as_u64()),
            _ => start_position["new_line"].as_u64().or_else(|| start_position["old_line"].as_u64()),
        }
        .map(|line| line as u32)
        .filter(|start| Some(*start) != line);
        let original = note.get("original_position").filter(|original| original.is_object()).unwrap_or(&position);
        let original_line = match side {
            Some("left") => original["old_line"].as_u64().or_else(|| original["new_line"].as_u64()),
            _ => original["new_line"].as_u64().or_else(|| original["old_line"].as_u64()),
        }
        .map(|line| line as u32);
        let original_start = &original["line_range"]["start"];
        let original_start_line = match side {
            Some("left") => original_start["old_line"].as_u64().or_else(|| original_start["new_line"].as_u64()),
            _ => original_start["new_line"].as_u64().or_else(|| original_start["old_line"].as_u64()),
        }
        .map(|line| line as u32);

        Some(PrComment {
            id: note["id"].clone(),
            body: note["body"].as_str().unwrap_or("").to_string(),
            path,
            line,
            start_line,
            side: side.map(str::to_string),
            author: Self::map_user(&note["author"]),
            created_at: note["created_at"].as_str().unwrap_or("").to_string(),
            commit_id: position["head_sha"].as_str().map(str::to_string),
            original_commit_id: original["head_sha"].as_str().map(str::to_string),
            original_line,
            original_start_line,
            diff_hunk: None,
            thread_id: thread_id.to_string(),
            reply_to_id,
            resolved,
            resolvable,
            can_edit: false,
            can_delete: false,
        })
    }
}
#[async_trait]
impl super::JsonPageSource for GitLabAdapter {
    async fn fetch_json_page(&self, endpoint: &str, page: u32) -> Result<super::JsonPage, AppError> {
        let url = super::json_page_url(endpoint, page);
        let response = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .send()
            .await?;
        let status = response.status();
        let next_page_header = response.headers().get("x-next-page");
        let next_page = next_page_header
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|next| *next > page);
        let total_pages = response
            .headers()
            .get("x-total-pages")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok());
        let pagination_known = next_page_header.is_some() || total_pages.is_some();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitLab API {status} ({url}): {body}")));
        }
        let items = response.json().await?;
        Ok(super::JsonPage {
            items,
            next_page: next_page.or_else(|| total_pages.filter(|total| page < *total).map(|_| page.saturating_add(1))),
            pagination_known,
        })
    }
}

#[async_trait]
impl GitPlatform for GitLabAdapter {
    fn name(&self) -> &'static str {
        "gitlab"
    }

    async fn current_user(&self) -> Result<User, AppError> {
        let url = format!("{}/user", self.base_url);
        let json = self.get_json::<Value>(&url).await?;
        Ok(Self::map_user(&json))
    }

    async fn list_repos(&self, page: u32) -> Result<Paginated<RepoSummary>, AppError> {
        let url = format!("{}/projects?membership=true&per_page=100&page={}", self.base_url, page);
        let resp = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .send()
            .await?;
        let total_count = resp
            .headers()
            .get("x-total")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        let next_page = resp
            .headers()
            .get("x-next-page")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok());
        let total_pages = resp
            .headers()
            .get("x-total-pages")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok())
            .or_else(|| (total_count > 0).then(|| total_count.div_ceil(100)))
            .or(next_page)
            .unwrap_or(page)
            .max(page);
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitLab API {status} ({url}): {body}")));
        }
        let items: Vec<Value> = resp.json().await?;

        let repos: Vec<RepoSummary> = items
            .iter()
            .map(|r| {
                let path = r["path_with_namespace"].as_str().unwrap_or("");
                let fork = r["forked_from_project"].is_object();
                let (parent_full_name, parent_owner) = if fork {
                    let parent_name = r["forked_from_project"]["path_with_namespace"].as_str().map(|s| s.to_string());
                    let parent_owner = r["forked_from_project"]["namespace"]["path"].as_str().map(|s| s.to_string());
                    (parent_name, parent_owner)
                } else {
                    (None, None)
                };
                let owner_type = match r["namespace"]["kind"].as_str() {
                    Some("group") => "organization",
                    _ => "user",
                };
                let owner_path = r["namespace"]["path"].as_str().unwrap_or("").to_string();
                let owner_display_name = r["namespace"]["name"].as_str().unwrap_or(&owner_path).to_string();
                RepoSummary {
                    id: r["id"].clone(),
                    name: r["name"].as_str().unwrap_or("").to_string(),
                    full_name: path.to_string(),
                    owner: owner_path,
                    owner_type: owner_type.to_string(),
                    owner_display_name,
                    description: r["description"].as_str().unwrap_or("").to_string(),
                    private: r["visibility"].as_str().unwrap_or("") != "public",
                    fork,
                    parent_full_name,
                    parent_owner,
                }
            })
            .collect();

        Ok(Paginated { items: repos, page, total_pages, total_count })
    }

    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: &PrState,
        page: u32,
        per_page: u32,
    ) -> Result<Paginated<PrSummary>, AppError> {
        let project_id = urlencoding(owner, repo);
        let state_param = match state {
            PrState::All => "all",
            PrState::Merged => "merged",
            PrState::Closed => "closed",
            PrState::Open => "opened",
        };
        let url = format!(
            "{}/projects/{}/merge_requests?state={}&per_page={}&page={}",
            self.base_url, project_id, state_param, per_page, page
        );

        let resp = self
            .client
            .raw_client()
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .send()
            .await?
            .error_for_status()?;

        let total_pages = resp
            .headers()
            .get("x-total-pages")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(page);

        let total_count = resp
            .headers()
            .get("x-total")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(total_pages * per_page);

        let items: Vec<Value> = resp.json().await?;

        let mrs: Vec<PrSummary> = items
            .iter()
            .map(|mr| {
                let state = mr["state"].as_str().unwrap_or("");
                let mr_state = match (state, mr["merged_at"].is_null()) {
                    ("merged", _) | (_, false) => PrState::Merged,
                    ("closed", _) => PrState::Closed,
                    _ => PrState::Open,
                };
                PrSummary {
                    number: mr["iid"].as_u64().unwrap_or(0),
                    title: mr["title"].as_str().unwrap_or("").to_string(),
                    author: Self::map_user(&mr["author"]),
                    status: matches!(mr_state, PrState::Open).then(|| Self::inbox_status(mr)),
                    state: mr_state,
                    created_at: mr["created_at"].as_str().unwrap_or("").to_string(),
                    updated_at: mr["updated_at"].as_str().unwrap_or("").to_string(),
                    labels: mr["labels"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|l| l.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                }
            })
            .collect();

        Ok(Paginated { items: mrs, page, total_pages, total_count })
    }

    async fn list_review_inbox(
        &self,
        category: &ReviewInboxCategory,
        page: u32,
        per_page: u32,
    ) -> Result<Paginated<ReviewInboxItem>, AppError> {
        let user = self.current_user().await?;
        let user_id = user.id.as_u64().ok_or_else(|| AppError::Api("GitLab 当前用户响应缺少数字 id".into()))?;
        let raw_items = match category {
            ReviewInboxCategory::ReviewRequested => {
                let (review_requests, assignments) = tokio::try_join!(
                    self.list_all_inbox_merge_requests("reviewer_id", user_id),
                    self.list_all_inbox_merge_requests("assignee_id", user_id),
                )?;
                review_requests
                    .into_iter()
                    .map(|item| (item, ReviewInboxRelationship::Reviewer))
                    .chain(assignments.into_iter().map(|item| (item, ReviewInboxRelationship::Assignee)))
                    .collect::<Vec<_>>()
            }
            ReviewInboxCategory::Authored => self
                .list_all_inbox_merge_requests("author_id", user_id)
                .await?
                .into_iter()
                .map(|item| (item, ReviewInboxRelationship::Author))
                .collect(),
        };

        let mut items = Vec::with_capacity(raw_items.len());
        for (mr, relationship) in raw_items {
            let (owner, repo) = Self::repository_from_merge_request(&mr)?;
            items.push(ReviewInboxItem {
                platform: self.name().to_string(),
                repository_full_name: format!("{owner}/{repo}"),
                owner,
                repo,
                categories: vec![*category],
                relationships: vec![relationship],
                status: Self::inbox_status(&mr),
                head_sha: mr["sha"].as_str().or_else(|| mr["diff_refs"]["head_sha"].as_str()).map(str::to_string),
                comments_count: mr["user_notes_count"].as_u64(),
                summary: PrSummary {
                    number: mr["iid"].as_u64().unwrap_or(0),
                    title: mr["title"].as_str().unwrap_or("").to_string(),
                    author: Self::map_user(&mr["author"]),
                    state: PrState::Open,
                    created_at: mr["created_at"].as_str().unwrap_or("").to_string(),
                    updated_at: mr["updated_at"].as_str().unwrap_or("").to_string(),
                    labels: mr["labels"]
                        .as_array()
                        .map(|labels| labels.iter().filter_map(|label| label.as_str().map(str::to_string)).collect())
                        .unwrap_or_default(),
                    status: None,
                },
            });
        }
        let mut items = super::merge_review_inbox_items(items);
        items.sort_by(|left, right| right.summary.updated_at.cmp(&left.summary.updated_at));

        let total_count = items.len() as u32;
        let total_pages = if total_count == 0 { 1 } else { total_count.div_ceil(per_page) };
        let start = page.saturating_sub(1).saturating_mul(per_page) as usize;
        let page_items = items.into_iter().skip(start).take(per_page as usize).collect();

        Ok(Paginated { items: page_items, page, total_pages, total_count })
    }

    async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrDetail, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}", self.base_url, project_id, pr_number);
        let json = self.get_json::<Value>(&url).await?;

        let summary = PrSummary {
            number: json["iid"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["author"]),
            state: match (json["state"].as_str().unwrap_or(""), json["merged_at"].is_null()) {
                (_, false) => PrState::Merged,
                ("closed", _) => PrState::Closed,
                _ => PrState::Open,
            },
            created_at: json["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: json["updated_at"].as_str().unwrap_or("").to_string(),
            labels: json["labels"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|l| l.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            status: None,
        };

        let metadata_permissions = self.metadata_permissions(owner, repo, &summary.author.login).await;
        Ok(PrDetail {
            summary,
            body: json["description"].as_str().unwrap_or("").to_string(),
            source_branch: json["source_branch"].as_str().unwrap_or("").to_string(),
            target_branch: json["target_branch"].as_str().unwrap_or("").to_string(),
            mergeable: None,
            head_sha: json["sha"].as_str().or_else(|| json["diff_refs"]["head_sha"].as_str()).unwrap_or("").to_string(),
            base_sha: json["diff_refs"]["base_sha"].as_str().unwrap_or("").to_string(),
            draft: json["draft"].as_bool().or_else(|| json["work_in_progress"].as_bool()),
            reviewers: json["reviewers"]
                .as_array()
                .map(|users| users.iter().map(Self::map_user).collect())
                .unwrap_or_default(),
            assignees: json["assignees"]
                .as_array()
                .map(|users| users.iter().map(Self::map_user).collect())
                .unwrap_or_default(),
            milestone: Self::metadata_milestone(&json["milestone"]),
            metadata_permissions,
        })
    }

    async fn list_pr_dependency_candidates(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrDependencyCandidates, AppError> {
        let project_id = urlencoding(owner, repo);
        let fallback_repository = format!("{owner}/{repo}");
        let map_candidate = |mr: &Value| {
            let number = mr["iid"].as_u64()?;
            let source_branch = mr["source_branch"].as_str()?.trim().to_string();
            let target_branch = mr["target_branch"].as_str()?.trim().to_string();
            if source_branch.is_empty() || target_branch.is_empty() {
                return None;
            }
            let source_repository = mr["source_project_id"]
                .as_u64()
                .map(|id| format!("gitlab-project:{id}"))
                .unwrap_or_else(|| format!("gitlab-unknown-source:{number}"));
            let target_repository = mr["target_project_id"]
                .as_u64()
                .map(|id| format!("gitlab-project:{id}"))
                .unwrap_or_else(|| fallback_repository.clone());
            let state = match (mr["state"].as_str().unwrap_or(""), mr["merged_at"].is_null()) {
                ("merged", _) | (_, false) => PrState::Merged,
                ("closed", _) => PrState::Closed,
                _ => PrState::Open,
            };
            Some(PrDependencyCandidate {
                number,
                title: mr["title"].as_str().unwrap_or("").to_string(),
                state,
                source_branch,
                target_branch,
                source_repository,
                target_repository,
            })
        };

        let current_url = format!("{}/projects/{project_id}/merge_requests/{pr_number}", self.base_url);
        let current_json = self.get_json::<Value>(&current_url).await?;
        let current =
            map_candidate(&current_json).ok_or_else(|| AppError::Api("当前 MR 缺少依赖分析所需的分支信息".into()))?;
        super::walk_pr_dependency_candidates(self, current, map_candidate, |candidate| {
            [
                format!(
                    "{}/projects/{project_id}/merge_requests?state=all&target_branch={}",
                    self.base_url,
                    urlencoding::encode(&candidate.source_branch)
                ),
                format!(
                    "{}/projects/{project_id}/merge_requests?state=all&source_branch={}",
                    self.base_url,
                    urlencoding::encode(&candidate.target_branch)
                ),
            ]
        })
        .await
    }

    async fn get_pr_merge_queue_status(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrMergeQueueStatus, AppError> {
        const PAGE_SIZE: usize = 100;
        const MAX_PAGES: u32 = 1_000;

        let project_id = urlencoding(owner, repo);
        let merge_request_url = format!("{}/projects/{project_id}/merge_requests/{pr_number}", self.base_url);
        let merge_request = self.get_json::<Value>(&merge_request_url).await?;
        let target_branch = merge_request["target_branch"]
            .as_str()
            .filter(|branch| !branch.trim().is_empty())
            .ok_or_else(|| AppError::Api("GitLab MR 响应缺少目标分支".into()))?
            .to_string();
        let current_head_sha = merge_request["sha"]
            .as_str()
            .or_else(|| merge_request["diff_refs"]["head_sha"].as_str())
            .map(str::to_string);

        let mut page = 1_u32;
        let mut preceding = 0_u32;
        loop {
            let url = format!(
                "{}/projects/{project_id}/merge_trains/{}?scope=active&sort=asc&per_page={PAGE_SIZE}&page={page}",
                self.base_url,
                urlencoding::encode(&target_branch)
            );
            let Some(response) = self.merge_train_response(&url).await? else {
                let mut status = PrMergeQueueStatus::unavailable(
                    MergeQueueKind::MergeTrain,
                    "当前 GitLab 实例、许可证或 Token 权限未提供 Merge Train API",
                );
                status.target_branch = Some(target_branch);
                status.head_sha = current_head_sha;
                return Ok(status);
            };
            let next_page_header = response.headers().get("x-next-page");
            let next_page = next_page_header
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.is_empty())
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|next| *next > page);
            let total_pages = response
                .headers()
                .get("x-total-pages")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u32>().ok());
            let total = response
                .headers()
                .get("x-total")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u32>().ok());
            let pagination_known = next_page_header.is_some() || total_pages.is_some() || total.is_some();
            let entries: Vec<Value> = response.json().await?;
            if let Some((index, entry)) =
                entries.iter().enumerate().find(|(_, entry)| entry["merge_request"]["iid"].as_u64() == Some(pr_number))
            {
                let position = preceding
                    .checked_add(u32::try_from(index).unwrap_or(u32::MAX))
                    .and_then(|value| value.checked_add(1));
                let mut status = Self::map_merge_train_entry(entry, position, total);
                if status.target_branch.is_none() {
                    status.target_branch = Some(target_branch);
                }
                if status.head_sha.is_none() {
                    status.head_sha = current_head_sha;
                }
                return Ok(status);
            }
            let fetched = entries.len();
            preceding = preceding.saturating_add(u32::try_from(fetched).unwrap_or(u32::MAX));
            let header_next = next_page
                .or_else(|| total_pages.filter(|total| page < *total).map(|_| page + 1))
                .or_else(|| total.filter(|total| preceding < *total).map(|_| page + 1));
            // Without any pagination metadata, a full page is ambiguous. Fetch the next page
            // so a proxy that strips headers cannot hide a second page of merge-train entries.
            let inferred_next = (!pagination_known && fetched == PAGE_SIZE).then(|| page.saturating_add(1));
            let Some(next) = header_next.or(inferred_next) else {
                break;
            };
            if next <= page || next > MAX_PAGES {
                return Err(AppError::Api("GitLab Merge Train 分页超过安全上限".into()));
            }
            page = next;
        }

        let history_url = format!("{}/projects/{project_id}/merge_trains/merge_requests/{pr_number}", self.base_url);
        let Some(response) = self.merge_train_response(&history_url).await? else {
            return Ok(PrMergeQueueStatus {
                kind: MergeQueueKind::MergeTrain,
                available: true,
                state: MergeQueueState::NotQueued,
                position: None,
                total: None,
                target_branch: Some(target_branch),
                enqueued_at: None,
                updated_at: None,
                estimated_time_seconds: None,
                head_sha: current_head_sha,
                pipeline_status: None,
                failure_reason: None,
            });
        };
        let entry: Value = response.json().await?;
        let mut status = Self::map_merge_train_entry(&entry, None, None);
        if status.target_branch.is_none() {
            status.target_branch = Some(target_branch);
        }
        if status.head_sha.is_none() {
            status.head_sha = current_head_sha;
        }
        Ok(status)
    }

    async fn list_branches(&self, owner: &str, repo: &str) -> Result<PrBranchOptions, AppError> {
        let project_id = urlencoding(owner, repo);
        let endpoint = format!("{}/projects/{}/repository/branches", self.base_url, project_id);
        let items = super::collect_json_pages(self, &endpoint).await?;
        let branches = items.iter().filter_map(|branch| branch["name"].as_str().map(str::to_string)).collect();
        let project_url = format!("{}/projects/{}", self.base_url, project_id);
        let project = self.get_json::<Value>(&project_url).await?;
        let default_branch = project["default_branch"].as_str().map(str::to_string);
        Ok(PrBranchOptions { branches, default_branch })
    }

    async fn list_labels(&self, owner: &str, repo: &str) -> Result<Vec<PrLabel>, AppError> {
        let project_id = urlencoding(owner, repo);
        let endpoint = format!("{}/projects/{}/labels", self.base_url, project_id);
        let items = super::collect_json_pages(self, &endpoint).await?;
        Ok(items
            .iter()
            .filter_map(|label| {
                label["name"].as_str().map(|name| PrLabel {
                    name: name.to_string(),
                    color: label["color"].as_str().map(str::to_string),
                    description: label["description"].as_str().map(str::to_string),
                })
            })
            .collect())
    }

    async fn list_pr_participant_suggestions(&self, owner: &str, repo: &str) -> Result<Vec<User>, AppError> {
        let project_id = urlencoding(owner, repo);
        let endpoint = format!("{}/projects/{}/members/all", self.base_url, project_id);
        let items = super::collect_json_pages(self, &endpoint).await?;
        Ok(items.iter().map(Self::map_user).filter(|user| !user.login.is_empty()).collect())
    }

    async fn create_pull_request(&self, owner: &str, repo: &str, request: &PrCreateRequest) -> Result<u64, AppError> {
        let target_project_path = urlencoding(owner, repo);
        let source_is_target = request.source_owner == owner && request.source_repo == repo;
        let endpoint_project_path = if source_is_target {
            target_project_path.clone()
        } else {
            urlencoding(&request.source_owner, &request.source_repo)
        };
        let url = format!("{}/projects/{}/merge_requests", self.base_url, endpoint_project_path);
        let mut payload = serde_json::json!({
            "source_branch": request.source_branch,
            "target_branch": request.target_branch,
            "title": Self::title_for_draft(&request.title, Some(request.draft)),
            "description": request.body,
        });
        if !source_is_target {
            let target_url = format!("{}/projects/{}", self.base_url, target_project_path);
            let target_project = self.get_json::<Value>(&target_url).await?;
            let target_id =
                target_project["id"].as_u64().ok_or_else(|| AppError::Api("GitLab Fork 目标项目缺少项目 ID".into()))?;
            payload["target_project_id"] = serde_json::json!(target_id);
        }
        let json = self.post_json(&url, &payload).await?;
        json["iid"].as_u64().ok_or_else(|| AppError::Api("GitLab 创建 MR 后未返回编号".into()))
    }

    async fn preview_pull_request(
        &self,
        owner: &str,
        repo: &str,
        request: &PrCreatePreviewRequest,
    ) -> Result<PrCreatePreviewData, AppError> {
        let source_project = urlencoding(&request.source_owner, &request.source_repo);
        if let Some(commit_sha) = request.commit_sha.as_deref() {
            let sha = urlencoding::encode(commit_sha);
            let commit_url = format!("{}/projects/{}/repository/commits/{}", self.base_url, source_project, sha);
            let (commit, (changes, incomplete)) =
                tokio::try_join!(self.get_json::<Value>(&commit_url), self.list_commit_diffs(&source_project, &sha))?;
            let files = changes
                .iter()
                .map(|change| PrFile {
                    filename: change["new_path"]
                        .as_str()
                        .or_else(|| change["old_path"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    status: if change["new_file"].as_bool() == Some(true) {
                        FileStatus::Added
                    } else if change["deleted_file"].as_bool() == Some(true) {
                        FileStatus::Removed
                    } else if change["renamed_file"].as_bool() == Some(true) {
                        FileStatus::Renamed
                    } else {
                        FileStatus::Modified
                    },
                    patch: change["diff"].as_str().unwrap_or("").to_string(),
                    additions: change["additions"].as_u64().unwrap_or(0) as u32,
                    deletions: change["deletions"].as_u64().unwrap_or(0) as u32,
                })
                .collect::<Vec<_>>();
            let diff = changes.iter().map(Self::unified_diff).filter(|patch| !patch.is_empty()).collect();
            let summary = PrCommitSummary {
                sha: commit["id"].as_str().unwrap_or(commit_sha).to_string(),
                title: commit["title"]
                    .as_str()
                    .or_else(|| commit["message"].as_str().and_then(|message| message.lines().next()))
                    .unwrap_or("")
                    .to_string(),
                author_name: commit["author_name"].as_str().unwrap_or("").to_string(),
                authored_at: commit["authored_date"]
                    .as_str()
                    .or_else(|| commit["created_at"].as_str())
                    .unwrap_or("")
                    .to_string(),
            };
            return Ok(PrCreatePreviewData {
                commits: vec![summary],
                diff,
                files,
                incomplete,
                incomplete_reasons: if incomplete {
                    vec![PrCreatePreviewIncompleteReason::PlatformLimit]
                } else {
                    vec![]
                },
            });
        }
        let from = urlencoding::encode(&request.target_branch);
        let to = urlencoding::encode(&request.source_branch);
        let mut url = format!(
            "{}/projects/{}/repository/compare?from={}&to={}&straight=true",
            self.base_url, source_project, from, to
        );
        if request.source_owner != owner || request.source_repo != repo {
            let target_project = urlencoding(owner, repo);
            let target_url = format!("{}/projects/{}", self.base_url, target_project);
            let target = self.get_json::<Value>(&target_url).await?;
            let target_id =
                target["id"].as_u64().ok_or_else(|| AppError::Api("GitLab Fork 目标项目缺少项目 ID".into()))?;
            url.push_str(&format!("&from_project_id={target_id}"));
        }
        let json = self.get_json::<Value>(&url).await?;
        if json["compare_same_ref"].as_bool() == Some(true) {
            return Err(AppError::Api("GitLab compare 返回相同分支，无法生成创建预览".into()));
        }
        let changes =
            json["diffs"].as_array().ok_or_else(|| AppError::Api("GitLab compare 响应缺少 diffs 字段".into()))?;
        let commits_json = json["commits"].as_array();
        let incomplete = json["compare_timeout"].as_bool() == Some(true)
            || json["overflow"].as_bool() == Some(true)
            || changes.iter().any(|change| {
                change["collapsed"].as_bool() == Some(true) || change["too_large"].as_bool() == Some(true)
            })
            || super::create_compare_is_incomplete(&json, commits_json.map_or(0, Vec::len), changes.len());
        let files = changes
            .iter()
            .map(|change| PrFile {
                filename: change["new_path"].as_str().or_else(|| change["old_path"].as_str()).unwrap_or("").to_string(),
                status: if change["new_file"].as_bool() == Some(true) {
                    FileStatus::Added
                } else if change["deleted_file"].as_bool() == Some(true) {
                    FileStatus::Removed
                } else if change["renamed_file"].as_bool() == Some(true) {
                    FileStatus::Renamed
                } else {
                    FileStatus::Modified
                },
                patch: change["diff"].as_str().unwrap_or("").to_string(),
                additions: change["additions"].as_u64().unwrap_or(0) as u32,
                deletions: change["deletions"].as_u64().unwrap_or(0) as u32,
            })
            .collect::<Vec<_>>();
        let diff = changes.iter().map(Self::unified_diff).filter(|patch| !patch.is_empty()).collect();
        let commits = commits_json
            .map(|items| {
                items
                    .iter()
                    .filter_map(|commit| {
                        Some(PrCommitSummary {
                            sha: commit["id"].as_str()?.to_string(),
                            title: commit["title"]
                                .as_str()
                                .or_else(|| commit["message"].as_str().and_then(|message| message.lines().next()))
                                .unwrap_or("")
                                .to_string(),
                            author_name: commit["author_name"].as_str().unwrap_or("").to_string(),
                            authored_at: commit["authored_date"]
                                .as_str()
                                .or_else(|| commit["created_at"].as_str())
                                .unwrap_or("")
                                .to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(PrCreatePreviewData {
            commits,
            diff,
            files,
            incomplete,
            incomplete_reasons: if incomplete { vec![PrCreatePreviewIncompleteReason::PlatformLimit] } else { vec![] },
        })
    }

    async fn update_pull_request_metadata(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        current: &PrDetail,
        update: &PrMetadataUpdate,
    ) -> Result<PrMetadataMutationResult, AppError> {
        let mut result = PrMetadataMutationResult::default();
        let mut payload = serde_json::Map::new();
        let title_body_changed = current.summary.title != update.title || current.body != update.body;
        let draft_changed = update.draft.is_some() && current.draft != update.draft;
        if title_body_changed || draft_changed {
            payload.insert(
                "title".into(),
                Value::String(Self::title_for_draft(&update.title, update.draft.or(current.draft))),
            );
            payload.insert("description".into(), Value::String(update.body.clone()));
        }

        let current_reviewers =
            current.reviewers.iter().map(|user| user.login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let target_reviewers =
            update.reviewers.iter().map(|login| login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let reviewers_changed = current_reviewers != target_reviewers;
        if reviewers_changed {
            match self.resolve_user_ids(&update.reviewers).await {
                Ok(ids) => {
                    payload.insert("reviewer_ids".into(), serde_json::json!(ids));
                }
                Err(error) => result
                    .failures
                    .push(PrMetadataUpdateFailure { field: PrMetadataField::Reviewers, message: error.to_string() }),
            }
        }

        let current_assignees =
            current.assignees.iter().map(|user| user.login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let target_assignees =
            update.assignees.iter().map(|login| login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let assignees_changed = current_assignees != target_assignees;
        if assignees_changed {
            match self.resolve_user_ids(&update.assignees).await {
                Ok(ids) => {
                    payload.insert("assignee_ids".into(), serde_json::json!(ids));
                }
                Err(error) => result
                    .failures
                    .push(PrMetadataUpdateFailure { field: PrMetadataField::Assignees, message: error.to_string() }),
            }
        }

        let labels_changed =
            current.summary.labels.iter().map(|label| label.to_lowercase()).collect::<std::collections::BTreeSet<_>>()
                != update.labels.iter().map(|label| label.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        if labels_changed {
            payload.insert("labels".into(), Value::String(update.labels.join(",")));
        }

        let milestone_changed =
            current.milestone.as_ref().map(|milestone| milestone.title.as_str()) != update.milestone.as_deref();
        if milestone_changed {
            match update.milestone.as_deref() {
                Some(title) => match self.resolve_milestone_id(owner, repo, title).await {
                    Ok(id) => {
                        payload.insert("milestone_id".into(), serde_json::json!(id));
                    }
                    Err(error) => result.failures.push(PrMetadataUpdateFailure {
                        field: PrMetadataField::Milestone,
                        message: error.to_string(),
                    }),
                },
                None => {
                    payload.insert("milestone_id".into(), serde_json::json!(0));
                }
            }
        }

        if payload.is_empty() {
            return Ok(result);
        }
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}", self.base_url, project_id, pr_number);
        match self.put_json(&url, &Value::Object(payload)).await {
            Ok(_) => {
                if title_body_changed {
                    result.updated_fields.push(PrMetadataField::TitleBody);
                }
                if draft_changed {
                    result.updated_fields.push(PrMetadataField::Draft);
                }
                if reviewers_changed
                    && !result.failures.iter().any(|failure| failure.field == PrMetadataField::Reviewers)
                {
                    result.updated_fields.push(PrMetadataField::Reviewers);
                }
                if assignees_changed
                    && !result.failures.iter().any(|failure| failure.field == PrMetadataField::Assignees)
                {
                    result.updated_fields.push(PrMetadataField::Assignees);
                }
                if labels_changed {
                    result.updated_fields.push(PrMetadataField::Labels);
                }
                if milestone_changed
                    && !result.failures.iter().any(|failure| failure.field == PrMetadataField::Milestone)
                {
                    result.updated_fields.push(PrMetadataField::Milestone);
                }
            }
            Err(error) => {
                for field in [
                    title_body_changed.then_some(PrMetadataField::TitleBody),
                    draft_changed.then_some(PrMetadataField::Draft),
                    (reviewers_changed
                        && !result.failures.iter().any(|failure| failure.field == PrMetadataField::Reviewers))
                    .then_some(PrMetadataField::Reviewers),
                    (assignees_changed
                        && !result.failures.iter().any(|failure| failure.field == PrMetadataField::Assignees))
                    .then_some(PrMetadataField::Assignees),
                    labels_changed.then_some(PrMetadataField::Labels),
                    (milestone_changed
                        && !result.failures.iter().any(|failure| failure.field == PrMetadataField::Milestone))
                    .then_some(PrMetadataField::Milestone),
                ]
                .into_iter()
                .flatten()
                {
                    result.failures.push(PrMetadataUpdateFailure { field, message: error.to_string() });
                }
            }
        }
        Ok(result)
    }

    async fn get_merge_readiness(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrMergeReadiness, AppError> {
        let project_id = urlencoding(owner, repo);
        let mr_url = format!("{}/projects/{}/merge_requests/{}", self.base_url, project_id, pr_number);
        let mr = self.get_json::<Value>(&mr_url).await?;
        let head_sha = mr["sha"].as_str().or_else(|| mr["diff_refs"]["head_sha"].as_str()).unwrap_or("").to_string();
        let draft = mr["draft"].as_bool().or_else(|| mr["work_in_progress"].as_bool());
        let merge_status = mr["detailed_merge_status"].as_str().or_else(|| mr["merge_status"].as_str()).unwrap_or("");
        let has_conflicts = mr["has_conflicts"].as_bool().or(match merge_status {
            "conflict" | "cannot_be_merged" => Some(true),
            "mergeable" | "can_be_merged" => Some(false),
            _ => None,
        });
        let branch_behind = matches!(merge_status, "need_rebase" | "behind").then_some(true);
        let has_merge_permission = mr["user"]["can_merge"].as_bool();
        let mut reasons = Vec::new();
        if mr["state"].as_str() != Some("opened") {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::NotOpen,
                message: "MR 不是打开状态".into(),
            });
        }
        if draft == Some(true) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Draft,
                message: "MR 仍处于 Draft 状态".into(),
            });
        }
        if has_conflicts == Some(true) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Conflicts,
                message: "源分支存在合并冲突".into(),
            });
        }
        if branch_behind == Some(true) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::BranchBehind,
                message: "源分支需要 Rebase".into(),
            });
        }
        if merge_status == "discussions_not_resolved" || mr["blocking_discussions_resolved"].as_bool() == Some(false) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::DiscussionsUnresolved,
                message: "仍有未解决的阻塞讨论".into(),
            });
        }

        if merge_status == "requested_changes" {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::ChangesRequested,
                message: "已有评审请求修改".into(),
            });
        }
        if merge_status == "not_approved" {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::ApprovalsRequired,
                message: "MR 尚未满足审批要求".into(),
            });
        }
        if matches!(merge_status, "blocked_status" | "broken_status") {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::PlatformBlocked,
                message: "GitLab 合并规则阻止当前 MR 合并".into(),
            });
        }
        if has_merge_permission == Some(false) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::NoMergePermission,
                message: "当前账号没有该 MR 的合并权限".into(),
            });
        }

        let pipelines_url =
            format!("{}/projects/{}/merge_requests/{}/pipelines?per_page=1", self.base_url, project_id, pr_number);
        let checks_status = match self.get_json::<Vec<Value>>(&pipelines_url).await {
            Ok(pipelines) if pipelines.is_empty() => {
                if matches!(merge_status, "mergeable" | "can_be_merged") {
                    // GitLab 已确认 MR 可合并，且没有关联 Pipeline，表示当前项目没有需要等待的 CI。
                    ReadinessState::Ready
                } else {
                    ReadinessState::Unknown
                }
            }
            Ok(pipelines) => match pipelines.first().and_then(|pipeline| pipeline["status"].as_str()) {
                Some("success") => ReadinessState::Ready,
                Some("failed") | Some("canceled") | Some("canceling") | Some("skipped") => {
                    reasons.push(MergeBlockingReason {
                        code: MergeBlockingReasonCode::ChecksFailed,
                        message: "Pipeline 未通过".into(),
                    });
                    ReadinessState::Blocked
                }
                Some("pending")
                | Some("running")
                | Some("created")
                | Some("waiting_for_resource")
                | Some("preparing") => {
                    reasons.push(MergeBlockingReason {
                        code: MergeBlockingReasonCode::ChecksPending,
                        message: "Pipeline 仍在进行中".into(),
                    });
                    ReadinessState::Pending
                }
                Some(_) => ReadinessState::Unknown,
                None => ReadinessState::Unknown,
            },
            Err(_) => ReadinessState::Unknown,
        };

        let checks_status = if checks_status == ReadinessState::Unknown {
            match merge_status {
                "ci_still_running" | "pipelines_must_succeed" => ReadinessState::Pending,
                "broken_status" => ReadinessState::Blocked,
                _ => checks_status,
            }
        } else {
            checks_status
        };
        if checks_status == ReadinessState::Pending
            && !reasons.iter().any(|reason| reason.code == MergeBlockingReasonCode::ChecksPending)
        {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::ChecksPending,
                message: "Pipeline 仍在进行中".into(),
            });
        }

        let approvals_url = format!("{}/projects/{}/merge_requests/{}/approvals", self.base_url, project_id, pr_number);
        let (approvals_status, approvals_required, approvals_received) =
            match self.get_json::<Value>(&approvals_url).await {
                Ok(approvals) => {
                    let required = approvals["approvals_required"].as_u64().map(|n| n as u32);
                    let received = approvals["approved_by"].as_array().map(|items| items.len() as u32);
                    let left = approvals["approvals_left"].as_u64();
                    if left.unwrap_or(0) > 0 || (required.is_some() && received.unwrap_or(0) < required.unwrap_or(0)) {
                        reasons.push(MergeBlockingReason {
                            code: MergeBlockingReasonCode::ApprovalsRequired,
                            message: format!(
                                "还需要 {} 个审批",
                                left.unwrap_or_else(|| u64::from(
                                    required.unwrap_or(0).saturating_sub(received.unwrap_or(0))
                                ))
                            ),
                        });
                        (ReadinessState::Blocked, required, received)
                    } else {
                        (ReadinessState::Ready, required, received)
                    }
                }
                Err(_) => (ReadinessState::Unknown, None, None),
            };

        let mergeable = match merge_status {
            "mergeable" | "can_be_merged" => Some(true),
            "conflict" | "cannot_be_merged" => Some(false),
            _ => None,
        };
        let has_hard_blocker = reasons.iter().any(|reason| reason.code != MergeBlockingReasonCode::ChecksPending);
        let status = if has_hard_blocker
            || checks_status == ReadinessState::Blocked
            || approvals_status == ReadinessState::Blocked
        {
            ReadinessState::Blocked
        } else if checks_status == ReadinessState::Pending {
            ReadinessState::Pending
        } else if mergeable == Some(true)
            && draft == Some(false)
            && has_conflicts == Some(false)
            && checks_status == ReadinessState::Ready
            && approvals_status == ReadinessState::Ready
            && has_merge_permission == Some(true)
        {
            ReadinessState::Ready
        } else {
            ReadinessState::Unknown
        };
        Ok(PrMergeReadiness {
            status,
            head_sha,
            mergeable,
            draft,
            has_conflicts,
            checks_status,
            approvals_status,
            approvals_required,
            approvals_received,
            has_merge_permission,
            branch_behind,
            blocking_reasons: reasons,
        })
    }

    async fn get_pr_diff(&self, owner: &str, repo: &str, pr_number: u64) -> Result<(String, Vec<PrFile>), AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}/changes", self.base_url, project_id, pr_number);
        let json = self.get_json::<Value>(&url).await?;

        let changes: Vec<PrFile> = json["changes"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|c| PrFile {
                        filename: c["new_path"].as_str().unwrap_or("").to_string(),
                        status: match c["new_file"].as_bool() {
                            Some(true) => FileStatus::Added,
                            _ => match c["deleted_file"].as_bool() {
                                Some(true) => FileStatus::Removed,
                                _ => match c["renamed_file"].as_bool() {
                                    Some(true) => FileStatus::Renamed,
                                    _ => FileStatus::Modified,
                                },
                            },
                        },
                        patch: c["diff"].as_str().unwrap_or("").to_string(),
                        additions: c["additions"].as_u64().unwrap_or(0) as u32,
                        deletions: c["deletions"].as_u64().unwrap_or(0) as u32,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // GitLab returns bare hunks in `changes[].diff`; diff2html requires file headers.
        let diff = json["changes"]
            .as_array()
            .map(|items| items.iter().map(Self::unified_diff).collect::<String>())
            .unwrap_or_default();

        Ok((diff, changes))
    }

    async fn get_compare_diff(
        &self,
        owner: &str,
        repo: &str,
        base_sha: &str,
        head_sha: &str,
    ) -> Result<(String, Vec<PrFile>), AppError> {
        let project_id = urlencoding(owner, repo);
        let from = urlencoding::encode(base_sha);
        let to = urlencoding::encode(head_sha);
        let url = format!(
            "{}/projects/{}/repository/compare?from={}&to={}&straight=true",
            self.base_url, project_id, from, to
        );
        let json = self.get_json::<Value>(&url).await?;
        if json["compare_timeout"].as_bool() == Some(true) {
            return Err(AppError::Api("GitLab compare 超时，无法可靠生成增量评审 Diff".into()));
        }
        if json["compare_same_ref"].as_bool() == Some(true) {
            return Err(AppError::Api("GitLab compare 返回相同提交版本，无法生成增量评审 Diff".into()));
        }
        let changes =
            json["diffs"].as_array().ok_or_else(|| AppError::Api("GitLab compare 响应缺少 diffs 字段".into()))?;
        let files: Vec<PrFile> = changes
            .iter()
            .map(|change| PrFile {
                filename: change["new_path"].as_str().or_else(|| change["old_path"].as_str()).unwrap_or("").to_string(),
                status: if change["new_file"].as_bool() == Some(true) {
                    FileStatus::Added
                } else if change["deleted_file"].as_bool() == Some(true) {
                    FileStatus::Removed
                } else if change["renamed_file"].as_bool() == Some(true) {
                    FileStatus::Renamed
                } else {
                    FileStatus::Modified
                },
                patch: change["diff"].as_str().unwrap_or("").to_string(),
                additions: change["additions"].as_u64().unwrap_or(0) as u32,
                deletions: change["deletions"].as_u64().unwrap_or(0) as u32,
            })
            .collect();
        let diff = changes.iter().map(Self::unified_diff).filter(|patch| !patch.is_empty()).collect();
        Ok((diff, files))
    }

    async fn get_pr_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        revision: &str,
    ) -> Result<PrFileContent, AppError> {
        crate::file_content::validate_request(path, revision)?;
        let project_id = urlencoding(owner, repo);
        let encoded_path = urlencoding::encode(path);
        let encoded_revision = urlencoding::encode(revision);
        let url = format!(
            "{}/projects/{}/repository/files/{}?ref={}",
            self.base_url, project_id, encoded_path, encoded_revision
        );
        let json = self.get_json::<Value>(&url).await?;
        crate::file_content::decode_response("GitLab", path, revision, &json)
    }

    async fn create_review(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        body: &str,
        event: &ReviewEvent,
        _comments: &[ReviewCommentPosition],
    ) -> Result<Review, AppError> {
        if !matches!(event, ReviewEvent::Comment) {
            return Err(AppError::NotImplemented("该平台仅支持评论评审".to_string()));
        }
        let project_id = urlencoding(owner, repo);

        // GitLab uses notes for reviews; approval API is separate
        let url = format!("{}/projects/{}/merge_requests/{}/notes", self.base_url, project_id, pr_number);
        let payload = serde_json::json!({
            "body": body,
        });

        let json = self.post_json(&url, &payload).await?;

        Ok(Review {
            id: json["id"].clone(),
            body: json["body"].as_str().unwrap_or("").to_string(),
            state: "commented".to_string(),
            author: Self::map_user(&json["author"]),
            submitted_at: json["created_at"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn create_pr_comment(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        commit_id: &str,
        path: &str,
        start_line: Option<u32>,
        line: u32,
        side: &str,
        body: &str,
    ) -> Result<PrComment, AppError> {
        if path.is_empty() || body.trim().is_empty() || line == 0 {
            return Err(AppError::Api("GitLab 行级评论的文件、行号和内容不能为空".into()));
        }
        if start_line.is_some_and(|start| start == 0 || start > line) {
            return Err(AppError::Api("GitLab 多行评论起始行无效".into()));
        }
        let (old_line, new_line, line_type) = Self::line_position(side, line)?;

        let project_id = urlencoding(owner, repo);
        let merge_request_url = format!("{}/projects/{project_id}/merge_requests/{pr_number}", self.base_url);
        let merge_request: Value = self.get_json(&merge_request_url).await?;
        let diff_refs = &merge_request["diff_refs"];
        let base_sha = Self::required_string(diff_refs, "base_sha", "MR diff refs")?;
        let start_sha = Self::required_string(diff_refs, "start_sha", "MR diff refs")?;
        let head_sha = Self::required_string(diff_refs, "head_sha", "MR diff refs")?;
        if commit_id != head_sha {
            return Err(AppError::Api("GitLab MR 已更新，请刷新 Diff 后重新评论".into()));
        }

        let mut position = serde_json::json!({
            "position_type": "text",
            "base_sha": base_sha,
            "start_sha": start_sha,
            "head_sha": head_sha,
            "old_path": path,
            "new_path": path,
        });
        if let Some(old_line) = old_line {
            position["old_line"] = old_line.into();
        }
        if let Some(new_line) = new_line {
            position["new_line"] = new_line.into();
        }
        if let Some(start_line) = start_line.filter(|start| *start != line) {
            let (start_old_line, start_new_line, _) = Self::line_position(side, start_line)?;
            position["line_range"] = serde_json::json!({
                "start": {
                    "line_code": Self::line_code(path, start_old_line, start_new_line),
                    "type": line_type,
                    "old_line": start_old_line,
                    "new_line": start_new_line,
                },
                "end": {
                    "line_code": Self::line_code(path, old_line, new_line),
                    "type": line_type,
                    "old_line": old_line,
                    "new_line": new_line,
                },
            });
        }

        let url = format!("{merge_request_url}/discussions");
        let discussion = self.post_json(&url, &serde_json::json!({ "body": body, "position": position })).await?;
        let thread_id = Self::value_id(&discussion["id"]);
        let note = discussion["notes"]
            .as_array()
            .and_then(|notes| notes.iter().find(|note| note["position"].is_object()))
            .ok_or_else(|| AppError::Api("GitLab 创建行级评论后未返回有效 Note".into()))?;
        Self::map_pr_comment(
            note,
            &thread_id,
            &note["position"],
            None,
            note["resolved"].as_bool(),
            note["resolvable"].as_bool().unwrap_or(false),
        )
        .ok_or_else(|| AppError::Api("GitLab 行级评论响应缺少位置信息".into()))
    }

    async fn list_pr_comments(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<PrComment>, AppError> {
        let project_id = urlencoding(owner, repo);
        let endpoint = format!("{}/projects/{project_id}/merge_requests/{pr_number}/discussions", self.base_url);
        let discussions = super::collect_json_pages(self, &endpoint).await?;
        let mut comments = Vec::new();
        for discussion in &discussions {
            let Some(notes) = discussion["notes"].as_array() else {
                continue;
            };
            let Some(root_note) =
                notes.iter().find(|note| !note["system"].as_bool().unwrap_or(false) && note["position"].is_object())
            else {
                continue;
            };
            let thread_id = Self::value_id(&discussion["id"]);
            let root_id = Self::value_id(&root_note["id"]);
            let resolved = notes.iter().find_map(|note| note["resolved"].as_bool());
            let resolvable = notes.iter().any(|note| note["resolvable"].as_bool().unwrap_or(false));
            for note in notes.iter().filter(|note| !note["system"].as_bool().unwrap_or(false)) {
                let note_id = Self::value_id(&note["id"]);
                let reply_to_id = (note_id != root_id).then(|| root_id.clone());
                if let Some(comment) =
                    Self::map_pr_comment(note, &thread_id, &root_note["position"], reply_to_id, resolved, resolvable)
                {
                    comments.push(comment);
                }
            }
        }
        Ok(comments)
    }

    async fn reply_to_review_thread(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        thread_id: &str,
        _reply_to_id: &str,
        body: &str,
    ) -> Result<(), AppError> {
        let project_id = urlencoding(owner, repo);
        let url =
            format!("{}/projects/{project_id}/merge_requests/{pr_number}/discussions/{thread_id}/notes", self.base_url);
        self.post_json(&url, &serde_json::json!({ "body": body })).await?;
        Ok(())
    }

    async fn update_review_comment(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        thread_id: &str,
        comment_id: &str,
        body: &str,
    ) -> Result<(), AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!(
            "{}/projects/{project_id}/merge_requests/{pr_number}/discussions/{thread_id}/notes/{comment_id}",
            self.base_url
        );
        self.put_json(&url, &serde_json::json!({ "body": body })).await?;
        Ok(())
    }

    async fn delete_review_comment(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        thread_id: &str,
        comment_id: &str,
    ) -> Result<(), AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!(
            "{}/projects/{project_id}/merge_requests/{pr_number}/discussions/{thread_id}/notes/{comment_id}",
            self.base_url
        );
        self.delete_empty(&url).await
    }

    async fn set_review_thread_resolved(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        thread_id: &str,
        resolved: bool,
    ) -> Result<(), AppError> {
        if thread_id.trim().is_empty() {
            return Err(AppError::Api("GitLab Discussion ID 不能为空".into()));
        }
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{project_id}/merge_requests/{pr_number}/discussions/{thread_id}", self.base_url);
        self.put_json(&url, &serde_json::json!({ "resolved": resolved })).await?;
        Ok(())
    }

    async fn list_reviews(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<Review>, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}/notes?per_page=100", self.base_url, project_id, pr_number);
        let items: Vec<Value> = self.get_json(&url).await?;

        let reviews = items
            .iter()
            .filter(|n| !n["system"].as_bool().unwrap_or(false))
            .filter(|n| !n["position"].is_object())
            .map(|n| Review {
                id: n["id"].clone(),
                body: n["body"].as_str().unwrap_or("").to_string(),
                state: "commented".to_string(),
                author: Self::map_user(&n["author"]),
                submitted_at: n["created_at"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(reviews)
    }

    async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        state: &IssueState,
        page: u32,
    ) -> Result<Paginated<IssueSummary>, AppError> {
        let project_id = urlencoding(owner, repo);
        let state_param = state.as_str();
        let url = format!(
            "{}/projects/{}/issues?state={}&per_page=100&page={}",
            self.base_url, project_id, state_param, page
        );
        let items: Vec<Value> = self.get_json(&url).await?;

        let issues = items
            .iter()
            .map(|i| IssueSummary {
                number: i["iid"].as_u64().unwrap_or(0),
                title: i["title"].as_str().unwrap_or("").to_string(),
                author: Self::map_user(&i["author"]),
                state: match i["state"].as_str().unwrap_or("") {
                    "closed" => IssueState::Closed,
                    _ => IssueState::Open,
                },
                labels: i["labels"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|l| l.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                created_at: i["created_at"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(Paginated { items: issues, page, total_pages: 1, total_count: 0 })
    }

    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<Issue, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/issues", self.base_url, project_id);
        let payload = serde_json::json!({
            "title": title,
            "description": body,
            "labels": labels.join(","),
        });

        let json = self.post_json(&url, &payload).await?;

        Ok(Issue {
            number: json["iid"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            body: json["description"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["author"]),
            state: match json["state"].as_str().unwrap_or("") {
                "closed" => IssueState::Closed,
                _ => IssueState::Open,
            },
            labels: json["labels"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|l| l.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            created_at: json["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: json["updated_at"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        strategy: &MergeStrategy,
        _commit_title: Option<String>,
        commit_message: Option<String>,
        sha: &str,
    ) -> Result<PrMergeResult, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}/merge", self.base_url, project_id, pr_number);
        let squash = matches!(strategy, MergeStrategy::Squash);
        let mut payload = serde_json::json!({
            "squash": squash,
            "sha": sha,
        });
        if let Some(m) = commit_message {
            payload["merge_commit_message"] = serde_json::Value::String(m);
        }
        let json = self.put_json(&url, &payload).await?;
        Ok(PrMergeResult { merged: true, sha: json["id"].as_str().unwrap_or("").to_string(), message: String::new() })
    }

    async fn close_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}", self.base_url, project_id, pr_number);
        let payload = serde_json::json!({ "state_event": "close" });
        self.put_json(&url, &payload).await?;
        Ok(PrState::Closed)
    }

    async fn reopen_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}", self.base_url, project_id, pr_number);
        let payload = serde_json::json!({ "state_event": "reopen" });
        self.put_json(&url, &payload).await?;
        Ok(PrState::Open)
    }

    async fn close_issue(&self, owner: &str, repo: &str, issue_number: u64) -> Result<(), AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/issues/{}", self.base_url, project_id, issue_number);
        let payload = serde_json::json!({ "state_event": "close" });
        self.put_json(&url, &payload).await?;
        Ok(())
    }
}

fn urlencoding(owner: &str, repo: &str) -> String {
    urlencoding::encode(&format!("{owner}/{repo}")).into_owned()
}
