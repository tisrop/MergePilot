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

pub(crate) fn create_compare_is_incomplete(json: &serde_json::Value, commit_count: usize, file_count: usize) -> bool {
    let reported_commit_count = json["total_commits"].as_u64().into_iter().chain(json["ahead_by"].as_u64()).max();
    let commits_incomplete = reported_commit_count
        .map(|total| total > commit_count as u64)
        .unwrap_or(commit_count >= CREATE_COMPARE_COMMIT_PAGE_SIZE);

    // Compare APIs cap the returned files without exposing a reliable file total.
    commits_incomplete || file_count >= CREATE_COMPARE_FILE_LIMIT
}

pub(crate) const JSON_PAGE_SIZE: usize = 100;
const MAX_JSON_PAGES: u32 = 1_000;

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
    use std::{collections::VecDeque, sync::Mutex};

    use super::{
        capabilities_for, collect_json_pages, create_compare_is_incomplete, next_page_from_link, normalize_api_base,
        AppError, JsonPage, JsonPageSource, JSON_PAGE_SIZE,
    };

    struct MockPageSource {
        pages: Mutex<VecDeque<JsonPage>>,
        requested_pages: Mutex<Vec<u32>>,
    }

    impl MockPageSource {
        fn new(pages: Vec<JsonPage>) -> Self {
            Self { pages: Mutex::new(pages.into()), requested_pages: Mutex::new(Vec::new()) }
        }
    }

    #[async_trait::async_trait]
    impl JsonPageSource for MockPageSource {
        async fn fetch_json_page(&self, _endpoint: &str, page: u32) -> Result<JsonPage, AppError> {
            self.requested_pages.lock().unwrap().push(page);
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
