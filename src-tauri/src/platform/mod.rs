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
        },
        "gitlab" => PlatformCapabilities {
            platform: "gitlab",
            review_events: vec![ReviewEvent::Comment],
            merge_strategies: vec![MergeStrategy::Merge, MergeStrategy::Squash],
            supports_fork_context: true,
            supports_issue_auto_close: true,
        },
        "gitee" => PlatformCapabilities {
            platform: "gitee",
            review_events: vec![ReviewEvent::Comment],
            merge_strategies: vec![MergeStrategy::Merge, MergeStrategy::Squash, MergeStrategy::Rebase],
            supports_fork_context: true,
            supports_issue_auto_close: true,
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

    async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrDetail, AppError>;

    async fn get_merge_readiness(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrMergeReadiness, AppError>;

    async fn get_pr_diff(&self, owner: &str, repo: &str, pr_number: u64) -> Result<(String, Vec<PrFile>), AppError>;

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
    use super::{capabilities_for, normalize_api_base};

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
}
