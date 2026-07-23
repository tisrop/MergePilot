// ============================================================
// 数据模型 —— 前后端一致的类型定义
// ============================================================

export type CommandErrorCode =
  | "validation"
  | "authentication"
  | "permission_denied"
  | "not_found"
  | "conflict"
  | "rate_limited"
  | "network"
  | "timeout"
  | "invalid_response"
  | "storage"
  | "unsupported"
  | "ai"
  | "platform"
  | "unknown";

export interface CommandErrorPayload {
  code: CommandErrorCode;
  message: string;
  retryable: boolean;
  request_id?: string;
  http_status?: number;
}

// ── 用户 ──
export interface User {
  id: number | string;
  login: string;
  name: string;
  avatar_url: string;
}

export type CredentialStorage = "system_keyring" | "encrypted_file";

export interface AuthLoginResult {
  user: User;
  credential_storage: CredentialStorage;
}

// ── 平台 ──
export type Platform = "github" | "gitlab" | "gitee";
export type MergeQueueKind = "merge_queue" | "merge_train";

/** 平台 API 的静态协议能力，不包含登录、Token 权限或 PR 运行时状态。 */
export interface PlatformCapabilities {
  platform: Platform;
  review_events: ReviewEvent[];
  merge_strategies: MergeStrategy[];
  supports_fork_context: boolean;
  supports_issue_auto_close: boolean;
  supports_compare_diff: boolean;
  supports_review_thread_resolution: boolean;
  supports_remote_file_viewed_state: boolean;
  supports_pr_title_body_edit: boolean;
  supports_pr_draft_toggle: boolean;
  supports_pr_reviewer_management: boolean;
  supports_pr_assignee_management: boolean;
  supports_pr_label_management: boolean;
  supports_pr_milestone_management: boolean;
  supports_pr_creation: boolean;
  merge_queue_kind: MergeQueueKind | null;
}

export interface UpdateProgressEvent {
  request_id: string;
  downloaded: number;
  total: number | null;
  phase: "downloading" | "installing";
}

export interface UpdateCheckResult {
  current_version: string;
  available: boolean;
  version: string | null;
  notes: string | null;
  published_at: string | null;
  update_mode: "installer" | "portable";
  portable_download_url?: string | null;
}

export interface SupportInfo {
  app_version: string;
  operating_system: string;
  architecture: string;
  current_platform: string;
  platform_endpoint: string;
  credential_storage: string;
  ai_configured: boolean;
  ai_endpoint: string;
  local_cache_available: boolean;
  formatted: string;
}

// ── PR ──
export type PrState = "open" | "closed" | "merged" | "all";

export interface PrSummary {
  number: number;
  title: string;
  author: User;
  state: PrState;
  created_at: string;
  updated_at: string;
  labels: string[];
  status?: PrStatusSummary | null;
}

export type ReviewInboxCategory = "review_requested" | "authored";
export type ReviewInboxRelationship = "reviewer" | "assignee" | "tester" | "author";

export interface PrStatusSummary {
  status: ReadinessState;
  draft: boolean | null;
  has_conflicts: boolean | null;
  checks_status: ReadinessState;
  approvals_status: ReadinessState;
  blocking_reasons: MergeBlockingReason[];
}

export type ReviewInboxStatusSummary = PrStatusSummary;

export interface ReviewInboxLocalState {
  unread: boolean;
  new_commits: boolean;
  new_comments: boolean;
  status_changed: boolean;
}

export interface ReviewInboxItem {
  platform: Platform;
  owner: string;
  repo: string;
  repository_full_name: string;
  categories: ReviewInboxCategory[];
  relationships: ReviewInboxRelationship[];
  status: ReviewInboxStatusSummary;
  head_sha?: string | null;
  comments_count?: number | null;
  local_state?: ReviewInboxLocalState;
  summary: PrSummary;
}

export interface PrMilestone {
  id: number | string;
  number: number | null;
  title: string;
}

export interface PrMetadataPermissions {
  can_edit_title_body: boolean | null;
  can_toggle_draft: boolean | null;
  can_manage_reviewers: boolean | null;
  can_manage_assignees: boolean | null;
  can_manage_labels: boolean | null;
  can_manage_milestone: boolean | null;
}

export interface PrDetail {
  summary: PrSummary;
  body: string;
  source_branch: string;
  target_branch: string;
  mergeable: boolean | null;
  head_sha: string;
  base_sha: string;
  draft: boolean | null;
  reviewers: User[];
  assignees: User[];
  milestone: PrMilestone | null;
  metadata_permissions: PrMetadataPermissions;
}

export interface PrDependencyNode {
  number: number;
  title: string;
  state: PrState;
  source_branch: string;
  target_branch: string;
}

export interface PrDependencyEdge {
  parent_number: number;
  child_number: number;
}

export interface PrDependencyGraph {
  current_number: number;
  nodes: PrDependencyNode[];
  edges: PrDependencyEdge[];
  suggested_merge_order: number[];
  blocking_parent_numbers: number[];
  has_cycle: boolean;
  truncated: boolean;
}

export type MergeQueueState =
  | "not_queued"
  | "queued"
  | "waiting"
  | "ready"
  | "blocked"
  | "merging"
  | "failed"
  | "merged"
  | "unknown";

export interface PrMergeQueueStatus {
  kind: MergeQueueKind;
  available: boolean;
  state: MergeQueueState;
  position: number | null;
  total: number | null;
  target_branch: string | null;
  enqueued_at: string | null;
  updated_at: string | null;
  estimated_time_seconds: number | null;
  head_sha: string | null;
  pipeline_status: string | null;
  failure_reason: string | null;
}

export interface PrMetadataUpdate {
  title: string;
  body: string;
  draft: boolean | null;
  reviewers: string[];
  assignees: string[];
  labels: string[];
  milestone: string | null;
  expected_updated_at: string;
}

export type PrMetadataField =
  | "title_body"
  | "draft"
  | "reviewers"
  | "assignees"
  | "labels"
  | "milestone"
  | "refresh";

export interface PrMetadataUpdateFailure {
  field: PrMetadataField;
  message: string;
}

export interface PrMetadataUpdateOutcome {
  detail: PrDetail | null;
  updated_fields: PrMetadataField[];
  failures: PrMetadataUpdateFailure[];
}

export interface PrCreateRequest {
  source_owner: string;
  source_repo: string;
  source_branch: string;
  target_branch: string;
  title: string;
  body: string;
  draft: boolean;
  reviewers: string[];
  assignees: string[];
  labels: string[];
}

export interface PrCreatePreviewRequest {
  source_owner: string;
  source_repo: string;
  source_branch: string;
  target_branch: string;
  commit_sha?: string | null;
}

export interface PrCommitSummary {
  sha: string;
  title: string;
  author_name: string;
  authored_at: string;
}

export interface PrBranchOptions {
  branches: string[];
  default_branch: string | null;
}

export interface PrLabel {
  name: string;
  color: string | null;
  description: string | null;
}

export interface PrCreateOutcome {
  number: number;
  detail: PrDetail | null;
  updated_fields: PrMetadataField[];
  failures: PrMetadataUpdateFailure[];
}

export interface PrFileContent {
  path: string;
  revision: string;
  content: string;
  truncated: boolean;
  binary: boolean;
}

export type ReadinessState = "ready" | "blocked" | "pending" | "unknown";

export type MergeBlockingReasonCode =
  | "not_open"
  | "draft"
  | "conflicts"
  | "checks_failed"
  | "checks_pending"
  | "changes_requested"
  | "approvals_required"
  | "branch_behind"
  | "discussions_unresolved"
  | "no_merge_permission"
  | "platform_blocked";

export interface MergeBlockingReason {
  code: MergeBlockingReasonCode;
  message: string;
}

export interface PrMergeReadiness {
  status: ReadinessState;
  head_sha: string;
  mergeable: boolean | null;
  draft: boolean | null;
  has_conflicts: boolean | null;
  checks_status: ReadinessState;
  approvals_status: ReadinessState;
  approvals_required: number | null;
  approvals_received: number | null;
  has_merge_permission: boolean | null;
  branch_behind: boolean | null;
  blocking_reasons: MergeBlockingReason[];
}

export type MergeStrategy = "merge" | "squash" | "rebase";

export interface MergeResult {
  merged: boolean;
  sha: string;
  message: string;
}

export interface IssueCloseFailure {
  number: number;
  error: string;
}

export interface PrMergeOutcome {
  merge: MergeResult;
  closed_issues: number[];
  issue_close_failures: IssueCloseFailure[];
}

export interface PrFile {
  filename: string;
  status: FileStatus;
  patch: string;
  additions: number;
  deletions: number;
}

export type FileStatus = "added" | "modified" | "removed" | "renamed";

export type PatchContentKind = "text" | "binary" | "metadata_only" | "unavailable";
export type PatchLineKind = "context" | "addition" | "deletion" | "no_newline";

export interface PatchLine {
  kind: PatchLineKind;
  content: string;
  old_line: number | null;
  new_line: number | null;
}

export interface PatchHunk {
  header: string;
  old_start: number;
  old_count: number;
  new_start: number;
  new_count: number;
  section_header: string | null;
  lines: PatchLine[];
}

/** 后端统一解析的文件 patch；供 Vue 组件受控渲染，不包含任何 HTML。 */
export interface StandardPatchFile {
  filename: string;
  old_path: string | null;
  new_path: string | null;
  status: FileStatus;
  additions: number;
  deletions: number;
  content_kind: PatchContentKind;
  patch: string;
  hunks: PatchHunk[];
  message: string | null;
}

export interface DiffResult {
  diff: string;
  files: PrFile[];
  patch_schema_version: number;
  patches: StandardPatchFile[];
}

export interface PrCreatePreview {
  commits: PrCommitSummary[];
  diff: DiffResult;
  incomplete: boolean;
  incomplete_reasons: Array<"platform_limit" | "pagination_failed" | "pagination_limit">;
}

export type DiffSide = "left" | "right";

export interface DiffLocationRequest {
  id: number;
  path: string;
  line: number | null;
  side?: DiffSide | null;
}

export interface DiffLocationResult {
  id: number;
  success: boolean;
  message: string | null;
}

// ── Review ──
export type ReviewEvent = "approve" | "comment" | "request_changes";

export interface CreateReviewRequest {
  body: string;
  event: ReviewEvent;
  comments: ReviewCommentPosition[];
}

export interface ReviewCommentPosition {
  path: string;
  position: number;
  end_line?: number;
  body: string;
}

export interface Review {
  id: number;
  body: string;
  state: string;
  author: User;
  submitted_at: string;
}

export interface PrComment {
  id: number | string;
  body: string;
  path: string;
  line: number | null;
  start_line: number | null;
  side: "left" | "right" | null;
  author: User;
  created_at: string;
  commit_id: string | null;
  original_commit_id: string | null;
  original_line: number | null;
  original_start_line: number | null;
  diff_hunk: string | null;
  thread_id: string;
  reply_to_id: string | null;
  resolved: boolean | null;
  resolvable: boolean;
  can_edit?: boolean;
  can_delete?: boolean;
}

export interface ReviewThreadFileSummary {
  comments: number;
  unresolved: number;
}

export interface ReviewThreadSummary {
  comments: number;
  threads: number;
  unresolved: number;
  by_file: Record<string, ReviewThreadFileSummary>;
}

// ── Issue ──
export type IssueState = "open" | "closed" | "all";

export interface IssueSummary {
  number: number;
  title: string;
  author: User;
  state: IssueState;
  labels: string[];
  created_at: string;
}

export interface Issue {
  number: number;
  title: string;
  body: string;
  author: User;
  state: IssueState;
  labels: string[];
  created_at: string;
  updated_at: string;
}

export interface CreateIssueRequest {
  title: string;
  body: string;
  labels: string[];
}

// ── 分页 ──
export interface Paginated<T> {
  items: T[];
  page: number;
  total_pages: number;
  total_count: number;
}

// ── 仓库 ──
export interface RepoSummary {
  id: number;
  name: string;
  full_name: string;
  owner: string;
  owner_type: string;
  owner_display_name: string;
  description: string;
  private: boolean;
  fork: boolean;
  parent_full_name: string | null;
  parent_owner: string | null;
}

// ── AI ──
export type Severity = "critical" | "major" | "minor" | "info";
export type AiReviewFocus = "all" | "security" | "performance" | "logic" | "code_style";

export interface AiConfig {
  endpoint: string;
  model: string;
  api_key_configured: boolean;
  api_key_encrypted?: string | null;
  system_prompt: string | null;
  temperature: number | null;
  max_tokens: number | null;
}

export interface AiReviewRequest {
  diff: string;
  context: PrContext | null;
  file_filter: string[] | null;
  focus: AiReviewFocus | null;
}

export interface AiStreamEvent<T> {
  request_id: string;
  payload: T;
}

export interface PrContext {
  title: string;
  body: string;
  repository_rules?: string | null;
}

export interface AiReviewResult {
  summary: string;
  suggestions: AiSuggestion[];
}

export interface AiSuggestion {
  file: string;
  line_start: number | null;
  line_end: number | null;
  severity: Severity;
  category: string;
  description: string;
  suggestion: string | null;
  action?: AiSuggestionAction;
}

export type AiSuggestionAction = "accept" | "reject" | "submitted" | { edit: string };

export type AiReviewMode = "full" | "incremental";

export interface AiReviewHistoryEntry {
  id: string;
  created_at: number;
  head_sha: string;
  base_sha: string | null;
  focus: AiReviewFocus;
  mode: AiReviewMode;
  model: string;
  truncated: boolean;
  result: AiReviewResult;
}

// ── AI 预设 ──
export interface AiPreset {
  name: string;
  endpoint: string;
  default_model: string;
}
