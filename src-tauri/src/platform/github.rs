use async_trait::async_trait;
use serde_json::Value;

use super::GitPlatform;
use crate::error::AppError;
use crate::http_client::HttpClient;
use crate::models::*;

pub struct GitHubAdapter {
    client: HttpClient,
    token: String,
    base_url: String,
}

#[derive(Debug, Clone)]
struct GithubInboxSupplement {
    status: ReviewInboxStatusSummary,
    head_sha: Option<String>,
    comments_count: Option<u64>,
}

impl GitHubAdapter {
    pub fn new(client: HttpClient, token: String) -> Self {
        Self { client, token, base_url: "https://api.github.com".to_string() }
    }

    #[allow(dead_code)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url.trim_end_matches('/').to_string();
        self
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    fn legacy_commit_status(value: &Value) -> Option<ReadinessState> {
        let total_count = value["total_count"].as_u64();
        let statuses = value["statuses"].as_array();
        let has_statuses = if total_count == Some(0) {
            false
        } else if total_count.is_some() {
            true
        } else if let Some(statuses) = statuses {
            !statuses.is_empty()
        } else {
            // Older GitHub Enterprise responses and test doubles may only expose `state`.
            value["state"].as_str().is_some()
        };
        if !has_statuses {
            return None;
        }

        Some(match value["state"].as_str() {
            Some("success") => ReadinessState::Ready,
            Some("failure") | Some("error") => ReadinessState::Blocked,
            Some("pending") => ReadinessState::Pending,
            _ => ReadinessState::Unknown,
        })
    }

    fn check_runs_status(value: &Value) -> Option<ReadinessState> {
        let total_count = value["total_count"].as_u64();
        let Some(check_runs) = value["check_runs"].as_array() else {
            return if total_count == Some(0) { None } else { Some(ReadinessState::Unknown) };
        };
        if check_runs.is_empty() {
            return if total_count.unwrap_or(0) == 0 { None } else { Some(ReadinessState::Unknown) };
        }
        if total_count.is_some_and(|count| usize::try_from(count).map_or(true, |count| count > check_runs.len())) {
            // Never report Ready from a truncated first page that may omit a failing run.
            return Some(ReadinessState::Unknown);
        }

        let mut has_pending = false;
        let mut has_unknown = false;
        for check_run in check_runs {
            match check_run["status"].as_str() {
                Some("completed") => match check_run["conclusion"].as_str() {
                    Some("success") | Some("neutral") | Some("skipped") => {}
                    Some("failure")
                    | Some("cancelled")
                    | Some("timed_out")
                    | Some("action_required")
                    | Some("stale")
                    | Some("startup_failure") => return Some(ReadinessState::Blocked),
                    _ => has_unknown = true,
                },
                Some("queued") | Some("in_progress") | Some("pending") | Some("waiting") | Some("requested") => {
                    has_pending = true;
                }
                _ => has_unknown = true,
            }
        }

        if has_pending {
            Some(ReadinessState::Pending)
        } else if has_unknown {
            Some(ReadinessState::Unknown)
        } else {
            Some(ReadinessState::Ready)
        }
    }

    fn combined_checks_status(
        legacy_status: Option<ReadinessState>,
        check_runs_status: Option<ReadinessState>,
    ) -> ReadinessState {
        let states = [legacy_status, check_runs_status];
        if states.contains(&Some(ReadinessState::Blocked)) {
            ReadinessState::Blocked
        } else if states.contains(&Some(ReadinessState::Pending)) {
            ReadinessState::Pending
        } else if states.contains(&Some(ReadinessState::Unknown)) {
            ReadinessState::Unknown
        } else if states.contains(&Some(ReadinessState::Ready)) {
            ReadinessState::Ready
        } else {
            // Both GitHub status APIs successfully returned no entries. This means the
            // repository has no CI status configured for the commit, not that the lookup
            // failed. Request failures are represented explicitly as `Some(Unknown)`.
            ReadinessState::Ready
        }
    }

    fn repository_merge_permission(value: &Value) -> Option<bool> {
        let permissions = &value["permissions"];
        let admin = permissions["admin"].as_bool();
        let maintain = permissions["maintain"].as_bool();
        let push = permissions["push"].as_bool();

        if admin.is_none() && maintain.is_none() && push.is_none() {
            None
        } else {
            Some(admin == Some(true) || maintain == Some(true) || push == Some(true))
        }
    }

    /// Fetch org/enterprise display names via `/orgs/{login}`.
    /// Updates `owner_display_name` in place for org repos.
    async fn resolve_org_display_names(&self, repos: &mut [RepoSummary]) {
        let orgs: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            repos
                .iter()
                .filter(|r| r.owner_type == "organization" && seen.insert(r.owner.clone()))
                .map(|r| r.owner.clone())
                .collect()
        };
        if orgs.is_empty() {
            return;
        }

        let auth = self.auth_header();
        let client = self.client.clone();
        let base = self.base_url.clone();

        let futs: Vec<_> = orgs
            .into_iter()
            .map(|login| {
                let url = format!("{}/orgs/{}", base, login);
                let c = client.clone();
                let a = auth.clone();
                tokio::spawn(async move {
                    let resp = c
                        .get(&url)
                        .header("Authorization", &a)
                        .header("User-Agent", "mergebeacon")
                        .header("Accept", "application/vnd.github.v3+json")
                        .send()
                        .await
                        .ok()?;
                    let json: serde_json::Value = resp.json().await.ok()?;
                    let name = json["name"].as_str()?.to_string();
                    Some((login, name))
                })
            })
            .collect();

        let results = futures::future::join_all(futs).await;
        let mut name_map = std::collections::HashMap::new();
        for r in results {
            if let Ok(Some((login, name))) = r {
                name_map.insert(login, name);
            }
        }

        for r in repos.iter_mut() {
            if let Some(name) = name_map.get(&r.owner) {
                r.owner_display_name = name.clone();
            }
        }
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, AppError> {
        let resp = self
            .client
            .get(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, body)));
        }
        Ok(resp.json().await?)
    }

    /// Parse the `Link` header to extract the last page number.
    /// GitHub format: `<url?page=5>; rel="last"`
    fn parse_last_page(link: Option<&str>, fallback: u32) -> u32 {
        let Some(header) = link else {
            return fallback;
        };
        // Find the URL with rel="last"
        for part in header.split(',') {
            let part = part.trim();
            if part.contains(r#"rel="last""#) {
                // Extract the page=XX from the URL between < and >
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

    async fn get_text(&self, url: &str) -> Result<String, AppError> {
        let resp = self
            .client
            .get(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3.diff")
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, body)));
        }
        Ok(resp.text().await?)
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let resp = self
            .client
            .post(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, error_body)));
        }
        Ok(resp.json().await?)
    }

    async fn put_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let resp = self
            .client
            .put(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, error_body)));
        }
        Ok(resp.json().await?)
    }

    async fn patch_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        // PATCH via reqwest::Client
        let resp = self
            .client
            .raw_client()
            .patch(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, error_body)));
        }
        Ok(resp.json().await?)
    }

    async fn delete_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let resp = self
            .client
            .raw_client()
            .delete(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, error_body)));
        }
        if resp.status() == reqwest::StatusCode::NO_CONTENT {
            Ok(Value::Null)
        } else {
            Ok(resp.json().await?)
        }
    }

    fn map_compare_file(file: &Value) -> PrFile {
        let status = match file["status"].as_str().unwrap_or("") {
            "added" => FileStatus::Added,
            "removed" => FileStatus::Removed,
            "renamed" => FileStatus::Renamed,
            _ => FileStatus::Modified,
        };
        PrFile {
            filename: file["filename"].as_str().unwrap_or("").to_string(),
            status,
            patch: Self::compare_file_patch(file),
            additions: file["additions"].as_u64().unwrap_or(0) as u32,
            deletions: file["deletions"].as_u64().unwrap_or(0) as u32,
        }
    }

    fn compare_file_paths(file: &Value) -> (&str, &str) {
        let old_path = file["previous_filename"].as_str().unwrap_or_else(|| file["filename"].as_str().unwrap_or(""));
        let new_path = file["filename"].as_str().unwrap_or("");
        (old_path, new_path)
    }

    fn compare_file_patch(file: &Value) -> String {
        let patch = file["patch"].as_str().unwrap_or("");
        if !patch.trim().is_empty() {
            return patch.to_string();
        }
        let (old_path, new_path) = Self::compare_file_paths(file);
        let is_metadata_only_rename = file["status"].as_str() == Some("renamed")
            && file["additions"].as_u64().unwrap_or(0) == 0
            && file["deletions"].as_u64().unwrap_or(0) == 0
            && !old_path.is_empty()
            && !new_path.is_empty()
            && old_path != new_path;
        if is_metadata_only_rename {
            crate::patch::metadata_only_rename_patch(old_path, new_path)
        } else {
            String::new()
        }
    }

    fn compare_unified_diff(file: &Value) -> String {
        let patch = Self::compare_file_patch(file);
        if patch.is_empty() {
            return patch;
        }
        let mut diff = if patch.starts_with("diff --git ") {
            patch
        } else {
            let (old_path, new_path) = Self::compare_file_paths(file);
            let old_marker = if file["status"].as_str() == Some("added") {
                "/dev/null".to_string()
            } else {
                format!("a/{old_path}")
            };
            let new_marker = if file["status"].as_str() == Some("removed") {
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

    async fn list_all_inbox_search(&self, qualifier: &str) -> Result<Vec<Value>, AppError> {
        const REMOTE_PAGE_SIZE: u32 = 100;
        const SEARCH_RESULT_LIMIT: u32 = 1_000;

        let url = format!("{}/search/issues", self.base_url);
        let query = format!("is:pr is:open {qualifier}");
        let mut page = 1_u32;
        let mut items = Vec::new();

        loop {
            let response = self
                .client
                .raw_client()
                .get(&url)
                .header("Authorization", self.auth_header())
                .header("User-Agent", "mergebeacon")
                .header("Accept", "application/vnd.github.v3+json")
                .query(&[("q", query.as_str()), ("sort", "updated"), ("order", "desc")])
                .query(&[("page", page), ("per_page", REMOTE_PAGE_SIZE)])
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, body)));
            }

            let mut json: Value = response.json().await?;
            let total_count = json["total_count"].as_u64().unwrap_or(0).min(u64::from(SEARCH_RESULT_LIMIT)) as u32;
            let page_items =
                json["items"].as_array_mut().ok_or_else(|| AppError::Api("GitHub 收件箱响应缺少 items".into()))?;
            let fetched = page_items.len();
            items.append(page_items);

            let total_pages = total_count.div_ceil(REMOTE_PAGE_SIZE);
            if fetched == 0 || page >= total_pages || items.len() >= SEARCH_RESULT_LIMIT as usize {
                break;
            }
            page = page.saturating_add(1);
        }

        items.truncate(SEARCH_RESULT_LIMIT as usize);
        Ok(items)
    }

    fn inbox_status(node: &Value) -> ReviewInboxStatusSummary {
        let draft = node["isDraft"].as_bool();
        let mergeable = node["mergeable"].as_str();
        let merge_state = node["mergeStateStatus"].as_str();
        let review_decision = node["reviewDecision"].as_str();
        let check_rollup = node["commits"]["nodes"]
            .as_array()
            .and_then(|nodes| nodes.last())
            .and_then(|commit| commit["commit"]["statusCheckRollup"]["state"].as_str());
        let has_conflicts = match mergeable {
            Some("CONFLICTING") => Some(true),
            Some("MERGEABLE") => Some(false),
            _ => None,
        };
        let mut blocking_reasons = Vec::new();

        if draft == Some(true) || merge_state == Some("DRAFT") {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Draft,
                message: "PR 仍处于 Draft 状态".into(),
            });
        }
        if has_conflicts == Some(true) || merge_state == Some("DIRTY") {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Conflicts,
                message: "源分支存在合并冲突".into(),
            });
        }
        if merge_state == Some("BEHIND") {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::BranchBehind,
                message: "源分支落后于目标分支".into(),
            });
        }
        if merge_state == Some("BLOCKED") {
            blocking_reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::PlatformBlocked,
                message: "GitHub 分支保护或仓库规则阻止合并".into(),
            });
        }

        let checks_status = match check_rollup {
            Some("SUCCESS") => ReadinessState::Ready,
            Some("FAILURE") | Some("ERROR") => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ChecksFailed,
                    message: "CI 检查未通过".into(),
                });
                ReadinessState::Blocked
            }
            Some("PENDING") | Some("EXPECTED") => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ChecksPending,
                    message: "CI 检查仍在进行中".into(),
                });
                ReadinessState::Pending
            }
            Some(_) | None => ReadinessState::Unknown,
        };
        let approvals_status = match review_decision {
            Some("APPROVED") => ReadinessState::Ready,
            Some("CHANGES_REQUESTED") => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ChangesRequested,
                    message: "已有评审请求修改".into(),
                });
                ReadinessState::Blocked
            }
            Some("REVIEW_REQUIRED") => {
                blocking_reasons.push(MergeBlockingReason {
                    code: MergeBlockingReasonCode::ApprovalsRequired,
                    message: "审批尚未满足合并要求".into(),
                });
                ReadinessState::Blocked
            }
            Some(_) | None => ReadinessState::Unknown,
        };

        let has_hard_blocker =
            blocking_reasons.iter().any(|reason| reason.code != MergeBlockingReasonCode::ChecksPending);
        let status = if has_hard_blocker
            || checks_status == ReadinessState::Blocked
            || approvals_status == ReadinessState::Blocked
        {
            ReadinessState::Blocked
        } else if checks_status == ReadinessState::Pending || mergeable == Some("UNKNOWN") {
            ReadinessState::Pending
        } else if matches!(merge_state, Some("CLEAN") | Some("HAS_HOOKS") | Some("UNSTABLE"))
            && mergeable == Some("MERGEABLE")
            && draft != Some(true)
        {
            ReadinessState::Ready
        } else {
            ReadinessState::Unknown
        };

        ReviewInboxStatusSummary { status, draft, has_conflicts, checks_status, approvals_status, blocking_reasons }
    }

    async fn inbox_statuses(
        &self,
        node_ids: &[String],
    ) -> Result<std::collections::HashMap<String, GithubInboxSupplement>, AppError> {
        if node_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let url = format!("{}/graphql", self.base_url);
        let payload = serde_json::json!({
            "query": r#"query ReviewInboxStatuses($ids: [ID!]!) {
                nodes(ids: $ids) {
                    ... on PullRequest {
                        id
                        headRefOid
                        isDraft
                        mergeable
                        mergeStateStatus
                        reviewDecision
                        commits(last: 1) {
                            nodes {
                                commit {
                                    statusCheckRollup { state }
                                }
                            }
                        }
                        comments(first: 1) { totalCount }
                        reviewThreads(first: 100) {
                            nodes { comments(first: 1) { totalCount } }
                        }
                    }
                }
            }"#,
            "variables": { "ids": node_ids },
        });
        let response = self.post_json(&url, &payload).await?;
        if let Some(errors) = response["errors"].as_array().filter(|errors| !errors.is_empty()) {
            return Err(AppError::Api(format!("GitHub GraphQL 收件箱状态查询失败: {errors:?}")));
        }
        let nodes = response["data"]["nodes"]
            .as_array()
            .ok_or_else(|| AppError::Api("GitHub GraphQL 收件箱状态响应缺少 data.nodes".into()))?;
        let mut statuses = std::collections::HashMap::new();
        for node in nodes {
            if let Some(id) = node["id"].as_str() {
                let issue_comments = node["comments"]["totalCount"].as_u64();
                let thread_comments = node["reviewThreads"]["nodes"].as_array().map(|threads| {
                    threads
                        .iter()
                        .filter_map(|thread| thread["comments"]["totalCount"].as_u64())
                        .fold(0_u64, u64::saturating_add)
                });
                let comments_count = if issue_comments.is_none() && thread_comments.is_none() {
                    None
                } else {
                    Some(issue_comments.unwrap_or_default().saturating_add(thread_comments.unwrap_or_default()))
                };
                statuses.insert(
                    id.to_string(),
                    GithubInboxSupplement {
                        status: Self::inbox_status(node),
                        head_sha: node["headRefOid"].as_str().map(str::to_string),
                        comments_count,
                    },
                );
            }
        }
        Ok(statuses)
    }

    async fn graphql(&self, operation: &str, query: &str, variables: Value) -> Result<Value, AppError> {
        let url = format!("{}/graphql", self.base_url);
        let response = self
            .post_json(
                &url,
                &serde_json::json!({
                    "query": query,
                    "variables": variables,
                }),
            )
            .await?;
        if let Some(errors) = response["errors"].as_array().filter(|errors| !errors.is_empty()) {
            return Err(AppError::Api(format!("GitHub GraphQL {operation}失败: {errors:?}")));
        }
        let data = response
            .get("data")
            .filter(|data| !data.is_null())
            .ok_or_else(|| AppError::Api(format!("GitHub GraphQL {operation}响应缺少 data")))?;
        Ok(data.clone())
    }

    fn graphql_database_id(value: &Value) -> Option<(Value, String)> {
        if let Some(id) = value["fullDatabaseId"].as_u64() {
            return Some((serde_json::json!(id), id.to_string()));
        }
        if let Some(id) = value["fullDatabaseId"].as_str() {
            let json_id = id.parse::<u64>().map_or_else(|_| Value::String(id.to_string()), |id| serde_json::json!(id));
            return Some((json_id, id.to_string()));
        }
        value["id"].as_str().map(|id| (Value::String(id.to_string()), id.to_string()))
    }

    fn map_graphql_user(json: &Value) -> User {
        User {
            id: json["id"].clone(),
            login: json["login"].as_str().unwrap_or("").to_string(),
            name: json["name"].as_str().unwrap_or("").to_string(),
            avatar_url: json["avatarUrl"].as_str().unwrap_or("").to_string(),
        }
    }

    fn map_graphql_review_comment(thread: &Value, comment: &Value) -> Result<PrComment, AppError> {
        let thread_id =
            thread["id"].as_str().ok_or_else(|| AppError::Api("GitHub 评审线程响应缺少线程 ID".into()))?.to_string();
        let resolved =
            thread["isResolved"].as_bool().ok_or_else(|| AppError::Api("GitHub 评审线程响应缺少解决状态".into()))?;
        let resolvable = if resolved {
            thread["viewerCanUnresolve"].as_bool().unwrap_or(false)
        } else {
            thread["viewerCanResolve"].as_bool().unwrap_or(false)
        };
        let (id, _) =
            Self::graphql_database_id(comment).ok_or_else(|| AppError::Api("GitHub 评审评论响应缺少评论 ID".into()))?;
        let reply_to_id = Self::graphql_database_id(&comment["replyTo"]).map(|(_, id)| id);

        Ok(PrComment {
            id,
            body: comment["body"].as_str().unwrap_or("").to_string(),
            path: comment["path"].as_str().unwrap_or("").to_string(),
            line: comment["line"].as_u64().map(|line| line as u32),
            start_line: comment["startLine"].as_u64().map(|line| line as u32),
            side: match thread["diffSide"].as_str() {
                Some("LEFT") => Some("left".into()),
                Some("RIGHT") => Some("right".into()),
                _ => None,
            },
            author: Self::map_graphql_user(&comment["author"]),
            created_at: comment["createdAt"].as_str().unwrap_or("").to_string(),
            commit_id: comment["commit"]["oid"].as_str().map(str::to_string),
            original_commit_id: comment["originalCommit"]["oid"].as_str().map(str::to_string),
            original_line: comment["originalLine"].as_u64().map(|line| line as u32),
            original_start_line: comment["originalStartLine"].as_u64().map(|line| line as u32),
            diff_hunk: comment["diffHunk"].as_str().map(str::to_string),
            thread_id,
            reply_to_id,
            resolved: Some(resolved),
            resolvable,
            can_edit: false,
            can_delete: false,
        })
    }

    async fn review_thread_comments_page(&self, thread_id: &str, after: &str) -> Result<Value, AppError> {
        let data = self
            .graphql(
                "评审线程评论分页查询",
                r#"query ReviewThreadComments($threadId: ID!, $after: String) {
                    node(id: $threadId) {
                        ... on PullRequestReviewThread {
                            comments(first: 100, after: $after) {
                                nodes {
                                    id
                                    fullDatabaseId
                                    body
                                    path
                                    line
                                    startLine
                                    author {
                                        login
                                        avatarUrl
                                        ... on User { id name }
                                    }
                                    createdAt
                                    commit { oid }
                                    originalCommit { oid }
                                    originalLine
                                    originalStartLine
                                    diffHunk
                                    replyTo { id fullDatabaseId }
                                }
                                pageInfo { hasNextPage endCursor }
                            }
                        }
                    }
                }"#,
                serde_json::json!({ "threadId": thread_id, "after": after }),
            )
            .await?;
        data["node"]["comments"]
            .as_object()
            .map(|comments| Value::Object(comments.clone()))
            .ok_or_else(|| AppError::Api("GitHub 评审线程评论分页响应缺少 comments".into()))
    }

    async fn pull_request_node_id(&self, owner: &str, repo: &str, pr_number: u64) -> Result<String, AppError> {
        let number =
            i32::try_from(pr_number).map_err(|_| AppError::Api("GitHub PR 编号超出 GraphQL Int 范围".into()))?;
        let data = self
            .graphql(
                "PR Node ID 查询",
                r#"query PullRequestNodeId($owner: String!, $repo: String!, $number: Int!) {
                    repository(owner: $owner, name: $repo) {
                        pullRequest(number: $number) { id }
                    }
                }"#,
                serde_json::json!({ "owner": owner, "repo": repo, "number": number }),
            )
            .await?;
        data["repository"]["pullRequest"]["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .ok_or_else(|| AppError::Api("GitHub PR Node ID 响应缺少 ID".into()))
    }

    fn repository_from_api_url(url: &str) -> Result<(String, String), AppError> {
        let path = url
            .split_once("/repos/")
            .map(|(_, path)| path)
            .ok_or_else(|| AppError::Api("GitHub 收件箱响应缺少仓库路径".into()))?;
        let mut parts = path.split('/');
        let owner = parts.next().unwrap_or_default();
        let repo = parts.next().unwrap_or_default();
        if owner.is_empty() || repo.is_empty() {
            return Err(AppError::Api("GitHub 收件箱响应中的仓库路径无效".into()));
        }
        Ok((owner.to_string(), repo.to_string()))
    }

    fn map_user(json: &Value) -> User {
        User {
            id: json["id"].clone(),
            login: json["login"].as_str().unwrap_or("").to_string(),
            name: json["name"].as_str().unwrap_or("").to_string(),
            avatar_url: json["avatar_url"].as_str().unwrap_or("").to_string(),
        }
    }

    fn map_pr_state(state_str: &str, merged: bool) -> PrState {
        if merged {
            PrState::Merged
        } else {
            match state_str {
                "closed" => PrState::Closed,
                _ => PrState::Open,
            }
        }
    }

    fn map_merge_queue_entry(entry: &Value, total: Option<u32>) -> PrMergeQueueStatus {
        let raw_state = entry["state"].as_str().unwrap_or("");
        let (state, failure_reason) = match raw_state {
            "QUEUED" => (MergeQueueState::Queued, None),
            "AWAITING_CHECKS" => (MergeQueueState::Waiting, None),
            "MERGEABLE" => (MergeQueueState::Ready, None),
            "LOCKED" => (MergeQueueState::Merging, None),
            "UNMERGEABLE" => (MergeQueueState::Blocked, Some("GitHub Merge Queue 判定当前项不可合并".into())),
            _ => (MergeQueueState::Unknown, None),
        };
        PrMergeQueueStatus {
            kind: MergeQueueKind::MergeQueue,
            available: true,
            state,
            position: entry["position"].as_u64().and_then(|value| u32::try_from(value).ok()),
            total,
            // The target branch lives on the PR node, so the caller populates it.
            target_branch: None,
            enqueued_at: entry["enqueuedAt"].as_str().map(str::to_string),
            updated_at: None,
            estimated_time_seconds: entry["estimatedTimeToMerge"].as_u64(),
            head_sha: entry["headCommit"]["oid"].as_str().map(str::to_string),
            pipeline_status: None,
            failure_reason,
        }
    }

    fn is_merge_queue_schema_error(message: &str) -> bool {
        message.to_ascii_lowercase().contains("mergequeue")
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
        let can_write = repository.ok().and_then(|repository| {
            let permissions = &repository["permissions"];
            let values = [
                permissions["admin"].as_bool(),
                permissions["maintain"].as_bool(),
                permissions["push"].as_bool(),
                permissions["triage"].as_bool(),
            ];
            values.iter().any(Option::is_some).then(|| values.contains(&Some(true)))
        });
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

    async fn resolve_milestone_number(&self, owner: &str, repo: &str, title: &str) -> Result<u64, AppError> {
        let encoded_title = urlencoding::encode(title);
        let url = format!(
            "{}/repos/{}/{}/milestones?state=all&per_page=100&title={}",
            self.base_url, owner, repo, encoded_title
        );
        let milestones = self.get_json::<Value>(&url).await?;
        milestones
            .as_array()
            .and_then(|items| {
                items.iter().find(|item| item["title"].as_str() == Some(title)).and_then(|item| item["number"].as_u64())
            })
            .ok_or_else(|| AppError::Api(format!("GitHub 仓库中不存在 Milestone：{title}")))
    }
}
#[async_trait]
impl super::JsonPageSource for GitHubAdapter {
    async fn fetch_json_page(&self, endpoint: &str, page: u32) -> Result<super::JsonPage, AppError> {
        let url = super::json_page_url(endpoint, page);
        let response = self
            .client
            .raw_client()
            .get(&url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;
        let status = response.status();
        let link = response.headers().get("link").and_then(|value| value.to_str().ok()).map(str::to_owned);
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {status} ({url}): {body}")));
        }
        let items = response.json().await?;
        Ok(super::JsonPage {
            items,
            next_page: super::next_page_from_link(link.as_deref()),
            pagination_known: link.is_some(),
        })
    }
}

#[async_trait]
impl super::CreateComparePageSource for GitHubAdapter {
    async fn fetch_create_compare_page(&self, endpoint: &str, page: u32) -> Result<Value, AppError> {
        self.get_json(&super::json_page_url(endpoint, page)).await
    }
}

#[async_trait]
impl GitPlatform for GitHubAdapter {
    fn name(&self) -> &'static str {
        "github"
    }

    async fn current_user(&self) -> Result<User, AppError> {
        let url = format!("{}/user", self.base_url);
        let json = self.get_json::<Value>(&url).await?;
        Ok(Self::map_user(&json))
    }

    async fn list_repos(&self, page: u32) -> Result<Paginated<RepoSummary>, AppError> {
        let url = format!("{}/user/repos?per_page=100&page={}", self.base_url, page);
        let resp = self
            .client
            .raw_client()
            .get(&url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;
        let total_pages = Self::parse_last_page(resp.headers().get("link").and_then(|value| value.to_str().ok()), page);
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {status} ({url}): {body}")));
        }
        let items: Vec<Value> = resp.json().await?;
        let total_count =
            if page == total_pages { (page.saturating_sub(1) * 100) + items.len() as u32 } else { total_pages * 100 };

        let mut repos: Vec<RepoSummary> = Vec::with_capacity(items.len());
        for r in &items {
            let full_name = r["full_name"].as_str().unwrap_or("");
            let parts: Vec<&str> = full_name.splitn(2, '/').collect();
            let fork = r["fork"].as_bool().unwrap_or(false);
            let (parent_full_name, parent_owner) = if fork {
                let mut pn = r["parent"]["full_name"].as_str().map(|s| s.to_string());
                let mut po = r["parent"]["owner"]["login"].as_str().map(|s| s.to_string());
                eprintln!("[mergebeacon] fork repo: {} parent_full_name={:?} parent_owner={:?}", full_name, pn, po);
                // Fallback: fetch repo detail if parent info missing from list endpoint
                if pn.is_none() || po.is_none() {
                    let detail_url = format!("{}/repos/{}", self.base_url, full_name);
                    if let Ok(detail) = self.get_json::<Value>(&detail_url).await {
                        pn = detail["parent"]["full_name"].as_str().map(|s| s.to_string());
                        po = detail["parent"]["owner"]["login"].as_str().map(|s| s.to_string());
                        eprintln!(
                            "[mergebeacon] fork repo fallback: {} parent_full_name={:?} parent_owner={:?}",
                            full_name, pn, po
                        );
                    }
                }
                (pn, po)
            } else {
                (None, None)
            };
            let owner_type = r["owner"]["type"].as_str().unwrap_or("user").to_lowercase();
            let owner_login = parts.first().unwrap_or(&"").to_string();
            repos.push(RepoSummary {
                id: r["id"].clone(),
                name: r["name"].as_str().unwrap_or("").to_string(),
                full_name: full_name.to_string(),
                owner: owner_login.clone(),
                owner_type,
                owner_display_name: owner_login,
                description: r["description"].as_str().unwrap_or("").to_string(),
                private: r["private"].as_bool().unwrap_or(false),
                fork,
                parent_full_name,
                parent_owner,
            });
        }

        // Fetch org display names from API
        self.resolve_org_display_names(&mut repos).await;

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
        // GitHub API only supports state=open|closed|all; "merged" is a subset of "closed"
        let api_state = match state {
            PrState::Merged => "closed",
            other => other.as_str(),
        };
        let url = format!(
            "{}/repos/{}/{}/pulls?state={}&per_page={}&page={}",
            self.base_url, owner, repo, api_state, per_page, page
        );

        // Use raw request to read Link header for pagination
        let resp = self
            .client
            .raw_client()
            .get(&url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        let link_header = resp.headers().get("link").and_then(|v| v.to_str().ok());
        let last_page = Self::parse_last_page(link_header, page);

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, body)));
        }

        let items: Vec<Value> = resp.json().await?;
        let node_ids = items
            .iter()
            .filter_map(|pr| Some((pr["number"].as_u64()?, pr["node_id"].as_str()?.to_string())))
            .collect::<std::collections::HashMap<_, _>>();

        let all_prs: Vec<PrSummary> = items
            .iter()
            .map(|pr| {
                let pr_state = Self::map_pr_state(pr["state"].as_str().unwrap_or(""), !pr["merged_at"].is_null());
                PrSummary {
                    number: pr["number"].as_u64().unwrap_or(0),
                    title: pr["title"].as_str().unwrap_or("").to_string(),
                    author: Self::map_user(&pr["user"]),
                    status: matches!(pr_state, PrState::Open).then(PrStatusSummary::default),
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

        // Filter by requested state (needed because GitHub groups merged into closed)
        let mut prs: Vec<PrSummary> = match state {
            PrState::Merged => all_prs.into_iter().filter(|p| matches!(p.state, PrState::Merged)).collect(),
            PrState::Closed => all_prs.into_iter().filter(|p| matches!(p.state, PrState::Closed)).collect(),
            _ => all_prs,
        };

        let requested_ids = prs
            .iter()
            .filter(|pr| matches!(pr.state, PrState::Open))
            .filter_map(|pr| node_ids.get(&pr.number).cloned())
            .collect::<Vec<_>>();
        if let Ok(statuses) = self.inbox_statuses(&requested_ids).await {
            for pr in &mut prs {
                if let Some(supplement) = node_ids.get(&pr.number).and_then(|node_id| statuses.get(node_id)) {
                    pr.status = Some(supplement.status.clone());
                }
            }
        }

        let total_count = if prs.is_empty() {
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
        let raw_items = match category {
            ReviewInboxCategory::ReviewRequested => {
                let (review_requests, assignments) = tokio::try_join!(
                    self.list_all_inbox_search("review-requested:@me"),
                    self.list_all_inbox_search("assignee:@me"),
                )?;
                review_requests
                    .into_iter()
                    .map(|item| (item, ReviewInboxRelationship::Reviewer))
                    .chain(assignments.into_iter().map(|item| (item, ReviewInboxRelationship::Assignee)))
                    .collect::<Vec<_>>()
            }
            ReviewInboxCategory::Authored => self
                .list_all_inbox_search("author:@me")
                .await?
                .into_iter()
                .map(|item| (item, ReviewInboxRelationship::Author))
                .collect(),
        };

        let mut node_ids = std::collections::HashMap::<(String, u64), String>::new();
        let mut items = Vec::with_capacity(raw_items.len());
        for (pr, relationship) in raw_items {
            let (owner, repo) = Self::repository_from_api_url(pr["repository_url"].as_str().unwrap_or(""))?;
            let repository_full_name = format!("{owner}/{repo}");
            let number = pr["number"].as_u64().unwrap_or(0);
            if let Some(node_id) = pr["node_id"].as_str() {
                node_ids.insert((repository_full_name.clone(), number), node_id.to_string());
            }
            items.push(ReviewInboxItem {
                platform: self.name().to_string(),
                repository_full_name,
                owner,
                repo,
                categories: vec![*category],
                relationships: vec![relationship],
                status: ReviewInboxStatusSummary::default(),
                head_sha: None,
                comments_count: None,
                summary: PrSummary {
                    number,
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
        let mut items = super::merge_review_inbox_items(items);
        items.sort_by(|left, right| right.summary.updated_at.cmp(&left.summary.updated_at));

        let total_count = items.len() as u32;
        let total_pages = if total_count == 0 { 1 } else { total_count.div_ceil(per_page) };
        let start = page.saturating_sub(1).saturating_mul(per_page) as usize;
        let mut page_items = items.into_iter().skip(start).take(per_page as usize).collect::<Vec<_>>();
        let requested_ids = page_items
            .iter()
            .filter_map(|item| node_ids.get(&(item.repository_full_name.clone(), item.summary.number)).cloned())
            .collect::<Vec<_>>();
        if let Ok(statuses) = self.inbox_statuses(&requested_ids).await {
            for item in &mut page_items {
                let key = (item.repository_full_name.clone(), item.summary.number);
                if let Some(supplement) = node_ids.get(&key).and_then(|node_id| statuses.get(node_id)) {
                    item.status = supplement.status.clone();
                    item.head_sha = supplement.head_sha.clone();
                    item.comments_count = supplement.comments_count;
                }
            }
        }

        Ok(Paginated { items: page_items, page, total_pages, total_count })
    }

    async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrDetail, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let json = self.get_json::<Value>(&url).await?;

        let summary = PrSummary {
            number: json["number"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["user"]),
            state: Self::map_pr_state(json["state"].as_str().unwrap_or(""), !json["merged_at"].is_null()),
            created_at: json["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: json["updated_at"].as_str().unwrap_or("").to_string(),
            labels: json["labels"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|l| l["name"].as_str().map(String::from)).collect())
                .unwrap_or_default(),
            status: None,
        };

        let metadata_permissions = self.metadata_permissions(owner, repo, &summary.author.login).await;
        Ok(PrDetail {
            summary,
            body: json["body"].as_str().unwrap_or("").to_string(),
            source_branch: json["head"]["ref"].as_str().unwrap_or("").to_string(),
            target_branch: json["base"]["ref"].as_str().unwrap_or("").to_string(),
            mergeable: json["mergeable"].as_bool(),
            head_sha: json["head"]["sha"].as_str().unwrap_or("").to_string(),
            base_sha: json["base"]["sha"].as_str().unwrap_or("").to_string(),
            draft: json["draft"].as_bool(),
            reviewers: json["requested_reviewers"]
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
        let repository = format!("{owner}/{repo}");
        let map_candidate = |pr: &Value| {
            let number = pr["number"].as_u64()?;
            let source_branch = pr["head"]["ref"].as_str()?.trim().to_string();
            let target_branch = pr["base"]["ref"].as_str()?.trim().to_string();
            if source_branch.is_empty() || target_branch.is_empty() {
                return None;
            }
            Some(PrDependencyCandidate {
                number,
                title: pr["title"].as_str().unwrap_or("").to_string(),
                state: Self::map_pr_state(pr["state"].as_str().unwrap_or(""), !pr["merged_at"].is_null()),
                source_branch,
                target_branch,
                source_repository: pr["head"]["repo"]["full_name"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("github-unknown-source:{number}")),
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

    async fn get_pr_merge_queue_status(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrMergeQueueStatus, AppError> {
        let number =
            i32::try_from(pr_number).map_err(|_| AppError::Api("GitHub PR 编号超出 GraphQL Int 范围".into()))?;
        let data = match self
            .graphql(
                "Merge Queue 状态查询",
                r#"query MergeQueueStatus($owner: String!, $repo: String!, $number: Int!) {
                    repository(owner: $owner, name: $repo) {
                        pullRequest(number: $number) {
                            baseRefName
                            headRefOid
                            mergeQueue {
                                entries { totalCount }
                            }
                            mergeQueueEntry {
                                position
                                state
                                enqueuedAt
                                estimatedTimeToMerge
                                headCommit { oid }
                            }
                        }
                    }
                }"#,
                serde_json::json!({ "owner": owner, "repo": repo, "number": number }),
            )
            .await
        {
            Ok(data) => data,
            Err(AppError::Api(message)) if Self::is_merge_queue_schema_error(&message) => {
                return Ok(PrMergeQueueStatus::unavailable(
                    MergeQueueKind::MergeQueue,
                    "当前 GitHub Enterprise 版本未提供 Merge Queue GraphQL 字段",
                ));
            }
            Err(error) => return Err(error),
        };
        let pull_request = data["repository"]["pullRequest"]
            .as_object()
            .ok_or_else(|| AppError::Api("GitHub Merge Queue 响应缺少 PR".into()))?;
        let total =
            pull_request["mergeQueue"]["entries"]["totalCount"].as_u64().and_then(|value| u32::try_from(value).ok());
        let mut status = match pull_request.get("mergeQueueEntry") {
            Some(entry) if !entry.is_null() => Self::map_merge_queue_entry(entry, total),
            _ if pull_request.get("mergeQueue").is_some_and(Value::is_object) => PrMergeQueueStatus {
                kind: MergeQueueKind::MergeQueue,
                available: true,
                state: MergeQueueState::NotQueued,
                position: None,
                total,
                target_branch: pull_request["baseRefName"].as_str().map(str::to_string),
                enqueued_at: None,
                updated_at: None,
                estimated_time_seconds: None,
                head_sha: pull_request["headRefOid"].as_str().map(str::to_string),
                pipeline_status: None,
                failure_reason: None,
            },
            _ => PrMergeQueueStatus::unavailable(MergeQueueKind::MergeQueue, "目标分支未启用 GitHub Merge Queue"),
        };
        if status.target_branch.is_none() {
            status.target_branch = pull_request["baseRefName"].as_str().map(str::to_string);
        }
        if status.head_sha.is_none() {
            status.head_sha = pull_request["headRefOid"].as_str().map(str::to_string);
        }
        Ok(status)
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
        let endpoint = format!("{}/repos/{}/{}/assignees", self.base_url, owner, repo);
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
                    "draft": request.draft,
                }),
            )
            .await?;
        json["number"].as_u64().ok_or_else(|| AppError::Api("GitHub 创建 PR 后未返回编号".into()))
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
            let files_json =
                json["files"].as_array().ok_or_else(|| AppError::Api("GitHub 提交响应缺少 files 字段".into()))?;
            let files = files_json.iter().map(Self::map_compare_file).collect();
            let diff = files_json.iter().map(Self::compare_unified_diff).filter(|patch| !patch.is_empty()).collect();
            let commit = &json["commit"];
            let summary = PrCommitSummary {
                sha: json["sha"].as_str().unwrap_or(commit_sha).to_string(),
                title: commit["message"].as_str().unwrap_or("").lines().next().unwrap_or("").to_string(),
                author_name: commit["author"]["name"].as_str().unwrap_or("").to_string(),
                authored_at: commit["author"]["date"].as_str().unwrap_or("").to_string(),
            };
            return Ok(PrCreatePreviewData {
                commits: vec![summary],
                diff,
                files,
                incomplete: false,
                incomplete_reasons: vec![],
            });
        }
        let base = urlencoding::encode(&request.target_branch);
        let head_reference = if request.source_owner == owner && request.source_repo == repo {
            request.source_branch.clone()
        } else {
            format!("{}:{}", request.source_owner, request.source_branch)
        };
        let head = urlencoding::encode(&head_reference);
        let endpoint = format!("{}/repos/{}/{}/compare/{}...{}", self.base_url, owner, repo, base, head);
        let collection =
            super::collect_create_compare_pages(self, &endpoint, "GitHub", "GitHub compare 响应缺少 files 字段")
                .await?;
        let files_json = &collection.files;
        let files = files_json.iter().map(Self::map_compare_file).collect();
        let diff = files_json.iter().map(Self::compare_unified_diff).filter(|patch| !patch.is_empty()).collect();
        let commits = collection
            .commits
            .iter()
            .filter_map(|commit| {
                let sha = commit["sha"].as_str()?.to_string();
                let message = commit["commit"]["message"].as_str().unwrap_or("");
                Some(PrCommitSummary {
                    sha,
                    title: message.lines().next().unwrap_or("").to_string(),
                    author_name: commit["commit"]["author"]["name"]
                        .as_str()
                        .or_else(|| commit["author"]["login"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    authored_at: commit["commit"]["author"]["date"].as_str().unwrap_or("").to_string(),
                })
            })
            .collect();
        Ok(PrCreatePreviewData {
            commits,
            diff,
            files,
            incomplete: collection.incomplete,
            incomplete_reasons: collection.incomplete_reasons,
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
        let pull_url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        if current.summary.title != update.title || current.body != update.body {
            match self.patch_json(&pull_url, &serde_json::json!({ "title": update.title, "body": update.body })).await {
                Ok(_) => result.updated_fields.push(PrMetadataField::TitleBody),
                Err(error) => result
                    .failures
                    .push(PrMetadataUpdateFailure { field: PrMetadataField::TitleBody, message: error.to_string() }),
            }
        }

        if update.draft.is_some() && current.draft != update.draft {
            let pull_request_id = self.pull_request_node_id(owner, repo, pr_number).await;
            let draft_result = match (pull_request_id, update.draft) {
                (Ok(pull_request_id), Some(true)) => self
                    .graphql(
                        "转换为 Draft",
                        r#"mutation ConvertPullRequestToDraft($pullRequestId: ID!) {
                            convertPullRequestToDraft(input: { pullRequestId: $pullRequestId }) {
                                pullRequest { id isDraft }
                            }
                        }"#,
                        serde_json::json!({ "pullRequestId": pull_request_id }),
                    )
                    .await
                    .map(|_| ()),
                (Ok(pull_request_id), Some(false)) => self
                    .graphql(
                        "标记为 Ready",
                        r#"mutation MarkPullRequestReadyForReview($pullRequestId: ID!) {
                            markPullRequestReadyForReview(input: { pullRequestId: $pullRequestId }) {
                                pullRequest { id isDraft }
                            }
                        }"#,
                        serde_json::json!({ "pullRequestId": pull_request_id }),
                    )
                    .await
                    .map(|_| ()),
                (Err(error), _) => Err(error),
                (_, None) => Ok(()),
            };
            match draft_result {
                Ok(()) => result.updated_fields.push(PrMetadataField::Draft),
                Err(error) => result
                    .failures
                    .push(PrMetadataUpdateFailure { field: PrMetadataField::Draft, message: error.to_string() }),
            }
        }

        let current_reviewers =
            current.reviewers.iter().map(|user| user.login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let target_reviewers =
            update.reviewers.iter().map(|login| login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        if current_reviewers != target_reviewers {
            let reviewers_url = format!("{pull_url}/requested_reviewers");
            let removed = current
                .reviewers
                .iter()
                .filter(|user| !target_reviewers.contains(&user.login.to_lowercase()))
                .map(|user| user.login.clone())
                .collect::<Vec<_>>();
            let added = update
                .reviewers
                .iter()
                .filter(|login| !current_reviewers.contains(&login.to_lowercase()))
                .cloned()
                .collect::<Vec<_>>();
            let remove_result = if removed.is_empty() {
                Ok(Value::Null)
            } else {
                self.delete_json(&reviewers_url, &serde_json::json!({ "reviewers": removed })).await
            };
            let add_result = if added.is_empty() {
                Ok(Value::Null)
            } else {
                self.post_json(&reviewers_url, &serde_json::json!({ "reviewers": added })).await
            };
            match remove_result.and(add_result) {
                Ok(_) => result.updated_fields.push(PrMetadataField::Reviewers),
                Err(error) => result
                    .failures
                    .push(PrMetadataUpdateFailure { field: PrMetadataField::Reviewers, message: error.to_string() }),
            }
        }

        let assignees_changed =
            current.assignees.iter().map(|user| user.login.to_lowercase()).collect::<std::collections::BTreeSet<_>>()
                != update.assignees.iter().map(|login| login.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let labels_changed =
            current.summary.labels.iter().map(|label| label.to_lowercase()).collect::<std::collections::BTreeSet<_>>()
                != update.labels.iter().map(|label| label.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
        let milestone_changed =
            current.milestone.as_ref().map(|milestone| milestone.title.as_str()) != update.milestone.as_deref();
        if assignees_changed || labels_changed || milestone_changed {
            let mut payload = serde_json::Map::new();
            if assignees_changed {
                payload.insert("assignees".into(), serde_json::json!(update.assignees));
            }
            if labels_changed {
                payload.insert("labels".into(), serde_json::json!(update.labels));
            }

            let milestone_update_available = if milestone_changed {
                match update.milestone.as_deref() {
                    Some(title) if current.milestone.as_ref().is_some_and(|milestone| milestone.title == title) => {
                        if let Some(number) = current.milestone.as_ref().and_then(|milestone| milestone.number) {
                            payload.insert("milestone".into(), serde_json::json!(number));
                            true
                        } else {
                            result.failures.push(PrMetadataUpdateFailure {
                                field: PrMetadataField::Milestone,
                                message: "当前 Milestone 缺少可用编号，请重新加载详情".into(),
                            });
                            false
                        }
                    }
                    Some(title) => match self.resolve_milestone_number(owner, repo, title).await {
                        Ok(number) => {
                            payload.insert("milestone".into(), serde_json::json!(number));
                            true
                        }
                        Err(error) => {
                            result.failures.push(PrMetadataUpdateFailure {
                                field: PrMetadataField::Milestone,
                                message: error.to_string(),
                            });
                            false
                        }
                    },
                    None => {
                        payload.insert("milestone".into(), serde_json::Value::Null);
                        true
                    }
                }
            } else {
                false
            };

            if !payload.is_empty() {
                let issue_url = format!("{}/repos/{}/{}/issues/{}", self.base_url, owner, repo, pr_number);
                match self.patch_json(&issue_url, &serde_json::Value::Object(payload)).await {
                    Ok(_) => {
                        if assignees_changed {
                            result.updated_fields.push(PrMetadataField::Assignees);
                        }
                        if labels_changed {
                            result.updated_fields.push(PrMetadataField::Labels);
                        }
                        if milestone_changed && milestone_update_available {
                            result.updated_fields.push(PrMetadataField::Milestone);
                        }
                    }
                    Err(error) => {
                        for field in [
                            assignees_changed.then_some(PrMetadataField::Assignees),
                            labels_changed.then_some(PrMetadataField::Labels),
                            (milestone_changed && milestone_update_available).then_some(PrMetadataField::Milestone),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            result.failures.push(PrMetadataUpdateFailure { field, message: error.to_string() });
                        }
                    }
                }
            }
        }
        Ok(result)
    }

    async fn get_merge_readiness(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrMergeReadiness, AppError> {
        let pull_url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let pull = self.get_json::<Value>(&pull_url).await?;
        let head_sha = pull["head"]["sha"].as_str().unwrap_or("").to_string();
        let mergeable = pull["mergeable"].as_bool();
        let draft = pull["draft"].as_bool();
        let mergeable_state = pull["mergeable_state"].as_str().unwrap_or("");
        let has_conflicts = match mergeable_state {
            "dirty" => Some(true),
            "clean" | "unstable" | "blocked" | "behind" => Some(false),
            _ if mergeable == Some(true) => Some(false),
            _ => None,
        };
        let branch_behind = (!mergeable_state.is_empty()).then_some(mergeable_state == "behind");
        let mut reasons = Vec::new();
        if pull["state"].as_str() != Some("open") {
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
        if branch_behind == Some(true) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::BranchBehind,
                message: "源分支落后于目标分支".into(),
            });
        }
        if mergeable_state == "blocked" {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::PlatformBlocked,
                message: "GitHub 分支保护规则阻止合并".into(),
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

        let legacy_status_url = format!("{}/repos/{}/{}/commits/{}/status", self.base_url, owner, repo, head_sha);
        let legacy_status = match self.get_json::<Value>(&legacy_status_url).await {
            Ok(status) => Self::legacy_commit_status(&status),
            Err(_) => Some(ReadinessState::Unknown),
        };
        let check_runs_url = format!(
            "{}/repos/{}/{}/commits/{}/check-runs?filter=latest&per_page=100",
            self.base_url, owner, repo, head_sha
        );
        let check_runs_status = match self.get_json::<Value>(&check_runs_url).await {
            Ok(check_runs) => Self::check_runs_status(&check_runs),
            Err(_) => Some(ReadinessState::Unknown),
        };
        let checks_status = Self::combined_checks_status(legacy_status, check_runs_status);
        match checks_status {
            ReadinessState::Blocked => reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::ChecksFailed,
                message: "CI 检查未通过".into(),
            }),
            ReadinessState::Pending => reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::ChecksPending,
                message: "CI 检查仍在进行中".into(),
            }),
            ReadinessState::Ready | ReadinessState::Unknown => {}
        }

        let reviews_url = format!("{}/repos/{}/{}/pulls/{}/reviews", self.base_url, owner, repo, pr_number);
        let approvals_status = match self.get_json::<Vec<Value>>(&reviews_url).await {
            Ok(reviews) => {
                let mut latest = std::collections::HashMap::<String, String>::new();
                for review in reviews {
                    let login = review["user"]["login"].as_str().unwrap_or("").to_string();
                    let state = review["state"].as_str().unwrap_or("").to_uppercase();
                    if !login.is_empty() && !state.is_empty() {
                        latest.insert(login, state);
                    }
                }
                let approved = latest.values().filter(|state| state.as_str() == "APPROVED").count() as u32;
                if latest.values().any(|state| state.as_str() == "CHANGES_REQUESTED") {
                    reasons.push(MergeBlockingReason {
                        code: MergeBlockingReasonCode::ChangesRequested,
                        message: "已有评审请求修改".into(),
                    });
                    ReadinessState::Blocked
                } else {
                    // GitHub API does not expose the branch-rule approval threshold here.
                    // A successful review lookup with no change request is safe to display as ready.
                    let _ = approved;
                    ReadinessState::Ready
                }
            }
            Err(_) => ReadinessState::Unknown,
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
            approvals_required: None,
            approvals_received: match approvals_status {
                ReadinessState::Unknown => None,
                _ => Some(0),
            },
            has_merge_permission,
            branch_behind,
            blocking_reasons: reasons,
        })
    }

    async fn get_pr_diff(&self, owner: &str, repo: &str, pr_number: u64) -> Result<(String, Vec<PrFile>), AppError> {
        // Get unified diff
        let diff_url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let diff = self.get_text(&diff_url).await?;

        // Get files list
        let files_url = format!("{}/repos/{}/{}/pulls/{}/files?per_page=300", self.base_url, owner, repo, pr_number);
        let files_json: Vec<Value> = self.get_json(&files_url).await?;

        let files: Vec<PrFile> = files_json
            .iter()
            .map(|f| PrFile {
                filename: f["filename"].as_str().unwrap_or("").to_string(),
                status: match f["status"].as_str().unwrap_or("") {
                    "added" => FileStatus::Added,
                    "removed" => FileStatus::Removed,
                    "renamed" => FileStatus::Renamed,
                    _ => FileStatus::Modified,
                },
                patch: f["patch"].as_str().unwrap_or("").to_string(),
                additions: f["additions"].as_u64().unwrap_or(0) as u32,
                deletions: f["deletions"].as_u64().unwrap_or(0) as u32,
            })
            .collect();

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
        let url = format!("{}/repos/{}/{}/compare/{}...{}?per_page=100", self.base_url, owner, repo, base, head);
        let json = self.get_json::<Value>(&url).await?;
        let files_json =
            json["files"].as_array().ok_or_else(|| AppError::Api("GitHub compare 响应缺少 files 字段".into()))?;
        let files: Vec<PrFile> = files_json.iter().map(Self::map_compare_file).collect();
        let diff = files_json.iter().map(Self::compare_unified_diff).filter(|patch| !patch.is_empty()).collect();
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
        let json = self.get_json::<Value>(&url).await?;
        crate::file_content::decode_response("GitHub", path, revision, &json)
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
        let url = format!("{}/repos/{}/{}/pulls/{}/reviews", self.base_url, owner, repo, pr_number);
        let event_str = match event {
            ReviewEvent::Approve => "APPROVE",
            ReviewEvent::Comment => "COMMENT",
            ReviewEvent::RequestChanges => "REQUEST_CHANGES",
        };

        let payload = if comments.is_empty() {
            serde_json::json!({
                "body": body,
                "event": event_str,
            })
        } else {
            let gh_comments: Vec<serde_json::Value> = comments
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "path": c.path,
                        "position": c.position,
                        "body": c.body,
                    })
                })
                .collect();
            serde_json::json!({
                "body": body,
                "event": event_str,
                "comments": gh_comments,
            })
        };

        let json = self.post_json(&url, &payload).await?;

        Ok(Review {
            id: json["id"].clone(),
            body: json["body"].as_str().unwrap_or("").to_string(),
            state: json["state"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["user"]),
            submitted_at: json["submitted_at"].as_str().unwrap_or("").to_string(),
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
        let gh_side = match side {
            "left" => "LEFT",
            _ => "RIGHT",
        };
        let payload = if let Some(sl) = start_line {
            serde_json::json!({
                "body": body,
                "commit_id": commit_id,
                "path": path,
                "start_line": sl,
                "line": line,
                "side": gh_side,
            })
        } else {
            serde_json::json!({
                "body": body,
                "commit_id": commit_id,
                "path": path,
                "line": line,
                "side": gh_side,
            })
        };
        let c: Value = self.post_json(&url, &payload).await?;
        let comment_id = c["id"].as_str().map(str::to_string).unwrap_or_else(|| c["id"].to_string());
        Ok(PrComment {
            id: c["id"].clone(),
            body: c["body"].as_str().unwrap_or("").to_string(),
            path: c["path"].as_str().unwrap_or("").to_string(),
            line: c["line"].as_u64().map(|n| n as u32),
            start_line: c["start_line"].as_u64().map(|n| n as u32),
            side: Some(if gh_side == "LEFT" { "left" } else { "right" }.into()),
            author: Self::map_user(&c["user"]),
            created_at: c["created_at"].as_str().unwrap_or("").to_string(),
            commit_id: c["commit_id"].as_str().map(|s| s.to_string()),
            original_commit_id: c["original_commit_id"].as_str().map(|s| s.to_string()),
            original_line: c["original_line"].as_u64().map(|n| n as u32),
            original_start_line: c["original_start_line"].as_u64().map(|n| n as u32),
            diff_hunk: c["diff_hunk"].as_str().map(|s| s.to_string()),
            thread_id: comment_id,
            reply_to_id: None,
            resolved: None,
            resolvable: false,
            can_edit: true,
            can_delete: true,
        })
    }

    async fn list_pr_comments(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<PrComment>, AppError> {
        let number =
            i32::try_from(pr_number).map_err(|_| AppError::Api("GitHub PR 编号超出 GraphQL Int 范围".into()))?;
        let mut comments = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let data = self
                .graphql(
                    "评审线程查询",
                    r#"query ReviewThreads($owner: String!, $repo: String!, $number: Int!, $after: String) {
                        repository(owner: $owner, name: $repo) {
                            pullRequest(number: $number) {
                                reviewThreads(first: 100, after: $after) {
                                    nodes {
                                        id
                                        isResolved
                                        viewerCanResolve
                                        viewerCanUnresolve
                                        diffSide
                                        comments(first: 100) {
                                            nodes {
                                                id
                                                fullDatabaseId
                                                body
                                                path
                                                line
                                                startLine
                                                author {
                                                    login
                                                    avatarUrl
                                                    ... on User { id name }
                                                }
                                                createdAt
                                                commit { oid }
                                                originalCommit { oid }
                                                originalLine
                                                originalStartLine
                                                diffHunk
                                                replyTo { id fullDatabaseId }
                                            }
                                            pageInfo { hasNextPage endCursor }
                                        }
                                    }
                                    pageInfo { hasNextPage endCursor }
                                }
                            }
                        }
                    }"#,
                    serde_json::json!({
                        "owner": owner,
                        "repo": repo,
                        "number": number,
                        "after": after,
                    }),
                )
                .await?;
            let review_threads = data["repository"]["pullRequest"]["reviewThreads"]
                .as_object()
                .ok_or_else(|| AppError::Api("GitHub 评审线程响应缺少 reviewThreads".into()))?;
            let threads = review_threads["nodes"]
                .as_array()
                .ok_or_else(|| AppError::Api("GitHub 评审线程响应缺少 nodes".into()))?;
            for thread in threads {
                let thread_id = thread["id"]
                    .as_str()
                    .filter(|thread_id| !thread_id.is_empty())
                    .ok_or_else(|| AppError::Api("GitHub 评审线程响应缺少线程 ID".into()))?;
                let mut comment_page = thread["comments"].clone();
                loop {
                    let page_comments = comment_page["nodes"]
                        .as_array()
                        .ok_or_else(|| AppError::Api("GitHub 评审线程响应缺少评论 nodes".into()))?;
                    for comment in page_comments {
                        comments.push(Self::map_graphql_review_comment(thread, comment)?);
                    }
                    if comment_page["pageInfo"]["hasNextPage"].as_bool() != Some(true) {
                        break;
                    }
                    let cursor = comment_page["pageInfo"]["endCursor"]
                        .as_str()
                        .filter(|cursor| !cursor.is_empty())
                        .ok_or_else(|| AppError::Api("GitHub 评审线程评论分页响应缺少游标".into()))?;
                    comment_page = self.review_thread_comments_page(thread_id, cursor).await?;
                }
            }

            if review_threads["pageInfo"]["hasNextPage"].as_bool() != Some(true) {
                break;
            }
            after = Some(
                review_threads["pageInfo"]["endCursor"]
                    .as_str()
                    .filter(|cursor| !cursor.is_empty())
                    .ok_or_else(|| AppError::Api("GitHub 评审线程分页响应缺少游标".into()))?
                    .to_string(),
            );
        }
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
        let url =
            format!("{}/repos/{}/{}/pulls/{}/comments/{}/replies", self.base_url, owner, repo, pr_number, reply_to_id);
        self.post_json(&url, &serde_json::json!({ "body": body })).await?;
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

    async fn set_review_thread_resolved(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
        thread_id: &str,
        resolved: bool,
    ) -> Result<(), AppError> {
        let (operation, mutation, result_field) = if resolved {
            (
                "解决评审线程",
                r#"mutation ResolveReviewThread($threadId: ID!) {
                    resolveReviewThread(input: { threadId: $threadId }) {
                        thread { id isResolved }
                    }
                }"#,
                "resolveReviewThread",
            )
        } else {
            (
                "重新打开评审线程",
                r#"mutation UnresolveReviewThread($threadId: ID!) {
                    unresolveReviewThread(input: { threadId: $threadId }) {
                        thread { id isResolved }
                    }
                }"#,
                "unresolveReviewThread",
            )
        };
        let data = self.graphql(operation, mutation, serde_json::json!({ "threadId": thread_id })).await?;
        let thread = &data[result_field]["thread"];
        if thread["id"].as_str() != Some(thread_id) || thread["isResolved"].as_bool() != Some(resolved) {
            return Err(AppError::Api(format!("GitHub {operation}响应与请求状态不一致")));
        }
        Ok(())
    }

    async fn list_viewed_pr_files(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<String>, AppError> {
        let number =
            i32::try_from(pr_number).map_err(|_| AppError::Api("GitHub PR 编号超出 GraphQL Int 范围".into()))?;
        let mut viewed_files = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let data = self
                .graphql(
                    "文件已查看状态查询",
                    r#"query PullRequestViewedFiles(
                        $owner: String!
                        $repo: String!
                        $number: Int!
                        $after: String
                    ) {
                        repository(owner: $owner, name: $repo) {
                            pullRequest(number: $number) {
                                files(first: 100, after: $after) {
                                    nodes { path viewerViewedState }
                                    pageInfo { hasNextPage endCursor }
                                }
                            }
                        }
                    }"#,
                    serde_json::json!({
                        "owner": owner,
                        "repo": repo,
                        "number": number,
                        "after": after,
                    }),
                )
                .await?;
            let files = data["repository"]["pullRequest"]["files"]
                .as_object()
                .ok_or_else(|| AppError::Api("GitHub 文件已查看状态响应缺少 files".into()))?;
            let nodes =
                files["nodes"].as_array().ok_or_else(|| AppError::Api("GitHub 文件已查看状态响应缺少 nodes".into()))?;
            for file in nodes {
                let path = file["path"]
                    .as_str()
                    .filter(|path| !path.is_empty())
                    .ok_or_else(|| AppError::Api("GitHub 文件已查看状态响应缺少路径".into()))?;
                let viewed_state = file["viewerViewedState"]
                    .as_str()
                    .ok_or_else(|| AppError::Api("GitHub 文件已查看状态响应缺少状态".into()))?;
                if viewed_state == "VIEWED" {
                    viewed_files.push(path.to_string());
                }
            }
            if files["pageInfo"]["hasNextPage"].as_bool() != Some(true) {
                break;
            }
            after = Some(
                files["pageInfo"]["endCursor"]
                    .as_str()
                    .filter(|cursor| !cursor.is_empty())
                    .ok_or_else(|| AppError::Api("GitHub 文件已查看状态分页响应缺少游标".into()))?
                    .to_string(),
            );
        }
        Ok(viewed_files)
    }

    async fn set_pr_file_viewed(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        path: &str,
        viewed: bool,
    ) -> Result<(), AppError> {
        let pull_request_id = self.pull_request_node_id(owner, repo, pr_number).await?;
        let (operation, mutation, result_field) = if viewed {
            (
                "标记文件为已查看",
                r#"mutation MarkFileAsViewed($pullRequestId: ID!, $path: String!) {
                    markFileAsViewed(input: { pullRequestId: $pullRequestId, path: $path }) {
                        pullRequest { id }
                    }
                }"#,
                "markFileAsViewed",
            )
        } else {
            (
                "标记文件为未查看",
                r#"mutation UnmarkFileAsViewed($pullRequestId: ID!, $path: String!) {
                    unmarkFileAsViewed(input: { pullRequestId: $pullRequestId, path: $path }) {
                        pullRequest { id }
                    }
                }"#,
                "unmarkFileAsViewed",
            )
        };
        let data = self
            .graphql(operation, mutation, serde_json::json!({ "pullRequestId": pull_request_id, "path": path }))
            .await?;
        if data[result_field]["pullRequest"]["id"].as_str() != Some(pull_request_id.as_str()) {
            return Err(AppError::Api(format!("GitHub {operation}响应缺少匹配的 PR")));
        }
        Ok(())
    }

    async fn list_reviews(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<Review>, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}/reviews", self.base_url, owner, repo, pr_number);
        let items: Vec<Value> = self.get_json(&url).await?;

        let reviews = items
            .iter()
            .map(|r| Review {
                id: r["id"].clone(),
                body: r["body"].as_str().unwrap_or("").to_string(),
                state: r["state"].as_str().unwrap_or("").to_string(),
                author: Self::map_user(&r["user"]),
                submitted_at: r["submitted_at"].as_str().unwrap_or("").to_string(),
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
        let items: Vec<Value> = self.get_json(&url).await?;

        let issues: Vec<IssueSummary> = items
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
        let url = format!("{}/repos/{}/{}/issues", self.base_url, owner, repo);
        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "labels": labels,
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
        let url = format!("{}/repos/{}/{}/pulls/{}/merge", self.base_url, owner, repo, pr_number);
        let method = match strategy {
            MergeStrategy::Merge => "merge",
            MergeStrategy::Squash => "squash",
            MergeStrategy::Rebase => "rebase",
        };
        let mut payload = serde_json::json!({
            "merge_method": method,
            "sha": sha,
        });
        if let Some(t) = commit_title {
            payload["commit_title"] = serde_json::Value::String(t);
        }
        if let Some(m) = commit_message {
            payload["commit_message"] = serde_json::Value::String(m);
        }
        let json = self.put_json(&url, &payload).await?;
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
        let state = Self::map_pr_state(json["state"].as_str().unwrap_or(""), !json["merged_at"].is_null());
        Ok(state)
    }

    async fn reopen_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let payload = serde_json::json!({ "state": "open" });
        let json = self.patch_json(&url, &payload).await?;
        let state = Self::map_pr_state(json["state"].as_str().unwrap_or(""), !json["merged_at"].is_null());
        Ok(state)
    }

    async fn close_issue(&self, owner: &str, repo: &str, issue_number: u64) -> Result<(), AppError> {
        let url = format!("{}/repos/{}/{}/issues/{}", self.base_url, owner, repo, issue_number);
        let payload = serde_json::json!({ "state": "closed" });
        self.patch_json(&url, &payload).await?;
        Ok(())
    }
}
