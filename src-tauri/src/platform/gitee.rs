use async_trait::async_trait;
use futures::{stream, StreamExt};
use serde_json::Value;

use super::GitPlatform;
use crate::error::AppError;
use crate::http_client::HttpClient;
use crate::models::*;

/// Gitee adapter — Gitee API v5 is largely compatible with GitHub v3
pub struct GiteeAdapter {
    client: HttpClient,
    token: String,
    base_url: String,
}

impl GiteeAdapter {
    pub fn new(client: HttpClient, token: String) -> Self {
        Self { client, token, base_url: "https://gitee.com/api/v5".to_string() }
    }

    #[allow(dead_code)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = super::normalize_api_base("gitee", &url);
        self
    }

    /// Batch-fetch display names for organization groups and enterprises.
    /// Gitee repo responses include a `namespace` object, but `namespace.name` may
    /// equal `namespace.path` (the URL slug) when no custom display name is set.
    /// This function calls `/orgs/{path}` for groups and `/enterprises/{path}` for
    /// enterprises to resolve proper display names.
    async fn resolve_namespace_display_names(&self, repos: &mut [RepoSummary]) {
        let mut org_logins = std::collections::HashSet::new();
        let mut ent_logins = std::collections::HashSet::new();
        for r in repos.iter() {
            if r.owner_type == "organization" && r.owner_display_name == r.owner {
                org_logins.insert(r.owner.clone());
            } else if r.owner_type == "enterprise" && r.owner_display_name == r.owner {
                ent_logins.insert(r.owner.clone());
            }
        }

        let base = self.base_url.clone();
        let client = self.client.clone();
        let auth = self.auth_query();
        let mut name_map = std::collections::HashMap::new();

        let mut futs: Vec<tokio::task::JoinHandle<Option<(String, String)>>> = Vec::new();

        for login in org_logins {
            let url = format!("{}/orgs/{}", base, login);
            let sep = if url.contains('?') { "&" } else { "?" };
            let full_url = format!("{}{}{}", url, sep, auth);
            let c = client.clone();
            futs.push(tokio::spawn(async move {
                let resp = c.get(&full_url).header("User-Agent", "mergebeacon").send().await.ok()?;
                let json: Value = resp.json().await.ok()?;
                let name = json["name"].as_str()?.to_string();
                Some((login, name))
            }));
        }

        for login in ent_logins {
            let url = format!("{}/enterprises/{}", base, login);
            let sep = if url.contains('?') { "&" } else { "?" };
            let full_url = format!("{}{}{}", url, sep, auth);
            let c = client.clone();
            futs.push(tokio::spawn(async move {
                let resp = c.get(&full_url).header("User-Agent", "mergebeacon").send().await.ok()?;
                let json: Value = resp.json().await.ok()?;
                let name = json["name"].as_str()?.to_string();
                Some((login, name))
            }));
        }

        let results = futures::future::join_all(futs).await;
        for r in results {
            if let Ok(Some((login, name))) = r {
                name_map.insert(login, name);
            }
        }

        for repo in repos.iter_mut() {
            if let Some(name) = name_map.get(&repo.owner) {
                repo.owner_display_name = name.clone();
            }
        }
    }

    fn parse_last_page_gitee(link: Option<&str>, fallback: u32) -> u32 {
        let Some(header) = link else {
            return fallback;
        };
        for part in header.split(',') {
            let part = part.trim();
            if part.contains("rel=\"last\"") || part.contains("rel='last'") {
                if let Some(url_start) = part.find('<') {
                    let url_end = part[url_start..].find('>').unwrap_or(part.len() - url_start);
                    let url = &part[url_start + 1..url_start + url_end];
                    let query = url.split('?').nth(1).unwrap_or("");
                    for seg in query.split('&') {
                        if let Some(page_str) = seg.strip_prefix("page=") {
                            if let Ok(n) = page_str.parse::<u32>() {
                                return n;
                            }
                        }
                    }
                }
            }
        }
        fallback
    }

    fn auth_query(&self) -> String {
        format!("access_token={}", self.token)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, AppError> {
        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self.client.get(&full_url).header("User-Agent", "mergebeacon").send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    fn acceptance_progress(json: &Value, gates: &[(&str, &str)]) -> (Option<u32>, Option<u32>) {
        let mut required_total = 0_u32;
        let mut received_total = 0_u32;
        let mut has_requirement = false;
        let mut has_complete_participants = true;

        for (required_field, participants_field) in gates {
            let Some(required) = json[*required_field].as_u64().and_then(|value| u32::try_from(value).ok()) else {
                continue;
            };
            has_requirement = true;
            required_total = required_total.saturating_add(required);

            match json[*participants_field].as_array() {
                Some(participants) => {
                    let received = participants
                        .iter()
                        .filter(|participant| participant["accept"].as_bool() == Some(true))
                        .fold(0_u32, |count, _| count.saturating_add(1));
                    // Each gate has its own threshold. Extra acceptances in one gate must
                    // not compensate for a missing acceptance in another gate.
                    received_total = received_total.saturating_add(received.min(required));
                }
                None if required > 0 => has_complete_participants = false,
                None => {}
            }
        }

        if !has_requirement {
            (None, None)
        } else if has_complete_participants {
            (Some(required_total), Some(received_total))
        } else {
            (Some(required_total), None)
        }
    }

    fn inbox_acceptance_progress(json: &Value, gates: &[(&str, &str)]) -> (Option<u32>, Option<u32>) {
        let progress = Self::acceptance_progress(json, gates);
        if progress.0.is_some() {
            return progress;
        }

        let mut required = 0_u32;
        let mut received = 0_u32;
        let mut found = false;
        for (_, participants_field) in gates {
            let Some(participants) = json[*participants_field].as_array() else {
                continue;
            };
            if participants.is_empty() {
                continue;
            }
            found = true;
            required = required.saturating_add(participants.len() as u32);
            received = received.saturating_add(
                participants.iter().filter(|participant| participant["accept"].as_bool() == Some(true)).count() as u32,
            );
        }
        if found {
            (Some(required), Some(received))
        } else {
            (None, None)
        }
    }

    fn inbox_status(pr: &Value) -> ReviewInboxStatusSummary {
        let mergeable = pr["mergeable"].as_bool();
        let draft = pr["draft"].as_bool().or_else(|| pr["work_in_progress"].as_bool());
        let has_conflicts = pr["has_conflicts"]
            .as_bool()
            .or_else(|| pr["conflict"].as_bool())
            .or_else(|| (mergeable == Some(true)).then_some(false));
        let mut blocking_reasons = Vec::new();

        if draft == Some(true) {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Draft,
                message: "PR 仍处于 Draft 状态".into(),
            });
        }
        if has_conflicts == Some(true) {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Conflicts,
                message: "源分支存在合并冲突".into(),
            });
        }
        if mergeable == Some(false) && has_conflicts != Some(true) {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::PlatformBlocked,
                message: "平台报告当前 PR 不可合并".into(),
            });
        }

        let (approvals_required, approvals_received) = Self::inbox_acceptance_progress(
            pr,
            &[("assignees_number", "assignees"), ("api_reviewers_number", "api_reviewers")],
        );
        let approvals_status = match (approvals_required, approvals_received) {
            (Some(required), Some(received)) if received >= required => ReadinessState::Ready,
            (Some(required), Some(received)) => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ApprovalsRequired,
                    message: format!("还需要 {} 个审批", required.saturating_sub(received)),
                });
                ReadinessState::Blocked
            }
            _ => ReadinessState::Unknown,
        };

        let (tests_required, tests_received) = Self::inbox_acceptance_progress(pr, &[("testers_number", "testers")]);
        let checks_status = match (tests_required, tests_received) {
            (Some(required), Some(received)) if received >= required => ReadinessState::Ready,
            (Some(required), Some(received)) => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ChecksPending,
                    message: format!("还需要 {} 个测试通过", required.saturating_sub(received)),
                });
                ReadinessState::Pending
            }
            _ => ReadinessState::Unknown,
        };

        let has_hard_blocker =
            blocking_reasons.iter().any(|reason| reason.code != MergeBlockingReasonCode::ChecksPending);
        let status = if has_hard_blocker || approvals_status == ReadinessState::Blocked {
            ReadinessState::Blocked
        } else if checks_status == ReadinessState::Pending {
            ReadinessState::Pending
        } else if mergeable == Some(true)
            && approvals_status == ReadinessState::Ready
            && checks_status == ReadinessState::Ready
            && draft != Some(true)
            && has_conflicts != Some(true)
        {
            ReadinessState::Ready
        } else {
            ReadinessState::Unknown
        };

        ReviewInboxStatusSummary { status, draft, has_conflicts, checks_status, approvals_status, blocking_reasons }
    }

    fn inbox_relationship(filter_name: &str) -> ReviewInboxRelationship {
        match filter_name {
            "assignee" => ReviewInboxRelationship::Reviewer,
            "tester" => ReviewInboxRelationship::Tester,
            "author" => ReviewInboxRelationship::Author,
            _ => ReviewInboxRelationship::Reviewer,
        }
    }

    fn file_paths(value: &Value) -> (&str, &str) {
        let old_path = value["previous_filename"]
            .as_str()
            .or_else(|| value["old_path"].as_str())
            .or_else(|| value["patch"]["old_path"].as_str())
            .or_else(|| value["filename"].as_str())
            .unwrap_or("");
        let new_path = value["filename"].as_str().or_else(|| value["new_path"].as_str()).unwrap_or("");
        (old_path, new_path)
    }

    fn file_patch(value: &Value) -> String {
        let patch = value["patch"]["diff"].as_str().or_else(|| value["patch"].as_str()).unwrap_or("");
        if !patch.trim().is_empty() {
            return patch.to_string();
        }

        let (old_path, new_path) = Self::file_paths(value);
        let is_metadata_only_rename = value["status"].as_str() == Some("renamed")
            && value["collapsed"].as_bool() != Some(true)
            && value["too_large"].as_bool() != Some(true)
            && value["truncated"].as_bool() != Some(true)
            && value["additions"].as_u64().unwrap_or(0) == 0
            && value["deletions"].as_u64().unwrap_or(0) == 0
            && !old_path.is_empty()
            && !new_path.is_empty()
            && old_path != new_path;

        if is_metadata_only_rename {
            crate::patch::metadata_only_rename_patch(old_path, new_path)
        } else {
            String::new()
        }
    }

    fn unified_diff(value: &Value) -> String {
        let patch = Self::file_patch(value);
        if patch.is_empty() {
            return patch;
        }

        let mut diff = if patch.starts_with("diff --git ") {
            patch
        } else {
            let (old_path, new_path) = Self::file_paths(value);
            let old_marker = if value["status"].as_str() == Some("added") {
                "/dev/null".to_string()
            } else {
                format!("a/{old_path}")
            };
            let new_marker = if value["status"].as_str() == Some("removed") {
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

    fn metadata_milestone(json: &Value) -> Option<PrMilestone> {
        (!json.is_null()).then(|| PrMilestone {
            id: json["id"].clone(),
            number: json["number"].as_u64(),
            title: json["title"].as_str().unwrap_or("").to_string(),
        })
    }

    fn known_or(left: Option<bool>, right: Option<bool>) -> Option<bool> {
        match (left, right) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        }
    }

    async fn metadata_permissions(&self, owner: &str, repo: &str, author_login: &str) -> PrMetadataPermissions {
        let user_url = format!("{}/user", self.base_url);
        let repo_url = format!("{}/repos/{}/{}", self.base_url, owner, repo);
        let (user, repository) = tokio::join!(self.get_json::<Value>(&user_url), self.get_json::<Value>(&repo_url));
        let is_author =
            user.ok().and_then(|user| user["login"].as_str().map(|login| login.eq_ignore_ascii_case(author_login)));
        let can_write = repository.ok().and_then(|repository| Self::repository_merge_permission(&repository));
        let can_edit = Self::known_or(is_author, can_write);
        PrMetadataPermissions {
            can_edit_title_body: can_edit,
            can_toggle_draft: Some(false),
            can_manage_reviewers: can_edit,
            can_manage_assignees: can_edit,
            can_manage_labels: can_edit,
            can_manage_milestone: can_edit,
        }
    }

    async fn resolve_milestone_number(&self, owner: &str, repo: &str, title: &str) -> Result<u64, AppError> {
        let encoded = urlencoding::encode(title);
        let url =
            format!("{}/repos/{}/{}/milestones?state=all&per_page=100&title={}", self.base_url, owner, repo, encoded);
        let milestones = self.get_json::<Value>(&url).await?;
        milestones
            .as_array()
            .and_then(|items| {
                items.iter().find(|item| item["title"].as_str() == Some(title)).and_then(|item| item["number"].as_u64())
            })
            .ok_or_else(|| AppError::Api(format!("Gitee 仓库中不存在 Milestone：{title}")))
    }

    fn repository_merge_permission(value: &Value) -> Option<bool> {
        let permissions = value.get("permission").or_else(|| value.get("permissions"))?;
        let admin = permissions["admin"].as_bool();
        let push = permissions["push"].as_bool();

        if admin.is_none() && push.is_none() {
            None
        } else {
            Some(admin == Some(true) || push == Some(true))
        }
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self
            .client
            .post(&full_url)
            .header("User-Agent", "mergebeacon")
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        if resp.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        Ok(resp.json().await?)
    }

    async fn patch_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self
            .client
            .raw_client()
            .patch(&full_url)
            .header("User-Agent", "mergebeacon")
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn delete_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self
            .client
            .raw_client()
            .delete(&full_url)
            .header("User-Agent", "mergebeacon")
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        if resp.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        Ok(resp.json().await?)
    }

    fn repository_parts(full_name: &str) -> Result<(String, String), AppError> {
        let (owner, repo) = full_name
            .rsplit_once('/')
            .filter(|(owner, repo)| !owner.is_empty() && !repo.is_empty())
            .ok_or_else(|| AppError::Api("Gitee 收件箱响应中的仓库路径无效".into()))?;
        Ok((owner.to_string(), repo.to_string()))
    }

    fn matches_inbox_category(json: &Value, category: &ReviewInboxCategory, login: &str, filter_name: &str) -> bool {
        match category {
            ReviewInboxCategory::Authored => json["user"]["login"].as_str() == Some(login),
            ReviewInboxCategory::ReviewRequested => {
                let mut found_login = false;
                let mut pending = false;
                let mut found_other_reviewer = false;
                let responsibility_fields: &[&str] = match filter_name {
                    "tester" => &["testers"],
                    _ => &["assignees", "api_reviewers"],
                };
                for field in responsibility_fields {
                    if let Some(reviewers) = json[field].as_array() {
                        for reviewer in reviewers {
                            if reviewer["login"].as_str() == Some(login) {
                                found_login = true;
                                pending |= reviewer["accept"].as_bool() != Some(true);
                            } else {
                                found_other_reviewer = true;
                            }
                        }
                    }
                }
                if found_login {
                    pending
                } else {
                    // The repository endpoint's active responsibility filter is authoritative. Some Gitee
                    // responses omit reviewer/tester arrays, so retain those results; only reject
                    // explicit responsibility lists that do not contain the current user.
                    !found_other_reviewer
                }
            }
        }
    }

    fn inbox_api_error(status: reqwest::StatusCode, url: &str, body: &str) -> AppError {
        let detail = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|json| json["message"].as_str().map(str::to_string))
            .or_else(|| {
                let trimmed = body.trim();
                (!trimmed.is_empty() && !trimmed.starts_with('<'))
                    .then(|| trimmed.chars().take(240).collect::<String>())
            })
            .unwrap_or_else(|| "远端返回了非 JSON 错误页面".to_string());
        AppError::Api(format!("Gitee API {status} ({url}): {detail}"))
    }

    async fn send_inbox_request(&self, url: &str, query: &[(&str, String)]) -> Result<reqwest::Response, AppError> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", "mergebeacon")
            .query(query)
            .query(&[("access_token", &self.token)])
            .send()
            .await
            .map_err(|_| AppError::Api(format!("Gitee 收件箱请求失败（{url}）")))?;
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(Self::inbox_api_error(status, url, &body))
    }

    async fn list_inbox_repositories(&self) -> Result<Vec<(String, String)>, AppError> {
        const REMOTE_PAGE_SIZE: usize = 100;
        let url = format!("{}/user/repos", self.base_url);
        let mut page = 1_u32;
        let mut repositories = Vec::new();
        let mut seen = std::collections::HashSet::new();

        loop {
            let query = [
                ("visibility", "all".to_string()),
                ("sort", "updated".to_string()),
                ("direction", "desc".to_string()),
                ("page", page.to_string()),
                ("per_page", REMOTE_PAGE_SIZE.to_string()),
            ];
            let response = self.send_inbox_request(&url, &query).await?;
            let link_header = response.headers().get("link").and_then(|value| value.to_str().ok());
            let total_pages = response
                .headers()
                .get("x-total-pages")
                .or_else(|| response.headers().get("total_page"))
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or_else(|| Self::parse_last_page_gitee(link_header, page));
            let raw_repositories: Vec<Value> = response.json().await?;
            let fetched = raw_repositories.len();

            for repository in raw_repositories {
                let full_name = repository["full_name"]
                    .as_str()
                    .ok_or_else(|| AppError::Api("Gitee 收件箱仓库响应缺少 full_name".into()))?;
                let parts = Self::repository_parts(full_name)?;
                if seen.insert(full_name.to_string()) {
                    repositories.push(parts);
                }
            }

            if page >= total_pages && fetched < REMOTE_PAGE_SIZE {
                break;
            }
            page = page.saturating_add(1);
        }
        Ok(repositories)
    }

    async fn list_repository_inbox_by_filter(
        &self,
        owner: &str,
        repo: &str,
        category: ReviewInboxCategory,
        login: &str,
        filter_name: &str,
    ) -> Result<Vec<ReviewInboxItem>, AppError> {
        const REMOTE_PAGE_SIZE: usize = 100;
        let url = format!("{}/repos/{owner}/{repo}/pulls", self.base_url);
        let filter_value = login;
        let mut page = 1_u32;
        let mut items = Vec::new();

        loop {
            let query = [
                ("state", "open".to_string()),
                ("sort", "updated".to_string()),
                ("direction", "desc".to_string()),
                (filter_name, filter_value.to_string()),
                ("page", page.to_string()),
                ("per_page", REMOTE_PAGE_SIZE.to_string()),
            ];
            let response = self.send_inbox_request(&url, &query).await?;
            let link_header = response.headers().get("link").and_then(|value| value.to_str().ok());
            let total_pages = response
                .headers()
                .get("x-total-pages")
                .or_else(|| response.headers().get("total_page"))
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or_else(|| Self::parse_last_page_gitee(link_header, page));
            let raw_items: Vec<Value> = response.json().await?;
            let fetched = raw_items.len();

            for pr in raw_items.iter().filter(|pr| Self::matches_inbox_category(pr, &category, login, filter_name)) {
                items.push(ReviewInboxItem {
                    platform: self.name().to_string(),
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                    repository_full_name: format!("{owner}/{repo}"),
                    categories: vec![category],
                    relationships: vec![Self::inbox_relationship(filter_name)],
                    status: Self::inbox_status(pr),
                    head_sha: pr["head"]["sha"]
                        .as_str()
                        .or_else(|| pr["head"]["commit_id"].as_str())
                        .map(str::to_string),
                    comments_count: pr["comments_count"].as_u64().or_else(|| pr["comments"].as_u64()),
                    summary: PrSummary {
                        number: pr["number"].as_u64().unwrap_or(0),
                        title: pr["title"].as_str().unwrap_or("").to_string(),
                        author: Self::map_user(&pr["user"]),
                        state: PrState::Open,
                        created_at: pr["created_at"].as_str().unwrap_or("").to_string(),
                        updated_at: pr["updated_at"].as_str().unwrap_or("").to_string(),
                        labels: pr["labels"]
                            .as_array()
                            .map(|labels| {
                                labels.iter().filter_map(|label| label["name"].as_str().map(str::to_string)).collect()
                            })
                            .unwrap_or_default(),
                        status: None,
                    },
                });
            }

            if page >= total_pages && fetched < REMOTE_PAGE_SIZE {
                break;
            }
            page = page.saturating_add(1);
        }
        Ok(items)
    }

    async fn list_repository_inbox(
        &self,
        owner: &str,
        repo: &str,
        category: ReviewInboxCategory,
        login: &str,
    ) -> Result<Vec<ReviewInboxItem>, AppError> {
        match category {
            ReviewInboxCategory::ReviewRequested => {
                let (reviews, tests) = tokio::try_join!(
                    self.list_repository_inbox_by_filter(owner, repo, category, login, "assignee"),
                    self.list_repository_inbox_by_filter(owner, repo, category, login, "tester"),
                )?;
                let mut items = super::merge_review_inbox_items(reviews.into_iter().chain(tests).collect());
                items.sort_by(|left, right| right.summary.updated_at.cmp(&left.summary.updated_at));
                Ok(items)
            }
            ReviewInboxCategory::Authored => {
                self.list_repository_inbox_by_filter(owner, repo, category, login, "author").await
            }
        }
    }

    fn map_user(json: &Value) -> User {
        User {
            id: json["id"].clone(),
            login: json["login"].as_str().unwrap_or("").to_string(),
            name: json["name"].as_str().unwrap_or("").to_string(),
            avatar_url: json["avatar_url"].as_str().unwrap_or("").to_string(),
        }
    }
}
#[async_trait]
impl super::JsonPageSource for GiteeAdapter {
    async fn fetch_json_page(&self, endpoint: &str, page: u32) -> Result<super::JsonPage, AppError> {
        let url = super::json_page_url(endpoint, page);
        let authenticated_url = format!("{}&{}", url, self.auth_query());
        let response = self.client.get(&authenticated_url).header("User-Agent", "mergebeacon").send().await?;
        let status = response.status();
        let link = response.headers().get("link").and_then(|value| value.to_str().ok()).map(str::to_owned);
        let header_total_pages = response
            .headers()
            .get("x-total-pages")
            .or_else(|| response.headers().get("total_page"))
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok());
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("Gitee API {status} ({url}): {body}")));
        }
        let items = response.json().await?;
        let link_total_pages = link.as_deref().map(|value| Self::parse_last_page_gitee(Some(value), page));
        let total_pages = header_total_pages.or(link_total_pages);
        let next_page = super::next_page_from_link(link.as_deref())
            .or_else(|| total_pages.filter(|total| page < *total).map(|_| page.saturating_add(1)));
        Ok(super::JsonPage { items, next_page, pagination_known: header_total_pages.is_some() || link.is_some() })
    }
}

#[async_trait]
impl GitPlatform for GiteeAdapter {
    fn name(&self) -> &'static str {
        "gitee"
    }

    async fn current_user(&self) -> Result<User, AppError> {
        let url = format!("{}/user", self.base_url);
        let json = self.get_json::<Value>(&url).await?;
        Ok(Self::map_user(&json))
    }

    async fn list_repos(&self, page: u32) -> Result<Paginated<RepoSummary>, AppError> {
        let url = format!("{}/user/repos?per_page=100&page={}", self.base_url, page);

        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self.client.get(&full_url).header("User-Agent", "mergebeacon").send().await?;

        let link_header = resp.headers().get("link").and_then(|v| v.to_str().ok());
        let last_page = resp
            .headers()
            .get("x-total-pages")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or_else(|| Self::parse_last_page_gitee(link_header, page));
        let header_total_count = resp
            .headers()
            .get("x-total-count")
            .or_else(|| resp.headers().get("x-total"))
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok());

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("Gitee API {} ({}): {}", status, url, body)));
        }

        let items: Vec<Value> = resp.json().await?;
        let total_count = header_total_count.unwrap_or_else(|| {
            if page == last_page {
                page.saturating_sub(1) * 100 + items.len() as u32
            } else {
                last_page * 100
            }
        });

        let mut repos: Vec<RepoSummary> = items
            .iter()
            .map(|r| {
                let full_name = r["full_name"].as_str().unwrap_or("");
                let fork = r["fork"].as_bool().unwrap_or(false);
                let (parent_full_name, parent_owner) = if fork {
                    let pn = r["parent"]["full_name"].as_str().map(|s| s.to_string());
                    let po = pn.as_ref().and_then(|pfn| pfn.split_once('/').map(|(o, _)| o.to_string()));
                    (pn, po)
                } else {
                    (None, None)
                };
                // Use `namespace` field for owner info — `owner` points to a user,
                // while `namespace` is the actual space (personal/group/enterprise).
                let ns = &r["namespace"];
                let owner_type = match ns["type"].as_str().unwrap_or("personal") {
                    "group" => "organization",
                    "enterprise" => "enterprise",
                    _ => "user",
                }
                .to_string();
                let owner_login = ns["path"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| full_name.split_once('/').map(|(o, _)| o))
                    .unwrap_or("")
                    .to_string();
                let owner_display_name =
                    ns["name"].as_str().filter(|s| !s.is_empty()).unwrap_or(&owner_login).to_string();
                RepoSummary {
                    id: r["id"].clone(),
                    name: r["name"].as_str().unwrap_or("").to_string(),
                    full_name: full_name.to_string(),
                    owner: owner_login,
                    owner_type,
                    owner_display_name,
                    description: r["description"].as_str().unwrap_or("").to_string(),
                    private: r["private"].as_bool().unwrap_or(false),
                    fork,
                    parent_full_name,
                    parent_owner,
                }
            })
            .collect();

        // Fetch better display names for orgs/enterprises when namespace.name == path
        self.resolve_namespace_display_names(&mut repos).await;

        Ok(Paginated { items: repos, page, total_pages: last_page, total_count })
    }

    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: &PrState,
        page: u32,
        per_page: u32,
    ) -> Result<Paginated<PrSummary>, AppError> {
        let api_state = state.as_str();
        let url = format!(
            "{}/repos/{}/{}/pulls?state={}&per_page={}&page={}",
            self.base_url, owner, repo, api_state, per_page, page
        );

        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self.client.get(&full_url).header("User-Agent", "mergebeacon").send().await?;

        let header_total_count =
            resp.headers().get("total_count").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u32>().ok());
        let header_total_page =
            resp.headers().get("total_page").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u32>().ok());

        let link_header = resp.headers().get("link").and_then(|v| v.to_str().ok());
        let last_page = header_total_page.unwrap_or_else(|| Self::parse_last_page_gitee(link_header, page));

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("Gitee API {} ({}): {}", status, url, body)));
        }

        let items: Vec<Value> = resp.json().await?;

        let all_prs: Vec<PrSummary> = items
            .iter()
            .map(|pr| {
                let state_str = pr["state"].as_str().unwrap_or("");
                let merged = !pr["merged_at"].is_null();
                let pr_state = if merged || state_str == "merged" {
                    PrState::Merged
                } else if state_str == "closed" {
                    PrState::Closed
                } else {
                    PrState::Open
                };
                PrSummary {
                    number: pr["number"].as_u64().unwrap_or(0),
                    title: pr["title"].as_str().unwrap_or("").to_string(),
                    author: Self::map_user(&pr["user"]),
                    status: matches!(pr_state, PrState::Open).then(|| Self::inbox_status(pr)),
                    state: pr_state,
                    created_at: pr["created_at"].as_str().unwrap_or("").to_string(),
                    updated_at: pr["updated_at"].as_str().unwrap_or("").to_string(),
                    labels: pr["labels"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|l| l["name"].as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                }
            })
            .collect();

        let prs: Vec<PrSummary> = match state {
            PrState::Merged => all_prs.into_iter().filter(|p| matches!(p.state, PrState::Merged)).collect(),
            PrState::Closed => all_prs.into_iter().filter(|p| matches!(p.state, PrState::Closed)).collect(),
            _ => all_prs,
        };

        let total_count = if let Some(tc) = header_total_count {
            tc
        } else if prs.is_empty() {
            0
        } else if (prs.len() as u32) < per_page || page >= last_page {
            (page - 1) * per_page + prs.len() as u32
        } else {
            last_page * per_page
        };

        Ok(Paginated { items: prs, page, total_pages: last_page, total_count })
    }

    async fn list_review_inbox(
        &self,
        category: &ReviewInboxCategory,
        page: u32,
        per_page: u32,
    ) -> Result<Paginated<ReviewInboxItem>, AppError> {
        let user = self.current_user().await?;
        let repositories = self.list_inbox_repositories().await?;
        let category = *category;
        let login = user.login;
        let batches = stream::iter(repositories.into_iter().map(|(owner, repo)| {
            let login = login.clone();
            async move { self.list_repository_inbox(&owner, &repo, category, &login).await }
        }))
        .buffer_unordered(6)
        .collect::<Vec<_>>()
        .await;

        let mut items = Vec::new();
        for batch in batches {
            items.extend(batch?);
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
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let json = self.get_json::<Value>(&url).await?;

        let summary = PrSummary {
            number: json["number"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["user"]),
            state: {
                let st = json["state"].as_str().unwrap_or("");
                let merged = !json["merged_at"].is_null();
                if merged {
                    PrState::Merged
                } else if st == "closed" {
                    PrState::Closed
                } else {
                    PrState::Open
                }
            },
            created_at: json["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: json["updated_at"].as_str().unwrap_or("").to_string(),
            labels: json["labels"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|l| l["name"].as_str().map(String::from)).collect())
                .unwrap_or_default(),
            status: None,
        };

        let metadata_permissions = self.metadata_permissions(owner, repo, &summary.author.login).await;
        let mut reviewers = Vec::new();
        let mut reviewer_logins = std::collections::BTreeSet::new();
        for field in ["assignees", "api_reviewers"] {
            if let Some(users) = json[field].as_array() {
                for user in users {
                    let mapped = Self::map_user(user);
                    if reviewer_logins.insert(mapped.login.to_lowercase()) {
                        reviewers.push(mapped);
                    }
                }
            }
        }
        Ok(PrDetail {
            summary,
            body: json["body"].as_str().unwrap_or("").to_string(),
            source_branch: json["head"]["ref"].as_str().unwrap_or("").to_string(),
            target_branch: json["base"]["ref"].as_str().unwrap_or("").to_string(),
            mergeable: json["mergeable"].as_bool(),
            head_sha: json["head"]["sha"].as_str().unwrap_or("").to_string(),
            base_sha: json["base"]["sha"].as_str().unwrap_or("").to_string(),
            draft: None,
            reviewers,
            assignees: json["testers"]
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
        let repository = format!("{owner}/{repo}");
        let map_candidate = |pr: &Value| {
            let number = pr["number"].as_u64()?;
            let source_branch = pr["head"]["ref"].as_str()?.trim().to_string();
            let target_branch = pr["base"]["ref"].as_str()?.trim().to_string();
            if source_branch.is_empty() || target_branch.is_empty() {
                return None;
            }
            let state = match (pr["state"].as_str().unwrap_or(""), pr["merged_at"].is_null()) {
                ("merged", _) | (_, false) => PrState::Merged,
                ("closed", _) => PrState::Closed,
                _ => PrState::Open,
            };
            Some(PrDependencyCandidate {
                number,
                title: pr["title"].as_str().unwrap_or("").to_string(),
                state,
                source_branch,
                target_branch,
                source_repository: pr["head"]["repo"]["full_name"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("gitee-unknown-source:{number}")),
                target_repository: pr["base"]["repo"]["full_name"].as_str().unwrap_or(&repository).to_string(),
            })
        };

        let current_url = format!("{}/repos/{owner}/{repo}/pulls/{pr_number}", self.base_url);
        let current_json = self.get_json::<Value>(&current_url).await?;
        let current =
            map_candidate(&current_json).ok_or_else(|| AppError::Api("当前 PR 缺少依赖分析所需的分支信息".into()))?;
        super::walk_pr_dependency_candidates(self, current, map_candidate, |candidate| {
            let target_owner = candidate.target_repository.split('/').next().unwrap_or(owner);
            let parent_head = format!("{target_owner}:{}", candidate.target_branch);
            [
                format!(
                    "{}/repos/{owner}/{repo}/pulls?state=all&base={}",
                    self.base_url,
                    urlencoding::encode(&candidate.source_branch)
                ),
                format!(
                    "{}/repos/{owner}/{repo}/pulls?state=all&head={}",
                    self.base_url,
                    urlencoding::encode(&parent_head)
                ),
            ]
        })
        .await
    }

    async fn list_branches(&self, owner: &str, repo: &str) -> Result<PrBranchOptions, AppError> {
        let endpoint = format!("{}/repos/{}/{}/branches", self.base_url, owner, repo);
        let items = super::collect_json_pages(self, &endpoint).await?;
        let branches = items.iter().filter_map(|branch| branch["name"].as_str().map(str::to_string)).collect();
        let repository_url = format!("{}/repos/{}/{}", self.base_url, owner, repo);
        let repository = self.get_json::<Value>(&repository_url).await?;
        let default_branch = repository["default_branch"].as_str().map(str::to_string);
        Ok(PrBranchOptions { branches, default_branch })
    }

    async fn list_labels(&self, owner: &str, repo: &str) -> Result<Vec<PrLabel>, AppError> {
        let endpoint = format!("{}/repos/{}/{}/labels", self.base_url, owner, repo);
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
        let endpoint = format!("{}/repos/{}/{}/collaborators", self.base_url, owner, repo);
        let items = super::collect_json_pages(self, &endpoint).await?;
        Ok(items.iter().map(Self::map_user).filter(|user| !user.login.is_empty()).collect())
    }

    async fn create_pull_request(&self, owner: &str, repo: &str, request: &PrCreateRequest) -> Result<u64, AppError> {
        let url = format!("{}/repos/{}/{}/pulls", self.base_url, owner, repo);
        let head = if request.source_owner == owner && request.source_repo == repo {
            request.source_branch.clone()
        } else {
            format!("{}:{}", request.source_owner, request.source_branch)
        };
        let json = self
            .post_json(
                &url,
                &serde_json::json!({
                    "title": request.title,
                    "body": request.body,
                    "head": head,
                    "base": request.target_branch,
                }),
            )
            .await?;
        json["number"].as_u64().ok_or_else(|| AppError::Api("Gitee 创建 PR 后未返回编号".into()))
    }

    async fn preview_pull_request(
        &self,
        owner: &str,
        repo: &str,
        request: &PrCreatePreviewRequest,
    ) -> Result<PrCreatePreviewData, AppError> {
        if let Some(commit_sha) = request.commit_sha.as_deref() {
            let source_owner = urlencoding::encode(&request.source_owner);
            let source_repo = urlencoding::encode(&request.source_repo);
            let sha = urlencoding::encode(commit_sha);
            let url = format!("{}/repos/{}/{}/commits/{}", self.base_url, source_owner, source_repo, sha);
            let json = self.get_json::<Value>(&url).await?;
            let files_json = json["files"]
                .as_array()
                .or_else(|| json["changes"].as_array())
                .ok_or_else(|| AppError::Api("Gitee 提交响应缺少 files/changes 字段".into()))?;
            let files = files_json
                .iter()
                .map(|file| {
                    let status = match file["status"].as_str().unwrap_or("") {
                        "added" => FileStatus::Added,
                        "removed" => FileStatus::Removed,
                        "renamed" => FileStatus::Renamed,
                        _ if file["new_file"].as_bool() == Some(true) => FileStatus::Added,
                        _ if file["deleted_file"].as_bool() == Some(true) => FileStatus::Removed,
                        _ if file["renamed_file"].as_bool() == Some(true) => FileStatus::Renamed,
                        _ => FileStatus::Modified,
                    };
                    PrFile {
                        filename: file["filename"]
                            .as_str()
                            .or_else(|| file["new_path"].as_str())
                            .or_else(|| file["old_path"].as_str())
                            .unwrap_or("")
                            .to_string(),
                        status,
                        patch: Self::file_patch(file),
                        additions: file["additions"].as_u64().unwrap_or(0) as u32,
                        deletions: file["deletions"].as_u64().unwrap_or(0) as u32,
                    }
                })
                .collect::<Vec<_>>();
            let diff = files_json.iter().map(Self::unified_diff).filter(|patch| !patch.is_empty()).collect();
            let commit = &json["commit"];
            let summary = PrCommitSummary {
                sha: json["sha"].as_str().or_else(|| json["id"].as_str()).unwrap_or(commit_sha).to_string(),
                title: commit["message"].as_str().unwrap_or("").lines().next().unwrap_or("").to_string(),
                author_name: commit["author"]["name"].as_str().unwrap_or("").to_string(),
                authored_at: commit["author"]["date"].as_str().unwrap_or("").to_string(),
            };
            return Ok(PrCreatePreviewData { commits: vec![summary], diff, files, incomplete: false });
        }
        let base = urlencoding::encode(&request.target_branch);
        let head_reference = if request.source_owner == owner && request.source_repo == repo {
            request.source_branch.clone()
        } else {
            format!("{}:{}", request.source_owner, request.source_branch)
        };
        let head = urlencoding::encode(&head_reference);
        let url = format!("{}/repos/{}/{}/compare/{}...{}?per_page=100", self.base_url, owner, repo, base, head);
        let json = self.get_json::<Value>(&url).await?;
        let files_json = json["files"]
            .as_array()
            .or_else(|| json["changes"].as_array())
            .ok_or_else(|| AppError::Api("Gitee compare 响应缺少 files/changes 字段".into()))?;
        let commits_json = json["commits"].as_array();
        let incomplete = super::create_compare_is_incomplete(&json, commits_json.map_or(0, Vec::len), files_json.len());
        let files = files_json
            .iter()
            .map(|file| {
                let status = match file["status"].as_str().unwrap_or("") {
                    "added" => FileStatus::Added,
                    "removed" => FileStatus::Removed,
                    "renamed" => FileStatus::Renamed,
                    _ if file["new_file"].as_bool() == Some(true) => FileStatus::Added,
                    _ if file["deleted_file"].as_bool() == Some(true) => FileStatus::Removed,
                    _ if file["renamed_file"].as_bool() == Some(true) => FileStatus::Renamed,
                    _ => FileStatus::Modified,
                };
                PrFile {
                    filename: file["filename"]
                        .as_str()
                        .or_else(|| file["new_path"].as_str())
                        .or_else(|| file["old_path"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    status,
                    patch: Self::file_patch(file),
                    additions: file["additions"].as_u64().unwrap_or(0) as u32,
                    deletions: file["deletions"].as_u64().unwrap_or(0) as u32,
                }
            })
            .collect::<Vec<_>>();
        let diff = files_json.iter().map(Self::unified_diff).filter(|patch| !patch.is_empty()).collect();
        let commits = commits_json
            .map(|items| {
                items
                    .iter()
                    .filter_map(|commit| {
                        let sha = commit["sha"].as_str().or_else(|| commit["id"].as_str())?.to_string();
                        let message =
                            commit["commit"]["message"].as_str().or_else(|| commit["message"].as_str()).unwrap_or("");
                        Some(PrCommitSummary {
                            sha,
                            title: message.lines().next().unwrap_or("").to_string(),
                            author_name: commit["commit"]["author"]["name"]
                                .as_str()
                                .or_else(|| commit["author"]["name"].as_str())
                                .unwrap_or("")
                                .to_string(),
                            authored_at: commit["commit"]["author"]["date"]
                                .as_str()
                                .or_else(|| commit["author"]["date"].as_str())
                                .unwrap_or("")
                                .to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(PrCreatePreviewData { commits, diff, files, incomplete })
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
        if title_body_changed {
            payload.insert("title".into(), Value::String(update.title.clone()));
            payload.insert("body".into(), Value::String(update.body.clone()));
        }

        let current_reviewers =
            current.reviewers.iter().map(|user| user.login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let target_reviewers =
            update.reviewers.iter().map(|login| login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let reviewers_changed = current_reviewers != target_reviewers;
        let mut reviewers_updated = false;
        if reviewers_changed {
            let assignees_url = format!("{}/repos/{}/{}/pulls/{}/assignees", self.base_url, owner, repo, pr_number);
            let removed = current
                .reviewers
                .iter()
                .filter(|user| !target_reviewers.contains(&user.login.to_lowercase()))
                .map(|user| user.login.as_str())
                .collect::<Vec<_>>();
            let added = update
                .reviewers
                .iter()
                .filter(|login| !current_reviewers.contains(&login.to_lowercase()))
                .map(String::as_str)
                .collect::<Vec<_>>();
            let mut reviewer_error = None;
            if !removed.is_empty() {
                if let Err(error) =
                    self.delete_json(&assignees_url, &serde_json::json!({ "assignees": removed.join(",") })).await
                {
                    reviewer_error = Some(error);
                }
            }
            if reviewer_error.is_none() && !added.is_empty() {
                if let Err(error) =
                    self.post_json(&assignees_url, &serde_json::json!({ "assignees": added.join(",") })).await
                {
                    reviewer_error = Some(error);
                }
            }
            match reviewer_error {
                Some(error) => result
                    .failures
                    .push(PrMetadataUpdateFailure { field: PrMetadataField::Reviewers, message: error.to_string() }),
                None => reviewers_updated = true,
            }
        }

        let current_testers =
            current.assignees.iter().map(|user| user.login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let target_testers =
            update.assignees.iter().map(|login| login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let testers_changed = current_testers != target_testers;
        let mut testers_updated = false;
        if testers_changed {
            let testers_url = format!("{}/repos/{}/{}/pulls/{}/testers", self.base_url, owner, repo, pr_number);
            let removed = current
                .assignees
                .iter()
                .filter(|user| !target_testers.contains(&user.login.to_lowercase()))
                .map(|user| user.login.as_str())
                .collect::<Vec<_>>();
            let added = update
                .assignees
                .iter()
                .filter(|login| !current_testers.contains(&login.to_lowercase()))
                .map(String::as_str)
                .collect::<Vec<_>>();
            let mut tester_error = None;
            if !removed.is_empty() {
                if let Err(error) =
                    self.delete_json(&testers_url, &serde_json::json!({ "testers": removed.join(",") })).await
                {
                    tester_error = Some(error);
                }
            }
            if tester_error.is_none() && !added.is_empty() {
                if let Err(error) =
                    self.post_json(&testers_url, &serde_json::json!({ "testers": added.join(",") })).await
                {
                    tester_error = Some(error);
                }
            }
            match tester_error {
                Some(error) => result
                    .failures
                    .push(PrMetadataUpdateFailure { field: PrMetadataField::Assignees, message: error.to_string() }),
                None => testers_updated = true,
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
                Some(title) => match self.resolve_milestone_number(owner, repo, title).await {
                    Ok(number) => {
                        payload.insert("milestone_number".into(), serde_json::json!(number));
                    }
                    Err(error) => result.failures.push(PrMetadataUpdateFailure {
                        field: PrMetadataField::Milestone,
                        message: error.to_string(),
                    }),
                },
                None => {
                    payload.insert("milestone_number".into(), serde_json::json!(0));
                }
            }
        }

        if payload.is_empty() {
            if reviewers_updated {
                result.updated_fields.push(PrMetadataField::Reviewers);
            }
            if testers_updated {
                result.updated_fields.push(PrMetadataField::Assignees);
            }
            return Ok(result);
        }
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        match self.patch_json(&url, &Value::Object(payload)).await {
            Ok(_) => {
                if title_body_changed {
                    result.updated_fields.push(PrMetadataField::TitleBody);
                }
                if reviewers_updated {
                    result.updated_fields.push(PrMetadataField::Reviewers);
                }
                if testers_updated {
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
                if reviewers_updated {
                    result.updated_fields.push(PrMetadataField::Reviewers);
                }
                if testers_updated {
                    result.updated_fields.push(PrMetadataField::Assignees);
                }
                for field in [
                    title_body_changed.then_some(PrMetadataField::TitleBody),
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
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let json = self.get_json::<Value>(&url).await?;
        let head_sha =
            json["head"]["sha"].as_str().or_else(|| json["head"]["commit_id"].as_str()).unwrap_or("").to_string();
        let mergeable = json["mergeable"].as_bool();
        let draft = json["draft"].as_bool().or_else(|| json["work_in_progress"].as_bool());
        let has_conflicts = json["has_conflicts"]
            .as_bool()
            .or_else(|| json["conflict"].as_bool())
            .or_else(|| (mergeable == Some(true)).then_some(false));
        let mut reasons = Vec::new();
        if json["state"].as_str() != Some("open") {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::NotOpen,
                message: "PR 不是打开状态".into(),
            });
        }
        if draft == Some(true) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Draft,
                message: "PR 仍处于 Draft 状态".into(),
            });
        }
        if has_conflicts == Some(true) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Conflicts,
                message: "源分支存在合并冲突".into(),
            });
        }
        if mergeable == Some(false) && has_conflicts != Some(true) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::PlatformBlocked,
                message: "平台报告当前 PR 不可合并".into(),
            });
        }
        let repository_url = format!("{}/repos/{}/{}", self.base_url, owner, repo);
        let has_merge_permission = match self.get_json::<Value>(&repository_url).await {
            Ok(repository) => Self::repository_merge_permission(&repository),
            Err(_) => None,
        };
        if has_merge_permission == Some(false) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::NoMergePermission,
                message: "当前账号没有该仓库的合并权限".into(),
            });
        }
        let (approvals_required, approvals_received) = Self::acceptance_progress(
            &json,
            &[("assignees_number", "assignees"), ("api_reviewers_number", "api_reviewers")],
        );
        let approvals_status = match (approvals_required, approvals_received) {
            (Some(required), Some(received)) if received >= required => ReadinessState::Ready,
            (Some(required), Some(received)) => {
                reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ApprovalsRequired,
                    message: format!("还需要 {} 个审批", required.saturating_sub(received)),
                });
                ReadinessState::Blocked
            }
            _ => ReadinessState::Unknown,
        };

        let (tests_required, tests_received) = Self::acceptance_progress(&json, &[("testers_number", "testers")]);
        let checks_status = match (tests_required, tests_received) {
            (Some(required), Some(received)) if received >= required => ReadinessState::Ready,
            (Some(required), Some(received)) => {
                reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ChecksPending,
                    message: format!("还需要 {} 个测试通过", required.saturating_sub(received)),
                });
                ReadinessState::Pending
            }
            _ => ReadinessState::Unknown,
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
            branch_behind: None,
            blocking_reasons: reasons,
        })
    }

    async fn get_pr_diff(&self, owner: &str, repo: &str, pr_number: u64) -> Result<(String, Vec<PrFile>), AppError> {
        let files_url = format!("{}/repos/{}/{}/pulls/{}/files?per_page=300", self.base_url, owner, repo, pr_number);
        let files_json: Vec<Value> = self.get_json(&files_url).await?;

        let files: Vec<PrFile> = files_json
            .iter()
            .map(|f| {
                let patch = Self::file_patch(f);
                PrFile {
                    filename: f["filename"].as_str().unwrap_or("").to_string(),
                    status: match f["status"].as_str().unwrap_or("") {
                        "added" => FileStatus::Added,
                        "removed" => FileStatus::Removed,
                        "renamed" => FileStatus::Renamed,
                        _ => FileStatus::Modified,
                    },
                    patch,
                    additions: f["additions"].as_u64().unwrap_or(0) as u32,
                    deletions: f["deletions"].as_u64().unwrap_or(0) as u32,
                }
            })
            .collect();

        let diff = files_json.iter().map(Self::unified_diff).filter(|patch| !patch.is_empty()).collect::<String>();

        Ok((diff, files))
    }

    async fn get_compare_diff(
        &self,
        owner: &str,
        repo: &str,
        base_sha: &str,
        head_sha: &str,
    ) -> Result<(String, Vec<PrFile>), AppError> {
        let base = urlencoding::encode(base_sha);
        let head = urlencoding::encode(head_sha);
        let url = format!("{}/repos/{}/{}/compare/{}...{}", self.base_url, owner, repo, base, head);
        let json = self.get_json::<Value>(&url).await?;
        let files_json = json["files"]
            .as_array()
            .or_else(|| json["changes"].as_array())
            .ok_or_else(|| AppError::Api("Gitee compare 响应缺少 files/changes 字段".into()))?;
        let files: Vec<PrFile> = files_json
            .iter()
            .map(|file| {
                let status = match file["status"].as_str().unwrap_or("") {
                    "added" => FileStatus::Added,
                    "removed" => FileStatus::Removed,
                    "renamed" => FileStatus::Renamed,
                    _ if file["new_file"].as_bool() == Some(true) => FileStatus::Added,
                    _ if file["deleted_file"].as_bool() == Some(true) => FileStatus::Removed,
                    _ if file["renamed_file"].as_bool() == Some(true) => FileStatus::Renamed,
                    _ => FileStatus::Modified,
                };
                PrFile {
                    filename: file["filename"]
                        .as_str()
                        .or_else(|| file["new_path"].as_str())
                        .or_else(|| file["old_path"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    status,
                    patch: Self::file_patch(file),
                    additions: file["additions"].as_u64().unwrap_or(0) as u32,
                    deletions: file["deletions"].as_u64().unwrap_or(0) as u32,
                }
            })
            .collect();
        let diff = files_json.iter().map(Self::unified_diff).filter(|patch| !patch.is_empty()).collect();
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
        let encoded_path = crate::file_content::encode_path_segments(path);
        let encoded_revision = urlencoding::encode(revision);
        let url =
            format!("{}/repos/{}/{}/contents/{}?ref={}", self.base_url, owner, repo, encoded_path, encoded_revision);
        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());
        let response = self
            .client
            .get(&full_url)
            .header("User-Agent", "mergebeacon")
            .send()
            .await
            .map_err(|error| AppError::Http(error.without_url()))?;
        if !response.status().is_success() {
            return Err(AppError::Api(format!("Gitee 文件内容请求失败（HTTP {}）", response.status())));
        }
        let json = response.json::<Value>().await.map_err(|error| AppError::Http(error.without_url()))?;
        crate::file_content::decode_response("Gitee", path, revision, &json)
    }

    async fn create_review(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        body: &str,
        event: &ReviewEvent,
        comments: &[ReviewCommentPosition],
    ) -> Result<Review, AppError> {
        if !matches!(event, ReviewEvent::Comment) {
            return Err(AppError::NotImplemented("该平台仅支持评论评审".to_string()));
        }
        // Gitee API does not support batch inline comments in review creation.
        // Create the main review comment first.
        let url = format!("{}/repos/{}/{}/pulls/{}/comments", self.base_url, owner, repo, pr_number);
        let payload = serde_json::json!({
            "body": body,
        });
        let json = self.post_json(&url, &payload).await?;

        // Then create each inline comment individually.
        for c in comments {
            let inline_url = format!("{}/repos/{}/{}/pulls/{}/comments", self.base_url, owner, repo, pr_number);
            let inline_payload = serde_json::json!({
                "body": c.body,
                "path": c.path,
                "position": c.position,
            });
            self.post_json(&inline_url, &inline_payload).await?;
        }

        Ok(Review {
            id: json["id"].clone(),
            body: json["body"].as_str().unwrap_or("").to_string(),
            state: "commented".to_string(),
            author: Self::map_user(&json["user"]),
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
        let url = format!("{}/repos/{}/{}/pulls/{}/comments", self.base_url, owner, repo, pr_number);
        // Gitee API does not support start_line/end_line for range comments.
        // For multi-line selections, embed the line range in the comment body.
        let final_body =
            if let Some(sl) = start_line { format!("[L{}-L{}]\n{}", sl, line, body) } else { body.to_string() };
        let payload = serde_json::json!({
            "body": final_body,
            "commit_id": commit_id,
            "path": path,
            "position": line,
        });
        let c: Value = self.post_json(&url, &payload).await?;
        let comment_id = c["id"].as_str().map(str::to_string).unwrap_or_else(|| c["id"].to_string());
        Ok(PrComment {
            id: c["id"].clone(),
            body: c["body"].as_str().unwrap_or("").to_string(),
            path: c["path"].as_str().unwrap_or("").to_string(),
            line: c["line"].as_u64().map(|n| n as u32).or_else(|| c["position"].as_u64().map(|n| n as u32)),
            start_line: c["start_line"].as_u64().map(|n| n as u32),
            side: Some(if side == "left" { "left" } else { "right" }.into()),
            author: Self::map_user(&c["user"]),
            created_at: c["created_at"].as_str().unwrap_or("").to_string(),
            commit_id: c["commit_id"].as_str().map(|s| s.to_string()),
            original_commit_id: c["original_commit_id"].as_str().map(|s| s.to_string()),
            original_line: c["original_line"].as_u64().map(|n| n as u32),
            original_start_line: c["original_start_line"].as_u64().map(|n| n as u32),
            diff_hunk: None, // Gitee API does not return diff_hunk
            thread_id: c["in_reply_to_id"]
                .as_str()
                .map(str::to_string)
                .or_else(|| c["in_reply_to_id"].as_u64().map(|id| id.to_string()))
                .unwrap_or(comment_id),
            reply_to_id: c["in_reply_to_id"]
                .as_str()
                .map(str::to_string)
                .or_else(|| c["in_reply_to_id"].as_u64().map(|id| id.to_string())),
            resolved: None,
            resolvable: false,
            can_edit: true,
            can_delete: true,
        })
    }

    async fn list_pr_comments(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<PrComment>, AppError> {
        let endpoint = format!("{}/repos/{}/{}/pulls/{}/comments", self.base_url, owner, repo, pr_number);
        let items = super::collect_json_pages(self, &endpoint).await?;
        let comments = items
            .iter()
            .filter(|c| c["path"].is_string() && !c["path"].as_str().unwrap_or("").is_empty())
            .map(|c| {
                let (line, side) = if let Some(line) = c["new_line"].as_u64() {
                    (Some(line as u32), Some("right".to_string()))
                } else if let Some(line) = c["old_line"].as_u64() {
                    (Some(line as u32), Some("left".to_string()))
                } else {
                    (
                        c["position"].as_u64().map(|line| line as u32),
                        c["position"].as_u64().map(|_| "right".to_string()),
                    )
                };
                let comment_id = c["id"].as_str().map(str::to_string).unwrap_or_else(|| c["id"].to_string());
                let reply_to_id = c["in_reply_to_id"]
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| c["in_reply_to_id"].as_u64().map(|id| id.to_string()));
                PrComment {
                    id: c["id"].clone(),
                    body: c["body"].as_str().unwrap_or("").to_string(),
                    path: c["path"].as_str().unwrap_or("").to_string(),
                    line,
                    start_line: c["start_line"].as_u64().map(|n| n as u32),
                    side,
                    author: Self::map_user(&c["user"]),
                    created_at: c["created_at"].as_str().unwrap_or("").to_string(),
                    commit_id: c["commit_id"].as_str().map(|s| s.to_string()),
                    original_commit_id: c["original_commit_id"].as_str().map(|s| s.to_string()),
                    original_line: c["original_line"].as_u64().map(|n| n as u32),
                    original_start_line: c["original_start_line"].as_u64().map(|n| n as u32),
                    diff_hunk: None, // populated by command layer from SQLite
                    thread_id: reply_to_id.clone().unwrap_or(comment_id),
                    reply_to_id,
                    resolved: None,
                    resolvable: false,
                    can_edit: false,
                    can_delete: false,
                }
            })
            .collect();
        Ok(comments)
    }

    async fn reply_to_review_thread(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        _thread_id: &str,
        reply_to_id: &str,
        body: &str,
    ) -> Result<(), AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}/comments", self.base_url, owner, repo, pr_number);
        let in_reply_to = reply_to_id
            .parse::<u64>()
            .map_or_else(|_| Value::String(reply_to_id.to_string()), |id| Value::Number(id.into()));
        self.post_json(&url, &serde_json::json!({ "body": body, "in_reply_to": in_reply_to })).await?;
        Ok(())
    }

    async fn update_review_comment(
        &self,
        owner: &str,
        repo: &str,
        _pr_number: u64,
        _thread_id: &str,
        comment_id: &str,
        body: &str,
    ) -> Result<(), AppError> {
        let url = format!("{}/repos/{}/{}/pulls/comments/{}", self.base_url, owner, repo, comment_id);
        self.patch_json(&url, &serde_json::json!({ "body": body })).await?;
        Ok(())
    }

    async fn delete_review_comment(
        &self,
        owner: &str,
        repo: &str,
        _pr_number: u64,
        _thread_id: &str,
        comment_id: &str,
    ) -> Result<(), AppError> {
        let url = format!("{}/repos/{}/{}/pulls/comments/{}", self.base_url, owner, repo, comment_id);
        self.delete_json(&url, &Value::Null).await?;
        Ok(())
    }

    async fn list_reviews(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<Review>, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}/comments?per_page=100", self.base_url, owner, repo, pr_number);
        let items: Vec<Value> = self.get_json(&url).await?;

        let reviews = items
            .iter()
            .filter(|c| {
                let path = c["path"].as_str().unwrap_or("");
                path.is_empty()
            })
            .map(|c| Review {
                id: c["id"].clone(),
                body: c["body"].as_str().unwrap_or("").to_string(),
                state: "commented".to_string(),
                author: Self::map_user(&c["user"]),
                submitted_at: c["created_at"].as_str().unwrap_or("").to_string(),
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
        let state_param = state.as_str();
        let url = format!(
            "{}/repos/{}/{}/issues?state={}&per_page=100&page={}",
            self.base_url, owner, repo, state_param, page
        );

        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self.client.get(&full_url).header("User-Agent", "mergebeacon").send().await?;

        let link_header = resp.headers().get("link").and_then(|v| v.to_str().ok());
        let last_page = Self::parse_last_page_gitee(link_header, page);

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("Gitee API {} ({}): {}", status, url, body)));
        }

        let items: Vec<Value> = resp.json().await?;

        let issues = items
            .iter()
            .filter(|i| !i["pull_request"].is_object())
            .map(|i| IssueSummary {
                number: i["number"].as_u64().unwrap_or(0),
                title: i["title"].as_str().unwrap_or("").to_string(),
                author: Self::map_user(&i["user"]),
                state: match i["state"].as_str().unwrap_or("") {
                    "closed" => IssueState::Closed,
                    _ => IssueState::Open,
                },
                labels: i["labels"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|l| l["name"].as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                created_at: i["created_at"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(Paginated { items: issues, page, total_pages: last_page, total_count: 0 })
    }

    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<Issue, AppError> {
        let url = format!("{}/repos/{}/{}/issues", self.base_url, owner, repo);
        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "labels": labels.join(","),
        });

        let json = self.post_json(&url, &payload).await?;

        Ok(Issue {
            number: json["number"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            body: json["body"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["user"]),
            state: match json["state"].as_str().unwrap_or("") {
                "closed" => IssueState::Closed,
                _ => IssueState::Open,
            },
            labels: json["labels"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|l| l["name"].as_str().map(String::from)).collect())
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
        commit_title: Option<String>,
        commit_message: Option<String>,
        sha: &str,
    ) -> Result<PrMergeResult, AppError> {
        let merge_method = match strategy {
            MergeStrategy::Merge => "merge",
            MergeStrategy::Squash => "squash",
            MergeStrategy::Rebase => "rebase",
        };
        let url = format!("{}/repos/{}/{}/pulls/{}/merge", self.base_url, owner, repo, pr_number);
        let mut payload = serde_json::json!({
            "merge_method": merge_method,
            "sha": sha,
        });
        if let Some(t) = commit_title {
            payload["title"] = serde_json::Value::String(t);
        }
        if let Some(m) = commit_message {
            payload["description"] = serde_json::Value::String(m);
        }
        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());
        let resp = self.client.put(&full_url).header("User-Agent", "mergebeacon").json(&payload).send().await?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            let detail = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v["message"].as_str().map(String::from))
                .unwrap_or_else(|| if body.is_empty() { "未知错误".to_string() } else { body });
            return Err(AppError::Api(detail));
        }
        let json: Value = resp.json().await?;
        Ok(PrMergeResult {
            merged: json["merged"].as_bool().unwrap_or(false),
            sha: json["sha"].as_str().unwrap_or("").to_string(),
            message: json["message"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn close_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let payload = serde_json::json!({ "state": "closed" });
        let json = self.patch_json(&url, &payload).await?;
        let state = if !json["merged_at"].is_null() {
            PrState::Merged
        } else if json["state"].as_str().unwrap_or("") == "closed" {
            PrState::Closed
        } else {
            PrState::Open
        };
        Ok(state)
    }

    async fn reopen_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let payload = serde_json::json!({ "state": "open" });
        let json = self.patch_json(&url, &payload).await?;
        let state = if json["state"].as_str().unwrap_or("") == "open" { PrState::Open } else { PrState::Closed };
        Ok(state)
    }

    async fn close_issue(&self, owner: &str, repo: &str, issue_number: u64) -> Result<(), AppError> {
        let url = format!("{}/repos/{}/{}/issues/{}", self.base_url, owner, repo, issue_number);
        let payload = serde_json::json!({ "state": "closed" });
        self.patch_json(&url, &payload).await?;
        Ok(())
    }
}
