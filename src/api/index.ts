import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import { normalizeApiError } from "@/api/errors";
export { ApiError, commandErrorCode, normalizeApiError } from "@/api/errors";
import type {
  Platform,
  PlatformCapabilities,
  PrComment,
  PrState,
  PrSummary,
  ReviewInboxCategory,
  ReviewInboxItem,
  PrDetail,
  PrDependencyGraph,
  PrMergeQueueStatus,
  PrBranchOptions,
  PrLabel,
  PrCreatePreview,
  PrCreatePreviewRequest,
  PrMetadataUpdate,
  PrMetadataUpdateOutcome,
  PrCreateRequest,
  PrCreateOutcome,
  PrMergeReadiness,
  DiffResult,
  PrFileContent,
  MergeStrategy,
  PrMergeOutcome,
  Review,
  ReviewCommentPosition,
  IssueState,
  IssueSummary,
  Issue,
  Paginated,
  RepoSummary,
  User,
  AuthLoginResult,
  AiConfig,
  AiReviewRequest,
  AiReviewResult,
  SupportInfo,
  UpdateCheckResult,
  UpdateProgressEvent,
} from "@/types";

// ============================================================
// Tauri IPC 封装 —— 所有后端调用统一入口
// ============================================================

const ERROR_LOG_RECORD_COMMAND = "error_log_record";
let errorRequestSequence = 0;

function createFallbackErrorRequestId(): string {
  try {
    return globalThis.crypto.randomUUID();
  } catch {
    errorRequestSequence = (errorRequestSequence + 1) % Number.MAX_SAFE_INTEGER;
    return `error-${Date.now().toString(36)}-${errorRequestSequence.toString(36)}`;
  }
}

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return args === undefined ? await tauriInvoke<T>(command) : await tauriInvoke<T>(command, args);
  } catch (error) {
    const normalized = normalizeApiError(error);
    if (command !== ERROR_LOG_RECORD_COMMAND) {
      void tauriInvoke(ERROR_LOG_RECORD_COMMAND, {
        record: {
          command,
          requestId: normalized.requestId ?? createFallbackErrorRequestId(),
          code: normalized.code,
          retryable: normalized.retryable,
          httpStatus: normalized.httpStatus ?? null,
        },
      }).catch(() => undefined);
    }
    throw normalized;
  }
}

export interface DesktopNotificationPayload {
  id: number;
  title: string;
  body: string;
  group: string;
  extra: Record<string, unknown>;
  actionable?: boolean;
}

export function isDesktopRuntime(): boolean {
  return isTauri();
}

export function desktopNotificationPermissionGranted(): Promise<boolean> {
  return invoke("desktop_notification_permission_granted");
}

export function requestDesktopNotificationPermission(): Promise<boolean> {
  return invoke("desktop_notification_request_permission");
}

export function sendDesktopNotification(payload: DesktopNotificationPayload): Promise<void> {
  return invoke("desktop_notification_send", { payload });
}

export async function listenDesktopNotificationActions(
  callback: (extra: Record<string, unknown>) => void,
): Promise<UnlistenFn> {
  return listen<Record<string, unknown>>("desktop-notification-action", (event) =>
    callback(event.payload),
  );
}

// ── Auth ──
export async function authLogin(
  platform: Platform,
  token: string,
  customUrl?: string,
): Promise<AuthLoginResult> {
  return invoke("auth_login", { platform, token, customUrl: customUrl ?? null });
}

export async function authLogout(platform: Platform): Promise<void> {
  return invoke("auth_logout", { platform });
}

export async function authCheck(platform: Platform): Promise<User | null> {
  return invoke("auth_check", { platform });
}

export async function authHasAnyToken(): Promise<boolean> {
  return invoke("auth_has_any_token");
}

export async function authHasToken(platform: Platform): Promise<boolean> {
  return invoke("auth_has_token", { platform });
}

export async function getPlatformCapabilities(platform: Platform): Promise<PlatformCapabilities> {
  return invoke("platform_capabilities", { platform });
}

// ── Support ──
export async function getAppVersion(): Promise<string> {
  return invoke("app_version");
}

export async function checkForUpdates(): Promise<UpdateCheckResult> {
  return invoke("update_check");
}

export async function openExternalUrl(url: string): Promise<void> {
  return open(url);
}

export async function downloadAndInstallUpdate(
  requestId: string,
  expectedVersion: string,
): Promise<void> {
  return invoke("update_download_and_install", { requestId, expectedVersion });
}

export async function restartAfterUpdate(): Promise<void> {
  return invoke("update_restart");
}

export async function listenToUpdateProgress(
  callback: (event: UpdateProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<UpdateProgressEvent>("update-progress", (event) => callback(event.payload));
}

export async function getSupportInfo(platform: Platform): Promise<SupportInfo> {
  return invoke("support_info", { platform });
}

export async function copySupportInfo(platform: Platform): Promise<void> {
  return invoke("copy_support_info", { platform });
}

export async function copyRecentErrorLogs(): Promise<number> {
  return invoke("copy_recent_error_logs");
}

// ── Repo ──
export async function repoList(
  platform: Platform,
  page: number = 1,
): Promise<Paginated<RepoSummary>> {
  return invoke("repo_list", { platform, page });
}

// ── PR ──
export async function reviewInboxList(
  platform: Platform,
  category: ReviewInboxCategory = "review_requested",
  page: number = 1,
  perPage: number = 20,
): Promise<Paginated<ReviewInboxItem>> {
  return invoke("review_inbox_list", { platform, category, page, perPage });
}

export async function prList(
  platform: Platform,
  owner: string,
  repo: string,
  state: PrState = "open",
  page: number = 1,
  perPage: number = 20,
): Promise<Paginated<PrSummary>> {
  return invoke("pr_list", { platform, owner, repo, stateFilter: state, page, perPage });
}

export async function prDetail(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
): Promise<PrDetail> {
  return invoke("pr_detail", { platform, owner, repo, number });
}

export async function prDependencies(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
): Promise<PrDependencyGraph> {
  return invoke("pr_dependencies", { platform, owner, repo, number });
}

export async function prMergeQueueStatus(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
): Promise<PrMergeQueueStatus> {
  return invoke("pr_merge_queue_status", { platform, owner, repo, number });
}

export async function prBranches(
  platform: Platform,
  owner: string,
  repo: string,
): Promise<PrBranchOptions> {
  return invoke("pr_branches", { platform, owner, repo });
}

export async function prLabels(
  platform: Platform,
  owner: string,
  repo: string,
): Promise<PrLabel[]> {
  return invoke("pr_labels", { platform, owner, repo });
}

export async function prParticipantSuggestions(
  platform: Platform,
  owner: string,
  repo: string,
): Promise<User[]> {
  return invoke("pr_participant_suggestions", { platform, owner, repo });
}

export async function prCreate(
  platform: Platform,
  owner: string,
  repo: string,
  request: PrCreateRequest,
): Promise<PrCreateOutcome> {
  return invoke("pr_create", { platform, owner, repo, request });
}

export async function prCreatePreview(
  platform: Platform,
  owner: string,
  repo: string,
  request: PrCreatePreviewRequest,
): Promise<PrCreatePreview> {
  return invoke("pr_create_preview", { platform, owner, repo, request });
}

export async function prMetadataUpdate(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
  update: PrMetadataUpdate,
): Promise<PrMetadataUpdateOutcome> {
  return invoke("pr_metadata_update", { platform, owner, repo, number, update });
}

export async function prMergeReadiness(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
): Promise<PrMergeReadiness> {
  return invoke("pr_merge_readiness", { platform, owner, repo, number });
}

export async function prDiff(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
): Promise<DiffResult> {
  return invoke("pr_diff", { platform, owner, repo, number });
}

export async function prCompareDiff(
  platform: Platform,
  owner: string,
  repo: string,
  baseSha: string,
  headSha: string,
): Promise<DiffResult> {
  return invoke("pr_compare_diff", { platform, owner, repo, baseSha, headSha });
}

export async function prFileContent(
  platform: Platform,
  owner: string,
  repo: string,
  path: string,
  revision: string,
): Promise<PrFileContent> {
  return invoke("pr_file_content", { platform, owner, repo, path, revision });
}

export async function prMerge(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
  strategy: MergeStrategy,
  commitTitle?: string,
  commitMessage?: string,
  closeIssues?: boolean,
): Promise<PrMergeOutcome> {
  return invoke("pr_merge", {
    platform,
    owner,
    repo,
    number,
    strategy,
    commitTitle: commitTitle ?? null,
    commitMessage: commitMessage ?? null,
    closeIssues: closeIssues ?? null,
  });
}

export async function prClose(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
): Promise<PrState> {
  return invoke("pr_close", { platform, owner, repo, number });
}

export async function prReopen(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
): Promise<PrState> {
  return invoke("pr_reopen", { platform, owner, repo, number });
}

// ── Review ──
export async function reviewSubmit(
  platform: Platform,
  owner: string,
  repo: string,
  prNumber: number,
  body: string,
  event: string,
  comments: ReviewCommentPosition[],
): Promise<Review> {
  return invoke("review_submit", { platform, owner, repo, prNumber, body, event, comments });
}

export async function reviewList(
  platform: Platform,
  owner: string,
  repo: string,
  prNumber: number,
): Promise<Review[]> {
  return invoke("review_list", { platform, owner, repo, prNumber });
}

export async function reviewCommentsList(
  platform: Platform,
  owner: string,
  repo: string,
  prNumber: number,
): Promise<PrComment[]> {
  return invoke("review_comments_list", { platform, owner, repo, prNumber });
}

export async function reviewThreadSetResolved(
  platform: Platform,
  owner: string,
  repo: string,
  prNumber: number,
  threadId: string,
  resolved: boolean,
): Promise<void> {
  return invoke("review_thread_set_resolved", {
    platform,
    owner,
    repo,
    prNumber,
    threadId,
    resolved,
  });
}

export async function reviewThreadReply(
  platform: Platform,
  owner: string,
  repo: string,
  prNumber: number,
  threadId: string,
  replyToId: string,
  body: string,
): Promise<void> {
  return invoke("review_thread_reply", {
    platform,
    owner,
    repo,
    prNumber,
    threadId,
    replyToId,
    body,
  });
}

export async function reviewCommentUpdate(
  platform: Platform,
  owner: string,
  repo: string,
  prNumber: number,
  threadId: string,
  commentId: string,
  body: string,
): Promise<void> {
  return invoke("review_comment_update", {
    platform,
    owner,
    repo,
    prNumber,
    threadId,
    commentId,
    body,
  });
}

export async function reviewCommentDelete(
  platform: Platform,
  owner: string,
  repo: string,
  prNumber: number,
  threadId: string,
  commentId: string,
): Promise<void> {
  return invoke("review_comment_delete", {
    platform,
    owner,
    repo,
    prNumber,
    threadId,
    commentId,
  });
}

export async function reviewViewedFilesList(
  platform: Platform,
  owner: string,
  repo: string,
  prNumber: number,
): Promise<string[]> {
  return invoke("review_viewed_files_list", { platform, owner, repo, prNumber });
}

export async function reviewFileSetViewed(
  platform: Platform,
  owner: string,
  repo: string,
  prNumber: number,
  path: string,
  viewed: boolean,
): Promise<void> {
  return invoke("review_file_set_viewed", { platform, owner, repo, prNumber, path, viewed });
}

export async function reviewCommentAdd(
  platform: Platform,
  owner: string,
  repo: string,
  prNumber: number,
  commitId: string,
  path: string,
  startLine: number | null,
  line: number,
  side: string,
  body: string,
  diffHunk?: string,
): Promise<PrComment> {
  return invoke("review_comment_add", {
    platform,
    owner,
    repo,
    prNumber,
    commitId,
    path,
    startLine,
    line,
    side,
    body,
    diffHunk: diffHunk ?? null,
  });
}

// ── Issue ──
export async function issueList(
  platform: Platform,
  owner: string,
  repo: string,
  state: IssueState = "open",
  page: number = 1,
): Promise<Paginated<IssueSummary>> {
  return invoke("issue_list", { platform, owner, repo, stateFilter: state, page });
}

export async function issueCreate(
  platform: Platform,
  owner: string,
  repo: string,
  title: string,
  body: string,
  labels: string[],
): Promise<Issue> {
  return invoke("issue_create", { platform, owner, repo, title, body, labels });
}

// ── AI ──
export async function aiGetConfig(): Promise<AiConfig> {
  return invoke("ai_get_config");
}

export async function aiSaveConfig(config: AiConfig): Promise<void> {
  return invoke("ai_save_config", { config });
}

export async function aiSaveApiKey(apiKey: string): Promise<void> {
  return invoke("ai_save_api_key", { apiKey });
}

export async function aiReview(request: AiReviewRequest): Promise<AiReviewResult> {
  return invoke("ai_review", { request });
}

export async function aiReviewStream(requestId: string, request: AiReviewRequest): Promise<void> {
  return invoke("ai_review_stream", { requestId, request });
}

export async function aiReviewCancel(requestId: string): Promise<void> {
  return invoke("ai_review_cancel", { requestId });
}

export async function aiListModels(endpoint: string): Promise<string[]> {
  return invoke("ai_list_models", { endpoint });
}

export async function aiTestConnection(): Promise<boolean> {
  return invoke("ai_test_connection");
}
