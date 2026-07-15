// ============================================================
// 数据模型 —— 前后端一致的类型定义
// ============================================================

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

/** 平台 API 的静态协议能力，不包含登录、Token 权限或 PR 运行时状态。 */
export interface PlatformCapabilities {
  platform: Platform;
  review_events: ReviewEvent[];
  merge_strategies: MergeStrategy[];
  supports_fork_context: boolean;
  supports_issue_auto_close: boolean;
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
}

export interface PrDetail {
  summary: PrSummary;
  body: string;
  source_branch: string;
  target_branch: string;
  mergeable: boolean | null;
  head_sha: string;
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

export interface DiffResult {
  diff: string;
  files: PrFile[];
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
  id: number;
  body: string;
  path: string;
  line: number | null;
  start_line: number | null;
  author: User;
  created_at: string;
  commit_id: string | null;
  original_commit_id: string | null;
  original_line: number | null;
  original_start_line: number | null;
  diff_hunk: string | null;
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

// ── AI 预设 ──
export interface AiPreset {
  name: string;
  endpoint: string;
  default_model: string;
}
