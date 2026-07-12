pub mod gitee;
pub mod github;
pub mod gitlab;

use crate::error::AppError;
use crate::models::*;
use async_trait::async_trait;

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
    use super::normalize_api_base;

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
