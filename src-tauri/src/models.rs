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
    pub status: Option<PrStatusSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewInboxCategory {
    ReviewRequested,
    Authored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewInboxRelationship {
    Reviewer,
    Assignee,
    Tester,
    Author,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrStatusSummary {
    pub status: ReadinessState,
    pub draft: Option<bool>,
    pub has_conflicts: Option<bool>,
    pub checks_status: ReadinessState,
    pub approvals_status: ReadinessState,
    pub blocking_reasons: Vec<MergeBlockingReason>,
}

impl Default for PrStatusSummary {
    fn default() -> Self {
        Self {
            status: ReadinessState::Unknown,
            draft: None,
            has_conflicts: None,
            checks_status: ReadinessState::Unknown,
            approvals_status: ReadinessState::Unknown,
            blocking_reasons: Vec::new(),
        }
    }
}

pub type ReviewInboxStatusSummary = PrStatusSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewInboxItem {
    pub platform: String,
    pub owner: String,
    pub repo: String,
    pub repository_full_name: String,
    pub categories: Vec<ReviewInboxCategory>,
    pub relationships: Vec<ReviewInboxRelationship>,
    pub status: ReviewInboxStatusSummary,
    #[serde(default)]
    pub head_sha: Option<String>,
    #[serde(default)]
    pub comments_count: Option<u64>,
    pub summary: PrSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMilestone {
    pub id: serde_json::Value,
    pub number: Option<u64>,
    pub title: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrMetadataPermissions {
    pub can_edit_title_body: Option<bool>,
    pub can_toggle_draft: Option<bool>,
    pub can_manage_reviewers: Option<bool>,
    pub can_manage_assignees: Option<bool>,
    pub can_manage_labels: Option<bool>,
    pub can_manage_milestone: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrDetail {
    pub summary: PrSummary,
    pub body: String,
    pub source_branch: String,
    pub target_branch: String,
    pub mergeable: Option<bool>,
    pub head_sha: String,
    pub base_sha: String,
    pub draft: Option<bool>,
    pub reviewers: Vec<User>,
    pub assignees: Vec<User>,
    pub milestone: Option<PrMilestone>,
    pub metadata_permissions: PrMetadataPermissions,
}

#[derive(Debug, Clone)]
pub struct PrDependencyCandidate {
    pub number: u64,
    pub title: String,
    pub state: PrState,
    pub source_branch: String,
    pub target_branch: String,
    pub source_repository: String,
    pub target_repository: String,
}

#[derive(Debug, Clone)]
pub struct PrDependencyCandidates {
    pub current: PrDependencyCandidate,
    pub items: Vec<PrDependencyCandidate>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrDependencyNode {
    pub number: u64,
    pub title: String,
    pub state: PrState,
    pub source_branch: String,
    pub target_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrDependencyEdge {
    pub parent_number: u64,
    pub child_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrDependencyGraph {
    pub current_number: u64,
    pub nodes: Vec<PrDependencyNode>,
    pub edges: Vec<PrDependencyEdge>,
    pub suggested_merge_order: Vec<u64>,
    pub blocking_parent_numbers: Vec<u64>,
    pub has_cycle: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeQueueKind {
    MergeQueue,
    MergeTrain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeQueueState {
    NotQueued,
    Queued,
    Waiting,
    Ready,
    Blocked,
    Merging,
    Failed,
    Merged,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrMergeQueueStatus {
    pub kind: MergeQueueKind,
    pub available: bool,
    pub state: MergeQueueState,
    pub position: Option<u32>,
    pub total: Option<u32>,
    pub target_branch: Option<String>,
    pub enqueued_at: Option<String>,
    pub updated_at: Option<String>,
    pub estimated_time_seconds: Option<u64>,
    pub head_sha: Option<String>,
    pub pipeline_status: Option<String>,
    pub failure_reason: Option<String>,
}

impl PrMergeQueueStatus {
    pub fn unavailable(kind: MergeQueueKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            available: false,
            state: MergeQueueState::Unknown,
            position: None,
            total: None,
            target_branch: None,
            enqueued_at: None,
            updated_at: None,
            estimated_time_seconds: None,
            head_sha: None,
            pipeline_status: None,
            failure_reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMetadataUpdate {
    pub title: String,
    pub body: String,
    pub draft: Option<bool>,
    pub reviewers: Vec<String>,
    pub assignees: Vec<String>,
    pub labels: Vec<String>,
    pub milestone: Option<String>,
    pub expected_updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrMetadataField {
    TitleBody,
    Draft,
    Reviewers,
    Assignees,
    Labels,
    Milestone,
    Refresh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMetadataUpdateFailure {
    pub field: PrMetadataField,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct PrMetadataMutationResult {
    pub updated_fields: Vec<PrMetadataField>,
    pub failures: Vec<PrMetadataUpdateFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMetadataUpdateOutcome {
    pub detail: Option<PrDetail>,
    pub updated_fields: Vec<PrMetadataField>,
    pub failures: Vec<PrMetadataUpdateFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrCreateRequest {
    pub source_owner: String,
    pub source_repo: String,
    pub source_branch: String,
    pub target_branch: String,
    pub title: String,
    pub body: String,
    pub draft: bool,
    pub reviewers: Vec<String>,
    pub assignees: Vec<String>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrCreatePreviewRequest {
    pub source_owner: String,
    pub source_repo: String,
    pub source_branch: String,
    pub target_branch: String,
    pub commit_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrCommitSummary {
    pub sha: String,
    pub title: String,
    pub author_name: String,
    pub authored_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrCreatePreviewIncompleteReason {
    PlatformLimit,
    PaginationFailed,
    PaginationLimit,
}

#[derive(Debug, Clone)]
pub struct PrCreatePreviewData {
    pub commits: Vec<PrCommitSummary>,
    pub diff: String,
    pub files: Vec<PrFile>,
    pub incomplete: bool,
    pub incomplete_reasons: Vec<PrCreatePreviewIncompleteReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrBranchOptions {
    pub branches: Vec<String>,
    pub default_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrLabel {
    pub name: String,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrCreateOutcome {
    pub number: u64,
    pub detail: Option<PrDetail>,
    pub updated_fields: Vec<PrMetadataField>,
    pub failures: Vec<PrMetadataUpdateFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrFileContent {
    pub path: String,
    pub revision: String,
    pub content: String,
    pub truncated: bool,
    pub binary: bool,
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
#[serde(rename_all = "snake_case")]
pub enum PatchContentKind {
    Text,
    Binary,
    MetadataOnly,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchLineKind {
    Context,
    Addition,
    Deletion,
    NoNewline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchLine {
    pub kind: PatchLineKind,
    pub content: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchHunk {
    pub header: String,
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub section_header: Option<String>,
    pub lines: Vec<PatchLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardPatchFile {
    pub filename: String,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: FileStatus,
    pub additions: u32,
    pub deletions: u32,
    pub content_kind: PatchContentKind,
    pub patch: String,
    pub hunks: Vec<PatchHunk>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub diff: String,
    pub files: Vec<PrFile>,
    pub patch_schema_version: u32,
    pub patches: Vec<StandardPatchFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrCreatePreview {
    pub commits: Vec<PrCommitSummary>,
    pub diff: DiffResult,
    pub incomplete: bool,
    pub incomplete_reasons: Vec<PrCreatePreviewIncompleteReason>,
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
    #[serde(default)]
    pub side: Option<String>,
    pub author: User,
    pub created_at: String,
    pub commit_id: Option<String>,
    pub original_commit_id: Option<String>,
    pub original_line: Option<u32>,
    pub original_start_line: Option<u32>,
    pub diff_hunk: Option<String>,
    pub thread_id: String,
    pub reply_to_id: Option<String>,
    pub resolved: Option<bool>,
    pub resolvable: bool,
    #[serde(default)]
    pub can_edit: bool,
    #[serde(default)]
    pub can_delete: bool,
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
    #[serde(default)]
    pub repository_rules: Option<String>,
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
