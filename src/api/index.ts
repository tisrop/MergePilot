import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import type {
  Platform,
  PlatformCapabilities,
  PrComment,
  PrState,
  PrSummary,
  PrDetail,
  DiffResult,
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

// ── Repo ──
export async function repoList(
  platform: Platform,
  page: number = 1,
): Promise<Paginated<RepoSummary>> {
  return invoke("repo_list", { platform, page });
}

// ── PR ──
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

export async function prDiff(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
): Promise<DiffResult> {
  return invoke("pr_diff", { platform, owner, repo, number });
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
