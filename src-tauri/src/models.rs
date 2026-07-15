use serde::{Deserialize, Serialize};

// ── User ──
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: serde_json::Value,
    pub login: String,
    pub name: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthLoginResult {
    pub user: User,
    pub credential_storage: crate::vault::CredentialStorage,
}

// ── Repository ──
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    pub id: serde_json::Value,
    pub name: String,
    pub full_name: String,
    pub owner: String,
    /// "user", "organization", or "group" (GitLab)
    pub owner_type: String,
    /// Display name of the owner (org/enterprise/user full name)
    pub owner_display_name: String,
    pub description: String,
    pub private: bool,
    pub fork: bool,
    /// Parent repo full name, if this is a fork (e.g. "torvalds/linux")
    pub parent_full_name: Option<String>,
    /// Parent repo owner, if this is a fork (e.g. "torvalds")
    pub parent_owner: Option<String>,
}

// ── PR / MR ──
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    Open,
    Closed,
    Merged,
    All,
}

impl PrState {
    pub fn as_str(&self) -> &str {
        match self {
            PrState::Open => "open",
            PrState::Closed => "closed",
            PrState::Merged => "merged",
            PrState::All => "all",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrSummary {
    pub number: u64,
    pub title: String,
    pub author: User,
    pub state: PrState,
    pub created_at: String,
    pub updated_at: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrDetail {
    pub summary: PrSummary,
    pub body: String,
    pub source_branch: String,
    pub target_branch: String,
    pub mergeable: Option<bool>,
    pub head_sha: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    Ready,
    Blocked,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeBlockingReasonCode {
    NotOpen,
    Draft,
    Conflicts,
    ChecksFailed,
    ChecksPending,
    ChangesRequested,
    ApprovalsRequired,
    BranchBehind,
    DiscussionsUnresolved,
    NoMergePermission,
    PlatformBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeBlockingReason {
    pub code: MergeBlockingReasonCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrMergeReadiness {
    pub status: ReadinessState,
    pub head_sha: String,
    pub mergeable: Option<bool>,
    pub draft: Option<bool>,
    pub has_conflicts: Option<bool>,
    pub checks_status: ReadinessState,
    pub approvals_status: ReadinessState,
    pub approvals_required: Option<u32>,
    pub approvals_received: Option<u32>,
    pub has_merge_permission: Option<bool>,
    pub branch_behind: Option<bool>,
    pub blocking_reasons: Vec<MergeBlockingReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrFile {
    pub filename: String,
    pub status: FileStatus,
    pub patch: String,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Added,
    Modified,
    Removed,
    Renamed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub diff: String,
    pub files: Vec<PrFile>,
}

// ── Review ──
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewEvent {
    Approve,
    Comment,
    RequestChanges,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCommentPosition {
    pub path: String,
    pub position: u32,
    pub end_line: Option<u32>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReviewRequest {
    pub body: String,
    pub event: ReviewEvent,
    pub comments: Vec<ReviewCommentPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: serde_json::Value,
    pub body: String,
    pub state: String,
    pub author: User,
    pub submitted_at: String,
}

// ── PR Comment ──
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrComment {
    pub id: serde_json::Value,
    pub body: String,
    pub path: String,
    pub line: Option<u32>,
    pub start_line: Option<u32>,
    pub author: User,
    pub created_at: String,
    pub commit_id: Option<String>,
    pub original_commit_id: Option<String>,
    pub original_line: Option<u32>,
    pub original_start_line: Option<u32>,
    pub diff_hunk: Option<String>,
}

// ── Issue ──
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueState {
    Open,
    Closed,
    All,
}

impl IssueState {
    pub fn as_str(&self) -> &str {
        match self {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
            IssueState::All => "all",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    pub number: u64,
    pub title: String,
    pub author: User,
    pub state: IssueState,
    pub labels: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub author: User,
    pub state: IssueState,
    pub labels: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIssueRequest {
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

// ── Merge / Close / Reopen ──
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    Merge,
    Squash,
    Rebase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMergeResult {
    pub merged: bool,
    pub sha: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCloseFailure {
    pub number: u64,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMergeOutcome {
    pub merge: PrMergeResult,
    pub closed_issues: Vec<u64>,
    pub issue_close_failures: Vec<IssueCloseFailure>,
}

// ── Pagination ──
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub total_pages: u32,
    pub total_count: u32,
}

// ── AI ──
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStreamEvent<T> {
    pub request_id: String,
    pub payload: T,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key_configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_encrypted: Option<String>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiReviewRequest {
    pub diff: String,
    pub context: Option<PrContext>,
    pub file_filter: Option<Vec<String>>,
    pub focus: Option<AiReviewFocus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrContext {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiReviewFocus {
    All,
    Security,
    Performance,
    Logic,
    CodeStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiReviewResult {
    pub summary: String,
    pub suggestions: Vec<AiSuggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSuggestion {
    pub file: String,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub severity: Severity,
    pub category: String,
    pub description: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    Major,
    Minor,
    Info,
}
