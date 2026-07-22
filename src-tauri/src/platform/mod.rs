pub mod gitee;
pub mod github;
pub mod gitlab;

use crate::error::AppError;
use crate::models::*;
use async_trait::async_trait;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlatformCapabilities {
    pub platform: &'static str,
    /// 平台 API 协议本身支持的评审事件。
    pub review_events: Vec<ReviewEvent>,
    /// 平台 API 协议本身支持的合并策略。
    pub merge_strategies: Vec<MergeStrategy>,
    pub supports_fork_context: bool,
    pub supports_issue_auto_close: bool,
    /// 平台是否提供可用于增量评审的可靠 base/head compare API。
    pub supports_compare_diff: bool,
    /// 平台 API 是否支持解决和重新打开评审线程。
    pub supports_review_thread_resolution: bool,
    /// 平台公开 API 是否支持读取和写入文件已查看状态。
    pub supports_remote_file_viewed_state: bool,
    pub supports_pr_title_body_edit: bool,
    pub supports_pr_draft_toggle: bool,
    pub supports_pr_reviewer_management: bool,
    pub supports_pr_assignee_management: bool,
    pub supports_pr_label_management: bool,
    pub supports_pr_milestone_management: bool,
    pub supports_pr_creation: bool,
    /// 平台原生只读合并队列类型；None 表示平台协议不提供该能力。
    pub merge_queue_kind: Option<MergeQueueKind>,
}

/// 平台协议能力的唯一静态定义入口。
///
/// 这里不表达用户是否登录、Token 是否具备权限、PR 是否可合并等运行时状态；
/// 前端展示状态应由协议能力与运行时状态共同派生。
pub fn capabilities_for(platform: &str) -> Option<PlatformCapabilities> {
    let capabilities = match platform {
        "github" => PlatformCapabilities {
            platform: "github",
            review_events: vec![ReviewEvent::Comment, ReviewEvent::Approve, ReviewEvent::RequestChanges],
            merge_strategies: vec![MergeStrategy::Merge, MergeStrategy::Squash, MergeStrategy::Rebase],
            supports_fork_context: true,
            supports_issue_auto_close: true,
            supports_compare_diff: true,
            supports_review_thread_resolution: true,
            supports_remote_file_viewed_state: true,
            supports_pr_title_body_edit: true,
            supports_pr_draft_toggle: true,
            supports_pr_reviewer_management: true,
            supports_pr_assignee_management: true,
            supports_pr_label_management: true,
            supports_pr_milestone_management: true,
            supports_pr_creation: true,
            merge_queue_kind: Some(MergeQueueKind::MergeQueue),
        },
        "gitlab" => PlatformCapabilities {
            platform: "gitlab",
            review_events: vec![ReviewEvent::Comment],
            merge_strategies: vec![MergeStrategy::Merge, MergeStrategy::Squash],
            supports_fork_context: true,
            supports_issue_auto_close: true,
            supports_compare_diff: true,
            supports_review_thread_resolution: true,
            supports_remote_file_viewed_state: false,
            supports_pr_title_body_edit: true,
            supports_pr_draft_toggle: true,
            supports_pr_reviewer_management: true,
            supports_pr_assignee_management: true,
            supports_pr_label_management: true,
            supports_pr_milestone_management: true,
            supports_pr_creation: true,
            merge_queue_kind: Some(MergeQueueKind::MergeTrain),
        },
        "gitee" => PlatformCapabilities {
            platform: "gitee",
            review_events: vec![ReviewEvent::Comment],
            merge_strategies: vec![MergeStrategy::Merge, MergeStrategy::Squash, MergeStrategy::Rebase],
            supports_fork_context: true,
            supports_issue_auto_close: true,
            supports_compare_diff: true,
            supports_review_thread_resolution: false,
            supports_remote_file_viewed_state: false,
            supports_pr_title_body_edit: true,
            supports_pr_draft_toggle: false,
            supports_pr_reviewer_management: true,
            supports_pr_assignee_management: true,
            supports_pr_label_management: true,
            supports_pr_milestone_management: true,
            supports_pr_creation: true,
            merge_queue_kind: None,
        },
        _ => return None,
    };
    Some(capabilities)
}

/// Normalize a custom host URL to the API root expected by each platform.
pub fn normalize_api_base(platform: &str, url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let suffix = match platform {
        "gitlab" => "/api/v4",
        "gitee" => "/api/v5",
        _ => return trimmed.to_string(),
    };
    if trimmed.ends_with(suffix) {
        trimmed.to_string()
    } else {
        format!("{trimmed}{suffix}")
    }
}

const CREATE_COMPARE_COMMIT_PAGE_SIZE: usize = 100;
const CREATE_COMPARE_FILE_LIMIT: usize = 300;
const MAX_CREATE_COMPARE_PAGES: u32 = 100;

pub(crate) struct CreateCompareCollection {
    pub commits: Vec<serde_json::Value>,
    pub files: Vec<serde_json::Value>,
    pub incomplete: bool,
    pub incomplete_reasons: Vec<PrCreatePreviewIncompleteReason>,
}

#[async_trait]
pub(crate) trait CreateComparePageSource {
    async fn fetch_create_compare_page(&self, endpoint: &str, page: u32) -> Result<serde_json::Value, AppError>;
}

fn compare_commit_identity(commit: &serde_json::Value) -> Option<&str> {
    commit["sha"].as_str().or_else(|| commit["id"].as_str())
}

fn compare_file_identity(file: &serde_json::Value) -> Option<&str> {
    file["filename"].as_str().or_else(|| file["new_path"].as_str()).or_else(|| file["old_path"].as_str())
}

fn compare_files(json: &serde_json::Value) -> Option<&Vec<serde_json::Value>> {
    json["files"].as_array().or_else(|| json["changes"].as_array())
}

fn reported_compare_commit_count(json: &serde_json::Value) -> Option<u64> {
    json["total_commits"].as_u64().into_iter().chain(json["ahead_by"].as_u64()).max()
}

fn add_compare_incomplete_reason(
    reasons: &mut Vec<PrCreatePreviewIncompleteReason>,
    reason: PrCreatePreviewIncompleteReason,
) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

fn compare_page_error_summary(error: &AppError) -> String {
    match error {
        AppError::Http(error) => {
            if let Some(status) = error.status() {
                format!("HTTP {}", status.as_u16())
            } else if error.is_timeout() {
                "请求超时".into()
            } else if error.is_connect() {
                "连接失败".into()
            } else {
                "HTTP 请求失败".into()
            }
        }
        AppError::Json(_) => "响应 JSON 无效".into(),
        AppError::Io(_) => "本地 IO 失败".into(),
        AppError::NotAuthenticated(_) => "认证失效".into(),
        AppError::Api(message) => [401_u16, 403, 404, 408, 409, 422, 429, 500, 502, 503, 504]
            .into_iter()
            .find(|status| {
                let expected = status.to_string();
                message.split(|character: char| !character.is_ascii_digit()).any(|part| part == expected)
            })
            .map(|status| format!("HTTP {status}"))
            .unwrap_or_else(|| "平台 API 请求失败".into()),
        AppError::UnsupportedStrategy(_) => "平台策略不支持".into(),
        AppError::Ai(_) => "AI 请求失败".into(),
        AppError::NotImplemented(_) => "平台能力不支持".into(),
        AppError::Unknown(_) => "未知错误".into(),
    }
}

pub(crate) async fn collect_create_compare_pages<S>(
    source: &S,
    endpoint: &str,
    platform_name: &str,
    missing_files_error: &str,
) -> Result<CreateCompareCollection, AppError>
where
    S: CreateComparePageSource + Sync,
{
    let first = source.fetch_create_compare_page(endpoint, 1).await?;
    let mut commits = first["commits"].as_array().cloned().unwrap_or_default();
    let mut files = compare_files(&first).cloned().ok_or_else(|| AppError::Api(missing_files_error.to_string()))?;
    let reported_commits = reported_compare_commit_count(&first);
    let mut commit_ids = commits
        .iter()
        .filter_map(compare_commit_identity)
        .map(str::to_string)
        .collect::<std::collections::HashSet<_>>();
    let mut file_ids =
        files.iter().filter_map(compare_file_identity).map(str::to_string).collect::<std::collections::HashSet<_>>();
    let mut page = 1_u32;
    let mut page_commit_count = commits.len();
    let mut incomplete_reasons = Vec::new();

    loop {
        let needs_next_page = reported_commits
            .map(|total| (commits.len() as u64) < total)
            .unwrap_or(page_commit_count >= CREATE_COMPARE_COMMIT_PAGE_SIZE);
        if !needs_next_page {
            break;
        }
        if page >= MAX_CREATE_COMPARE_PAGES {
            add_compare_incomplete_reason(&mut incomplete_reasons, PrCreatePreviewIncompleteReason::PaginationLimit);
            break;
        }

        page = page.saturating_add(1);
        let next = match source.fetch_create_compare_page(endpoint, page).await {
            Ok(next) => next,
            Err(error) => {
                eprintln!("{platform_name} Compare 补页失败（page={page}）：{}", compare_page_error_summary(&error));
                add_compare_incomplete_reason(
                    &mut incomplete_reasons,
                    PrCreatePreviewIncompleteReason::PaginationFailed,
                );
                break;
            }
        };
        let Some(page_commits) = next["commits"].as_array() else {
            eprintln!("{platform_name} Compare 补页失败（page={page}）：响应缺少 commits 字段");
            add_compare_incomplete_reason(&mut incomplete_reasons, PrCreatePreviewIncompleteReason::PaginationFailed);
            break;
        };
        page_commit_count = page_commits.len();
        let commit_count_before_page = commits.len();
        for commit in page_commits {
            let is_new = compare_commit_identity(commit).map(|id| commit_ids.insert(id.to_string())).unwrap_or(true);
            if is_new {
                commits.push(commit.clone());
            }
        }
        if let Some(page_files) = compare_files(&next) {
            for file in page_files {
                let is_new = compare_file_identity(file).map(|id| file_ids.insert(id.to_string())).unwrap_or(true);
                if is_new {
                    files.push(file.clone());
                }
            }
        }

        // Some compatible APIs accept page parameters but keep returning page one.
        if page_commit_count > 0 && commits.len() == commit_count_before_page {
            add_compare_incomplete_reason(&mut incomplete_reasons, PrCreatePreviewIncompleteReason::PlatformLimit);
            break;
        }

        if page_commit_count < CREATE_COMPARE_COMMIT_PAGE_SIZE {
            if reported_commits.is_some_and(|total| (commits.len() as u64) < total) {
                add_compare_incomplete_reason(&mut incomplete_reasons, PrCreatePreviewIncompleteReason::PlatformLimit);
            }
            break;
        }
    }

    let pagination_stopped_early = incomplete_reasons.iter().any(|reason| {
        matches!(
            reason,
            PrCreatePreviewIncompleteReason::PaginationFailed | PrCreatePreviewIncompleteReason::PaginationLimit
        )
    });
    if !pagination_stopped_early && reported_commits.is_some_and(|total| (commits.len() as u64) < total) {
        add_compare_incomplete_reason(&mut incomplete_reasons, PrCreatePreviewIncompleteReason::PlatformLimit);
    }
    if files.len() >= CREATE_COMPARE_FILE_LIMIT {
        add_compare_incomplete_reason(&mut incomplete_reasons, PrCreatePreviewIncompleteReason::PlatformLimit);
    }
    Ok(CreateCompareCollection { commits, files, incomplete: !incomplete_reasons.is_empty(), incomplete_reasons })
}

pub(crate) fn create_compare_is_incomplete(json: &serde_json::Value, commit_count: usize, file_count: usize) -> bool {
    let reported_commit_count = reported_compare_commit_count(json);
    let commits_incomplete = reported_commit_count
        .map(|total| total > commit_count as u64)
        .unwrap_or(commit_count >= CREATE_COMPARE_COMMIT_PAGE_SIZE);

    // Compare APIs cap the returned files without exposing a reliable file total.
    commits_incomplete || file_count >= CREATE_COMPARE_FILE_LIMIT
}

pub(crate) const JSON_PAGE_SIZE: usize = 100;
const MAX_JSON_PAGES: u32 = 1_000;
const MAX_DEPENDENCY_QUERY_PAGES: u32 = 2;
const MAX_DEPENDENCY_BRANCH_QUERIES: usize = 40;
const MAX_DEPENDENCY_CANDIDATES: usize = 200;

pub(crate) struct JsonPage {
    pub items: Vec<serde_json::Value>,
    pub next_page: Option<u32>,
    pub pagination_known: bool,
}

#[async_trait]
pub(crate) trait JsonPageSource {
    async fn fetch_json_page(&self, endpoint: &str, page: u32) -> Result<JsonPage, AppError>;
}

pub(crate) async fn collect_json_pages<S>(source: &S, endpoint: &str) -> Result<Vec<serde_json::Value>, AppError>
where
    S: JsonPageSource + Sync,
{
    let mut items = Vec::new();
    let mut page = 1_u32;
    loop {
        let result = source.fetch_json_page(endpoint, page).await?;
        let item_count = result.items.len();
        items.extend(result.items);
        let next_page = result
            .next_page
            .or_else(|| (!result.pagination_known && item_count == JSON_PAGE_SIZE).then(|| page.saturating_add(1)));
        let Some(next_page) = next_page else {
            break;
        };
        if next_page <= page {
            return Err(AppError::Api("远端分页游标未向前推进".into()));
        }
        if next_page > MAX_JSON_PAGES {
            return Err(AppError::Api(format!("远端分页超过安全上限（{MAX_JSON_PAGES} 页）")));
        }
        page = next_page;
    }
    Ok(items)
}

pub(crate) async fn collect_json_pages_limited<S>(
    source: &S,
    endpoint: &str,
    max_pages: u32,
) -> Result<(Vec<serde_json::Value>, bool), AppError>
where
    S: JsonPageSource + Sync,
{
    if max_pages == 0 {
        return Err(AppError::Api("分页上限必须大于 0".into()));
    }
    let mut items = Vec::new();
    let mut page = 1_u32;
    let mut pages_fetched = 0_u32;
    let max_pages = max_pages.min(MAX_JSON_PAGES);
    loop {
        let result = source.fetch_json_page(endpoint, page).await?;
        pages_fetched = pages_fetched.saturating_add(1);
        let item_count = result.items.len();
        items.extend(result.items);
        let next_page = result
            .next_page
            .or_else(|| (!result.pagination_known && item_count == JSON_PAGE_SIZE).then(|| page.saturating_add(1)));
        let Some(next_page) = next_page else {
            return Ok((items, false));
        };
        if next_page <= page {
            return Err(AppError::Api("远端分页游标未向前推进".into()));
        }
        if pages_fetched >= max_pages {
            return Ok((items, true));
        }
        page = next_page;
    }
}

pub(crate) async fn walk_pr_dependency_candidates<S, MapCandidate, NeighborEndpoints>(
    source: &S,
    current: PrDependencyCandidate,
    map_candidate: MapCandidate,
    neighbor_endpoints: NeighborEndpoints,
) -> Result<PrDependencyCandidates, AppError>
where
    S: JsonPageSource + Sync,
    MapCandidate: Fn(&serde_json::Value) -> Option<PrDependencyCandidate> + Sync,
    NeighborEndpoints: Fn(&PrDependencyCandidate) -> [String; 2] + Sync,
{
    let mut candidates = std::collections::BTreeMap::from([(current.number, current.clone())]);
    let mut queue = std::collections::VecDeque::from([current.clone()]);
    let mut queried = std::collections::BTreeSet::new();
    let mut truncated = false;

    'walk: while let Some(candidate) = queue.pop_front() {
        for endpoint in neighbor_endpoints(&candidate) {
            if !queried.insert(endpoint.clone()) {
                continue;
            }
            if queried.len() > MAX_DEPENDENCY_BRANCH_QUERIES {
                truncated = true;
                break 'walk;
            }
            let (items, page_limit_reached) =
                collect_json_pages_limited(source, &endpoint, MAX_DEPENDENCY_QUERY_PAGES).await?;
            truncated |= page_limit_reached;
            for item in items {
                let Some(candidate) = map_candidate(&item) else {
                    continue;
                };
                if candidates.contains_key(&candidate.number) {
                    continue;
                }
                if candidates.len() >= MAX_DEPENDENCY_CANDIDATES {
                    truncated = true;
                    break 'walk;
                }
                queue.push_back(candidate.clone());
                candidates.insert(candidate.number, candidate);
            }
        }
    }

    Ok(PrDependencyCandidates { current, items: candidates.into_values().collect(), truncated })
}

pub(crate) fn json_page_url(endpoint: &str, page: u32) -> String {
    let separator = if endpoint.contains('?') { '&' } else { '?' };
    format!("{endpoint}{separator}per_page={JSON_PAGE_SIZE}&page={page}")
}

pub(crate) fn next_page_from_link(link: Option<&str>) -> Option<u32> {
    let header = link?;
    header.split(',').find_map(|part| {
        let part = part.trim();
        if !part.contains(r#"rel="next""#) && !part.contains("rel='next'") {
            return None;
        }
        let start = part.find('<')? + 1;
        let end = part[start..].find('>')? + start;
        part[start..end]
            .split('?')
            .nth(1)?
            .split('&')
            .find_map(|segment| segment.strip_prefix("page=").and_then(|value| value.parse::<u32>().ok()))
    })
}

fn readiness_rank(state: ReadinessState) -> u8 {
    match state {
        ReadinessState::Blocked => 4,
        ReadinessState::Pending => 3,
        ReadinessState::Ready => 2,
        ReadinessState::Unknown => 1,
    }
}

fn merge_readiness_state(left: ReadinessState, right: ReadinessState) -> ReadinessState {
    if readiness_rank(right) > readiness_rank(left) {
        right
    } else {
        left
    }
}

fn merge_optional_flag(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), _) | (_, Some(false)) => Some(false),
        (None, None) => None,
    }
}

fn merge_inbox_status(left: &mut ReviewInboxStatusSummary, right: ReviewInboxStatusSummary) {
    left.status = merge_readiness_state(left.status, right.status);
    left.checks_status = merge_readiness_state(left.checks_status, right.checks_status);
    left.approvals_status = merge_readiness_state(left.approvals_status, right.approvals_status);
    left.draft = merge_optional_flag(left.draft, right.draft);
    left.has_conflicts = merge_optional_flag(left.has_conflicts, right.has_conflicts);
    for reason in right.blocking_reasons {
        if !left.blocking_reasons.contains(&reason) {
            left.blocking_reasons.push(reason);
        }
    }
}

pub(crate) fn merge_review_inbox_items(items: Vec<ReviewInboxItem>) -> Vec<ReviewInboxItem> {
    let mut merged = Vec::<ReviewInboxItem>::new();
    let mut indexes = std::collections::HashMap::<(String, u64), usize>::new();
    for item in items {
        let key = (item.repository_full_name.clone(), item.summary.number);
        if let Some(index) = indexes.get(&key).copied() {
            let existing = &mut merged[index];
            for category in item.categories {
                if !existing.categories.contains(&category) {
                    existing.categories.push(category);
                }
            }
            for relationship in item.relationships {
                if !existing.relationships.contains(&relationship) {
                    existing.relationships.push(relationship);
                }
            }
            merge_inbox_status(&mut existing.status, item.status);
            if existing.head_sha.is_none() {
                existing.head_sha = item.head_sha;
            }
            existing.comments_count = match (existing.comments_count, item.comments_count) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (left @ Some(_), None) => left,
                (None, right) => right,
            };
        } else {
            indexes.insert(key, merged.len());
            merged.push(item);
        }
    }
    merged
}

/// Common interface for all Git platforms (GitHub, GitLab, Gitee)
#[async_trait]
#[allow(clippy::too_many_arguments)]
pub trait GitPlatform: Send + Sync {
    fn name(&self) -> &'static str;

    // ── User ──
    async fn current_user(&self) -> Result<User, AppError>;
    async fn list_repos(&self, page: u32) -> Result<Paginated<RepoSummary>, AppError>;

    // ── PR ──
    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: &PrState,
        page: u32,
        per_page: u32,
    ) -> Result<Paginated<PrSummary>, AppError>;

    async fn list_review_inbox(
        &self,
        category: &ReviewInboxCategory,
        page: u32,
        per_page: u32,
    ) -> Result<Paginated<ReviewInboxItem>, AppError>;

    async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrDetail, AppError>;

    async fn list_pr_dependency_candidates(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrDependencyCandidates, AppError>;

    async fn get_pr_merge_queue_status(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
    ) -> Result<PrMergeQueueStatus, AppError> {
        Err(AppError::NotImplemented("当前平台不支持原生合并队列状态".into()))
    }

    async fn list_branches(&self, owner: &str, repo: &str) -> Result<PrBranchOptions, AppError>;

    async fn list_labels(&self, owner: &str, repo: &str) -> Result<Vec<PrLabel>, AppError>;

    async fn list_pr_participant_suggestions(&self, owner: &str, repo: &str) -> Result<Vec<User>, AppError>;

    async fn create_pull_request(&self, owner: &str, repo: &str, request: &PrCreateRequest) -> Result<u64, AppError>;

    async fn preview_pull_request(
        &self,
        owner: &str,
        repo: &str,
        request: &PrCreatePreviewRequest,
    ) -> Result<PrCreatePreviewData, AppError>;

    async fn update_pull_request_metadata(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        current: &PrDetail,
        update: &PrMetadataUpdate,
    ) -> Result<PrMetadataMutationResult, AppError>;

    async fn get_merge_readiness(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrMergeReadiness, AppError>;

    async fn get_pr_diff(&self, owner: &str, repo: &str, pr_number: u64) -> Result<(String, Vec<PrFile>), AppError>;

    async fn get_compare_diff(
        &self,
        owner: &str,
        repo: &str,
        base_sha: &str,
        head_sha: &str,
    ) -> Result<(String, Vec<PrFile>), AppError>;

    async fn get_pr_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        revision: &str,
    ) -> Result<PrFileContent, AppError>;

    // ── Review ──
    async fn create_review(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        body: &str,
        event: &ReviewEvent,
        comments: &[ReviewCommentPosition],
    ) -> Result<Review, AppError>;

    async fn list_reviews(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<Review>, AppError>;

    // ── PR Comment ──
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
    ) -> Result<PrComment, AppError>;

    async fn list_pr_comments(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<PrComment>, AppError>;

    async fn reply_to_review_thread(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
        _thread_id: &str,
        _reply_to_id: &str,
        _body: &str,
    ) -> Result<(), AppError> {
        Err(AppError::Api(format!("{} 不支持回复评审线程", self.name())))
    }

    async fn update_review_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
        _thread_id: &str,
        _comment_id: &str,
        _body: &str,
    ) -> Result<(), AppError> {
        Err(AppError::Api(format!("{} 不支持编辑评审评论", self.name())))
    }

    async fn delete_review_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
        _thread_id: &str,
        _comment_id: &str,
    ) -> Result<(), AppError> {
        Err(AppError::Api(format!("{} 不支持删除评审评论", self.name())))
    }

    async fn set_review_thread_resolved(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
        _thread_id: &str,
        _resolved: bool,
    ) -> Result<(), AppError> {
        Err(AppError::Api(format!("{} 不支持解决或重新打开评审线程", self.name())))
    }

    async fn list_viewed_pr_files(&self, _owner: &str, _repo: &str, _pr_number: u64) -> Result<Vec<String>, AppError> {
        Err(AppError::Api(format!("{} 不支持同步文件已查看状态", self.name())))
    }

    async fn set_pr_file_viewed(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
        _path: &str,
        _viewed: bool,
    ) -> Result<(), AppError> {
        Err(AppError::Api(format!("{} 不支持同步文件已查看状态", self.name())))
    }

    // ── Merge / Close / Reopen ──
    async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        strategy: &MergeStrategy,
        commit_title: Option<String>,
        commit_message: Option<String>,
        sha: &str,
    ) -> Result<PrMergeResult, AppError>;

    async fn close_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError>;

    async fn reopen_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError>;

    // ── Issue ──
    async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        state: &IssueState,
        page: u32,
    ) -> Result<Paginated<IssueSummary>, AppError>;

    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<Issue, AppError>;

    async fn close_issue(&self, owner: &str, repo: &str, issue_number: u64) -> Result<(), AppError>;
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{atomic::AtomicU32, atomic::Ordering, Mutex},
    };

    use super::{
        capabilities_for, collect_create_compare_pages, collect_json_pages, collect_json_pages_limited,
        compare_page_error_summary, create_compare_is_incomplete, json_page_url, next_page_from_link,
        normalize_api_base, walk_pr_dependency_candidates, AppError, CreateComparePageSource, JsonPage, JsonPageSource,
        CREATE_COMPARE_COMMIT_PAGE_SIZE, JSON_PAGE_SIZE, MAX_CREATE_COMPARE_PAGES,
    };
    use crate::models::{PrCreatePreviewIncompleteReason, PrDependencyCandidate, PrState};

    struct MockPageSource {
        pages: Mutex<VecDeque<JsonPage>>,
        requested_pages: Mutex<Vec<u32>>,
        requested_endpoints: Mutex<Vec<String>>,
    }

    impl MockPageSource {
        fn new(pages: Vec<JsonPage>) -> Self {
            Self {
                pages: Mutex::new(pages.into()),
                requested_pages: Mutex::new(Vec::new()),
                requested_endpoints: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl JsonPageSource for MockPageSource {
        async fn fetch_json_page(&self, endpoint: &str, page: u32) -> Result<JsonPage, AppError> {
            self.requested_pages.lock().unwrap().push(page);
            self.requested_endpoints.lock().unwrap().push(endpoint.to_string());
            self.pages.lock().unwrap().pop_front().ok_or_else(|| AppError::Api("测试分页响应不足".into()))
        }
    }

    struct UnboundedPageSource;

    #[async_trait::async_trait]
    impl JsonPageSource for UnboundedPageSource {
        async fn fetch_json_page(&self, _endpoint: &str, page: u32) -> Result<JsonPage, AppError> {
            Ok(JsonPage { items: Vec::new(), next_page: Some(page + 1), pagination_known: true })
        }
    }

    struct MockCompareSource {
        pages: Mutex<VecDeque<Result<serde_json::Value, AppError>>>,
        requested_pages: Mutex<Vec<u32>>,
    }

    impl MockCompareSource {
        fn new(pages: Vec<Result<serde_json::Value, AppError>>) -> Self {
            Self { pages: Mutex::new(pages.into()), requested_pages: Mutex::new(Vec::new()) }
        }
    }

    #[async_trait::async_trait]
    impl CreateComparePageSource for MockCompareSource {
        async fn fetch_create_compare_page(&self, _endpoint: &str, page: u32) -> Result<serde_json::Value, AppError> {
            self.requested_pages.lock().unwrap().push(page);
            self.pages.lock().unwrap().pop_front().unwrap_or_else(|| Err(AppError::Api("测试分页响应不足".into())))
        }
    }

    struct UnboundedCompareSource {
        requested_pages: AtomicU32,
    }

    #[async_trait::async_trait]
    impl CreateComparePageSource for UnboundedCompareSource {
        async fn fetch_create_compare_page(&self, _endpoint: &str, page: u32) -> Result<serde_json::Value, AppError> {
            self.requested_pages.fetch_add(1, Ordering::Relaxed);
            let start = (page as usize - 1) * CREATE_COMPARE_COMMIT_PAGE_SIZE;
            let commits = (start..start + CREATE_COMPARE_COMMIT_PAGE_SIZE)
                .map(|id| serde_json::json!({ "sha": format!("commit-{id}") }))
                .collect::<Vec<_>>();
            Ok(serde_json::json!({
                "total_commits": MAX_CREATE_COMPARE_PAGES as usize * CREATE_COMPARE_COMMIT_PAGE_SIZE + 1,
                "commits": commits,
                "files": if page == 1 { serde_json::json!([{ "filename": "src/main.rs" }]) } else { serde_json::json!([]) }
            }))
        }
    }

    fn dependency_candidate_json(number: u64, source: &str, target: &str) -> serde_json::Value {
        serde_json::json!({
            "number": number,
            "title": format!("PR {number}"),
            "source": source,
            "target": target,
        })
    }

    fn map_dependency_candidate(json: &serde_json::Value) -> Option<PrDependencyCandidate> {
        Some(PrDependencyCandidate {
            number: json["number"].as_u64()?,
            title: json["title"].as_str()?.to_string(),
            state: PrState::Open,
            source_branch: json["source"].as_str()?.to_string(),
            target_branch: json["target"].as_str()?.to_string(),
            source_repository: "team/repo".into(),
            target_repository: "team/repo".into(),
        })
    }

    fn dependency_endpoints(candidate: &PrDependencyCandidate) -> [String; 2] {
        [format!("children:{}", candidate.source_branch), format!("parents:{}", candidate.target_branch)]
    }

    #[test]
    fn exposes_the_platform_capability_matrix() {
        let github = serde_json::to_value(capabilities_for("github").unwrap()).unwrap();
        let gitlab = serde_json::to_value(capabilities_for("gitlab").unwrap()).unwrap();
        let gitee = serde_json::to_value(capabilities_for("gitee").unwrap()).unwrap();

        assert_eq!(github["review_events"], serde_json::json!(["comment", "approve", "request_changes"]));
        assert_eq!(gitlab["review_events"], serde_json::json!(["comment"]));
        assert_eq!(gitee["review_events"], serde_json::json!(["comment"]));
        assert_eq!(github["merge_strategies"], serde_json::json!(["merge", "squash", "rebase"]));
        assert_eq!(gitlab["merge_strategies"], serde_json::json!(["merge", "squash"]));
        assert_eq!(gitee["merge_strategies"], serde_json::json!(["merge", "squash", "rebase"]));
        assert!(github["supports_fork_context"].as_bool().unwrap());
        assert!(gitlab["supports_issue_auto_close"].as_bool().unwrap());
        assert_eq!(github["supports_review_thread_resolution"], serde_json::json!(true));
        assert_eq!(gitlab["supports_review_thread_resolution"], serde_json::json!(true));
        assert_eq!(gitee["supports_review_thread_resolution"], serde_json::json!(false));
        assert_eq!(github["supports_remote_file_viewed_state"], serde_json::json!(true));
        assert_eq!(gitlab["supports_remote_file_viewed_state"], serde_json::json!(false));
        assert_eq!(gitee["supports_remote_file_viewed_state"], serde_json::json!(false));
        assert_eq!(github["supports_pr_creation"], serde_json::json!(true));
        assert_eq!(gitlab["supports_pr_creation"], serde_json::json!(true));
        assert_eq!(gitee["supports_pr_creation"], serde_json::json!(true));
        assert_eq!(github["merge_queue_kind"], serde_json::json!("merge_queue"));
        assert_eq!(gitlab["merge_queue_kind"], serde_json::json!("merge_train"));
        assert!(gitee["merge_queue_kind"].is_null());
        assert!(capabilities_for("unknown").is_none());
    }

    #[test]
    fn normalizes_platform_api_roots() {
        assert_eq!(normalize_api_base("gitlab", "https://git.example.com/"), "https://git.example.com/api/v4");
        assert_eq!(
            normalize_api_base("gitlab", "https://git.example.com/proxy"),
            "https://git.example.com/proxy/api/v4"
        );
        assert_eq!(
            normalize_api_base("gitlab", "https://git.example.com/proxy/api/v4/"),
            "https://git.example.com/proxy/api/v4"
        );
        assert_eq!(normalize_api_base("gitee", "http://gitee.internal/base"), "http://gitee.internal/base/api/v5");
    }

    #[test]
    fn appends_pagination_to_endpoints_with_existing_queries() {
        assert_eq!(
            json_page_url("https://example.test/items?state=all", 2),
            "https://example.test/items?state=all&per_page=100&page=2"
        );
        assert_eq!(json_page_url("https://example.test/items", 1), "https://example.test/items?per_page=100&page=1");
    }

    #[test]
    fn marks_create_compare_incomplete_when_reported_commits_exceed_the_response() {
        for json in [serde_json::json!({ "total_commits": 101 }), serde_json::json!({ "ahead_by": 101 })] {
            assert!(create_compare_is_incomplete(&json, 100, 1));
        }
    }

    #[test]
    fn marks_create_compare_incomplete_when_an_unreported_limit_is_reached() {
        assert!(create_compare_is_incomplete(&serde_json::json!({}), 100, 1));
        assert!(create_compare_is_incomplete(&serde_json::json!({ "total_commits": 1 }), 1, 300));
    }

    #[test]
    fn leaves_create_compare_complete_when_the_response_is_below_the_limits() {
        assert!(!create_compare_is_incomplete(&serde_json::json!({ "total_commits": 100 }), 100, 299));
        assert!(!create_compare_is_incomplete(&serde_json::json!({}), 2, 3));
    }

    #[tokio::test]
    async fn collects_all_reported_compare_commit_pages_and_deduplicates_files() {
        let source = MockCompareSource::new(vec![
            Ok(serde_json::json!({
                "total_commits": 2,
                "commits": [{ "sha": "one" }],
                "files": [{ "filename": "src/one.rs" }]
            })),
            Ok(serde_json::json!({
                "commits": [{ "sha": "two" }],
                "files": [{ "filename": "src/one.rs" }, { "filename": "src/two.rs" }]
            })),
        ]);

        let result = collect_create_compare_pages(&source, "compare", "Test", "missing files").await.unwrap();

        assert_eq!(result.commits.len(), 2);
        assert_eq!(result.files.len(), 2);
        assert!(!result.incomplete);
        assert_eq!(*source.requested_pages.lock().unwrap(), vec![1, 2]);
    }

    #[tokio::test]
    async fn confirms_an_unreported_exact_page_boundary_with_an_empty_followup_page() {
        let commits = (0..CREATE_COMPARE_COMMIT_PAGE_SIZE)
            .map(|id| serde_json::json!({ "sha": format!("commit-{id}") }))
            .collect::<Vec<_>>();
        let source = MockCompareSource::new(vec![
            Ok(serde_json::json!({
                "commits": commits,
                "files": [{ "filename": "src/main.rs" }]
            })),
            Ok(serde_json::json!({ "commits": [] })),
        ]);

        let result = collect_create_compare_pages(&source, "compare", "Test", "missing files").await.unwrap();

        assert_eq!(result.commits.len(), CREATE_COMPARE_COMMIT_PAGE_SIZE);
        assert!(!result.incomplete);
        assert_eq!(*source.requested_pages.lock().unwrap(), vec![1, 2]);
    }

    #[tokio::test]
    async fn marks_compare_incomplete_when_a_supplemental_page_fails() {
        let source = MockCompareSource::new(vec![
            Ok(serde_json::json!({
                "total_commits": 2,
                "commits": [{ "sha": "one" }],
                "files": [{ "filename": "src/one.rs" }]
            })),
            Err(AppError::Api("rate limited".into())),
        ]);

        let result = collect_create_compare_pages(&source, "compare", "Test", "missing files").await.unwrap();

        assert_eq!(result.commits.len(), 1);
        assert!(result.incomplete);
        assert_eq!(result.incomplete_reasons, vec![PrCreatePreviewIncompleteReason::PaginationFailed]);
    }

    #[tokio::test]
    async fn stops_when_a_compare_api_repeats_the_first_page() {
        let page = serde_json::json!({
            "total_commits": 2,
            "commits": [{ "sha": "one" }],
            "files": [{ "filename": "src/one.rs" }]
        });
        let source = MockCompareSource::new(vec![Ok(page.clone()), Ok(page)]);

        let result = collect_create_compare_pages(&source, "compare", "Test", "missing files").await.unwrap();

        assert_eq!(result.commits.len(), 1);
        assert!(result.incomplete);
        assert_eq!(result.incomplete_reasons, vec![PrCreatePreviewIncompleteReason::PlatformLimit]);
        assert_eq!(*source.requested_pages.lock().unwrap(), vec![1, 2]);
    }

    #[tokio::test]
    async fn bounds_large_compare_pagination_and_marks_the_result_incomplete() {
        let source = UnboundedCompareSource { requested_pages: AtomicU32::new(0) };

        let result = collect_create_compare_pages(&source, "compare", "Test", "missing files").await.unwrap();

        assert_eq!(result.commits.len(), MAX_CREATE_COMPARE_PAGES as usize * CREATE_COMPARE_COMMIT_PAGE_SIZE);
        assert!(result.incomplete);
        assert_eq!(result.incomplete_reasons, vec![PrCreatePreviewIncompleteReason::PaginationLimit]);
        assert_eq!(source.requested_pages.load(Ordering::Relaxed), MAX_CREATE_COMPARE_PAGES);
    }

    #[test]
    fn sanitizes_compare_page_errors_before_logging() {
        let error =
            AppError::Api("Gitee API 429 (https://gitee.example/api?access_token=secret-token): rate limited".into());

        let summary = compare_page_error_summary(&error);

        assert_eq!(summary, "HTTP 429");
        assert!(!summary.contains("secret-token"));
        assert!(!summary.contains("gitee.example"));
    }

    #[tokio::test]
    async fn collects_pages_using_the_explicit_next_cursor() {
        let source = MockPageSource::new(vec![
            JsonPage { items: vec![serde_json::json!({ "id": 1 })], next_page: Some(3), pagination_known: true },
            JsonPage { items: vec![serde_json::json!({ "id": 2 })], next_page: None, pagination_known: true },
        ]);

        let items = collect_json_pages(&source, "https://example.test/items").await.unwrap();

        assert_eq!(items, vec![serde_json::json!({ "id": 1 }), serde_json::json!({ "id": 2 })]);
        assert_eq!(*source.requested_pages.lock().unwrap(), vec![1, 3]);
    }

    #[tokio::test]
    async fn requests_another_page_for_a_full_page_without_pagination_headers() {
        let source = MockPageSource::new(vec![
            JsonPage {
                items: (0..JSON_PAGE_SIZE).map(|id| serde_json::json!({ "id": id })).collect(),
                next_page: None,
                pagination_known: false,
            },
            JsonPage {
                items: vec![serde_json::json!({ "id": JSON_PAGE_SIZE })],
                next_page: None,
                pagination_known: false,
            },
        ]);

        let items = collect_json_pages(&source, "https://example.test/items").await.unwrap();

        assert_eq!(items.len(), JSON_PAGE_SIZE + 1);
        assert_eq!(*source.requested_pages.lock().unwrap(), vec![1, 2]);
    }

    #[tokio::test]
    async fn limited_collection_stops_at_the_requested_page_count() {
        let source = MockPageSource::new(vec![
            JsonPage { items: vec![serde_json::json!({ "id": 1 })], next_page: Some(3), pagination_known: true },
            JsonPage { items: vec![serde_json::json!({ "id": 2 })], next_page: Some(4), pagination_known: true },
        ]);

        let (items, truncated) = collect_json_pages_limited(&source, "https://example.test/items", 2).await.unwrap();

        assert_eq!(items.len(), 2);
        assert!(truncated);
        assert_eq!(*source.requested_pages.lock().unwrap(), vec![1, 3]);
    }

    #[tokio::test]
    async fn dependency_walker_discovers_neighbors_and_deduplicates_candidates() {
        let source = MockPageSource::new(vec![
            JsonPage {
                items: vec![dependency_candidate_json(3, "feature-c", "feature-b")],
                next_page: None,
                pagination_known: true,
            },
            JsonPage { items: Vec::new(), next_page: None, pagination_known: true },
            JsonPage { items: Vec::new(), next_page: None, pagination_known: true },
            JsonPage {
                items: vec![dependency_candidate_json(2, "feature-b", "feature-a")],
                next_page: None,
                pagination_known: true,
            },
        ]);
        let current = map_dependency_candidate(&dependency_candidate_json(2, "feature-b", "feature-a")).unwrap();

        let result = walk_pr_dependency_candidates(&source, current, map_dependency_candidate, dependency_endpoints)
            .await
            .unwrap();

        assert_eq!(result.items.iter().map(|candidate| candidate.number).collect::<Vec<_>>(), vec![2, 3]);
        assert!(!result.truncated);
        assert_eq!(
            *source.requested_endpoints.lock().unwrap(),
            vec!["children:feature-b", "parents:feature-a", "children:feature-c", "parents:feature-b"]
        );
    }

    #[tokio::test]
    async fn dependency_walker_marks_page_limit_truncation() {
        let source = MockPageSource::new(vec![
            JsonPage { items: Vec::new(), next_page: Some(2), pagination_known: true },
            JsonPage { items: Vec::new(), next_page: Some(3), pagination_known: true },
            JsonPage { items: Vec::new(), next_page: None, pagination_known: true },
        ]);
        let current = map_dependency_candidate(&dependency_candidate_json(1, "feature-a", "main")).unwrap();

        let result = walk_pr_dependency_candidates(&source, current, map_dependency_candidate, dependency_endpoints)
            .await
            .unwrap();

        assert!(result.truncated);
        assert_eq!(*source.requested_pages.lock().unwrap(), vec![1, 2, 1]);
    }

    #[tokio::test]
    async fn dependency_walker_marks_query_limit_truncation() {
        let pages = (0..super::MAX_DEPENDENCY_BRANCH_QUERIES)
            .map(|index| {
                let items = if index % 2 == 0 {
                    let number = index as u64 / 2 + 2;
                    vec![dependency_candidate_json(number, &format!("feature-{number}"), &format!("base-{number}"))]
                } else {
                    Vec::new()
                };
                JsonPage { items, next_page: None, pagination_known: true }
            })
            .collect();
        let source = MockPageSource::new(pages);
        let current = map_dependency_candidate(&dependency_candidate_json(1, "feature-1", "base-1")).unwrap();

        let result = walk_pr_dependency_candidates(&source, current, map_dependency_candidate, dependency_endpoints)
            .await
            .unwrap();

        assert!(result.truncated);
        assert_eq!(source.requested_endpoints.lock().unwrap().len(), super::MAX_DEPENDENCY_BRANCH_QUERIES);
    }

    #[tokio::test]
    async fn dependency_walker_marks_candidate_limit_truncation() {
        let items = (2..=super::MAX_DEPENDENCY_CANDIDATES as u64 + 1)
            .map(|number| dependency_candidate_json(number, &format!("feature-{number}"), &format!("base-{number}")))
            .collect();
        let source = MockPageSource::new(vec![JsonPage { items, next_page: None, pagination_known: true }]);
        let current = map_dependency_candidate(&dependency_candidate_json(1, "feature-1", "base-1")).unwrap();

        let result = walk_pr_dependency_candidates(&source, current, map_dependency_candidate, dependency_endpoints)
            .await
            .unwrap();

        assert!(result.truncated);
        assert_eq!(result.items.len(), super::MAX_DEPENDENCY_CANDIDATES);
        assert_eq!(source.requested_endpoints.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rejects_a_pagination_cursor_that_does_not_advance() {
        let source =
            MockPageSource::new(vec![JsonPage { items: Vec::new(), next_page: Some(1), pagination_known: true }]);

        let error = collect_json_pages(&source, "https://example.test/items").await.unwrap_err();

        assert!(matches!(error, AppError::Api(message) if message.contains("未向前推进")));
    }

    #[tokio::test]
    async fn rejects_pagination_beyond_the_safety_limit() {
        let error = collect_json_pages(&UnboundedPageSource, "https://example.test/items").await.unwrap_err();

        assert!(matches!(error, AppError::Api(message) if message.contains("1000 页")));
    }

    #[test]
    fn reads_the_next_page_from_a_link_header() {
        let link = concat!(
            "<https://example.test/items?per_page=100&page=2>; rel=\"next\", ",
            "<https://example.test/items?per_page=100&page=5>; rel=\"last\"",
        );

        assert_eq!(next_page_from_link(Some(link)), Some(2));
        assert_eq!(next_page_from_link(Some("<https://example.test/items?page=5>; rel=\"last\"")), None);
        assert_eq!(next_page_from_link(None), None);
    }
}
