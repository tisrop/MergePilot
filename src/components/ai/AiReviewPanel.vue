<script setup lang="ts">
import { computed, ref, onUnmounted, watch } from "vue";
import type {
  Platform,
  AiReviewFocus,
  AiReviewHistoryEntry,
  AiReviewMode,
  AiReviewResult,
  AiSuggestion,
  PrContext,
  AiSuggestionAction,
  AiStreamEvent,
} from "@/types";
import {
  aiReview,
  aiReviewCancel,
  aiGetConfig,
  aiReviewStream,
  prCompareDiff,
  prDetail,
  prDiff,
  reviewCommentAdd,
  reviewSubmit,
} from "@/api";
import { getErrorMessage } from "@/utils/error";
import { draftPositionIsCurrent } from "@/utils/aiReviewDraft";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import AiSuggestionCard from "./AiSuggestionCard.vue";
import AppSelect from "@/components/shared/AppSelect.vue";
import {
  appendAiReviewHistory,
  loadAiReviewHistory,
  loadRepositoryRules,
  saveRepositoryRules,
  updateAiReviewHistoryResult,
} from "@/services/aiReviewPersistence";
import { useReviewDraftStore, type UnifiedReviewDraft } from "@/stores/useReviewDraftStore";

const props = defineProps<{
  platform: Platform;
  owner: string;
  repo: string;
  prNumber: number;
  diff: string;
  context: PrContext | null;
  headSha: string;
  supportsCompareDiff: boolean;
}>();

const emit = defineEmits<{
  locateSuggestion: [suggestion: AiSuggestion];
}>();

const focus = ref<AiReviewFocus>("all");
const reviewMode = ref<AiReviewMode>("full");
const useStreaming = ref(true);
const loading = ref(false);
const error = ref("");
const result = ref<AiReviewResult | null>(null);
const resultHeadSha = ref("");
const resultFocus = ref<AiReviewFocus | null>(null);
const resultMode = ref<AiReviewMode>("full");
const resultBaseSha = ref("");
const resultModel = ref("");
const resultTruncated = ref(false);
const currentHistoryId = ref("");
const isResultOutdated = computed(
  () => !!result.value && !!resultHeadSha.value && resultHeadSha.value !== props.headSha,
);

const reviewStorageKey = computed(
  () =>
    `mergebeacon:ai-review-head:${props.platform}:${encodeURIComponent(props.owner)}:${encodeURIComponent(props.repo)}:${props.prNumber}`,
);

function loadLastSuccessfulHeadSha(): string {
  try {
    return localStorage.getItem(reviewStorageKey.value) ?? "";
  } catch {
    return "";
  }
}

const lastSuccessfulHeadSha = ref(loadLastSuccessfulHeadSha());
const repositoryRules = ref(
  loadRepositoryRules({ platform: props.platform, owner: props.owner, repo: props.repo }),
);
const rulesStatus = ref("");
const history = ref<AiReviewHistoryEntry[]>(
  loadAiReviewHistory({
    platform: props.platform,
    owner: props.owner,
    repo: props.repo,
    prNumber: props.prNumber,
  }),
);
const hasIncrementalBase = computed(
  () =>
    props.supportsCompareDiff &&
    !!lastSuccessfulHeadSha.value &&
    lastSuccessfulHeadSha.value !== props.headSha,
);
const incrementalDisabledReason = computed(() => {
  if (!props.supportsCompareDiff) return "当前平台不提供可靠的提交比较接口";
  if (!lastSuccessfulHeadSha.value) return "完成一次成功的完整评审后可用";
  if (lastSuccessfulHeadSha.value === props.headSha) return "当前版本没有新增提交";
  return "";
});
const canStartReview = computed(
  () => !!props.diff && (reviewMode.value === "full" || hasIncrementalBase.value),
);

function saveLastSuccessfulHeadSha(headSha: string, storageKey = reviewStorageKey.value) {
  if (storageKey === reviewStorageKey.value) lastSuccessfulHeadSha.value = headSha;
  try {
    localStorage.setItem(storageKey, headSha);
  } catch {
    // 本地存储不可用时保留内存状态，不影响当前评审。
  }
}

type ReviewDraft = UnifiedReviewDraft & { source: "ai"; suggestionIndex: number };

const reviewDrafts = useReviewDraftStore();
const reviewReference = computed(() => ({
  platform: props.platform,
  owner: props.owner,
  repo: props.repo,
  prNumber: props.prNumber,
}));
function loadAiDrafts(): ReviewDraft[] {
  return reviewDrafts
    .list(reviewReference.value)
    .filter(
      (draft): draft is ReviewDraft => draft.source === "ai" && draft.suggestionIndex !== null,
    )
    .map((draft) => ({ ...draft }));
}
const drafts = ref<ReviewDraft[]>(loadAiDrafts());
watch(drafts, (value) => reviewDrafts.replaceSource(reviewReference.value, "ai", value), {
  deep: true,
});
const submittingDrafts = ref(false);
const draftStatus = ref("");
const draftError = ref("");

const streamReceivedData = ref(false);
const streamStatusText = computed(() =>
  streamReceivedData.value ? "正在整理评审摘要和代码建议…" : "正在连接 AI 评审服务…",
);
let unlistenChunk: UnlistenFn | null = null;
let unlistenDone: UnlistenFn | null = null;
let unlistenError: UnlistenFn | null = null;
let activeRequestId: string | null = null;
let activeReviewHeadSha = "";
let activeReviewFocus: AiReviewFocus | null = null;
let activeReviewDiff = "";
let activeReviewContext: PrContext | null = null;
let activeReviewMode: AiReviewMode = "full";
let activeReviewBaseSha = "";
let activeReviewModel = "未知模型";
let activeReviewTruncated = false;
let activeReviewStorageKey = "";
let historySequence = 0;
let reviewSequence = 0;
let disposed = false;
let resultPersistenceTimer: ReturnType<typeof setTimeout> | null = null;

const foci: { value: AiReviewFocus; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "security", label: "安全" },
  { value: "performance", label: "性能" },
  { value: "logic", label: "逻辑" },
  { value: "code_style", label: "代码风格" },
];

const reviewModes = computed(() => [
  { value: "full", label: "完整变更" },
  {
    value: "incremental",
    label: "仅新增改动",
    disabled: !hasIncrementalBase.value,
  },
]);

watch(
  () => `${props.platform}:${props.owner}:${props.repo}:${props.prNumber}`,
  () => {
    repositoryRules.value = loadRepositoryRules(reviewReference.value);
    history.value = loadAiReviewHistory(reviewReference.value);
    currentHistoryId.value = "";
    rulesStatus.value = "";
    drafts.value = loadAiDrafts();
    restoreDraftHistory();
  },
);

function saveRules(): void {
  repositoryRules.value = saveRepositoryRules(reviewReference.value, repositoryRules.value);
  rulesStatus.value = repositoryRules.value ? "仓库级规则已保存" : "仓库级规则已清除";
}

function reviewContextWithRules(context: PrContext | null): PrContext | null {
  const rules = repositoryRules.value.trim();
  if (!context && !rules) return null;
  return {
    title: context?.title ?? "",
    body: context?.body ?? "",
    repository_rules: rules || null,
  };
}

function cloneResult(reviewResult: AiReviewResult): AiReviewResult {
  return JSON.parse(JSON.stringify(reviewResult)) as AiReviewResult;
}

function recordSuccessfulReview(reviewResult: AiReviewResult): void {
  const createdAt = Date.now();
  const entry: AiReviewHistoryEntry = {
    id: `${activeReviewHeadSha}:${createdAt}:${historySequence++}`,
    created_at: createdAt,
    head_sha: activeReviewHeadSha,
    base_sha: activeReviewBaseSha || null,
    focus: activeReviewFocus ?? "all",
    mode: activeReviewMode,
    model: activeReviewModel,
    truncated: activeReviewTruncated,
    result: cloneResult(reviewResult),
  };
  history.value = appendAiReviewHistory(reviewReference.value, entry);
  currentHistoryId.value = entry.id;
}

function persistCurrentResult(): void {
  if (!result.value || !currentHistoryId.value) return;
  history.value = updateAiReviewHistoryResult(
    reviewReference.value,
    currentHistoryId.value,
    cloneResult(result.value),
  );
}

function scheduleCurrentResultPersistence(): void {
  if (resultPersistenceTimer) clearTimeout(resultPersistenceTimer);
  resultPersistenceTimer = setTimeout(() => {
    resultPersistenceTimer = null;
    persistCurrentResult();
  }, 300);
}

function loadHistoryEntry(entry: AiReviewHistoryEntry): void {
  const draftHistoryIds = new Set(drafts.value.map((draft) => draft.historyId).filter(Boolean));
  if (drafts.value.length > 0 && !draftHistoryIds.has(entry.id)) {
    draftError.value = "请先提交或移除当前草稿，再切换历史评审";
    return;
  }
  result.value = cloneResult(entry.result);
  resultHeadSha.value = entry.head_sha;
  resultFocus.value = entry.focus;
  resultMode.value = entry.mode;
  resultBaseSha.value = entry.base_sha ?? "";
  resultModel.value = entry.model;
  resultTruncated.value = entry.truncated;
  currentHistoryId.value = entry.id;
  draftError.value = "";
}

function restoreDraftHistory(): void {
  const historyId = drafts.value[0]?.historyId;
  if (!historyId) return;
  const entry = history.value.find((candidate) => candidate.id === historyId);
  if (entry) loadHistoryEntry(entry);
}

restoreDraftHistory();

async function startReview() {
  if (drafts.value.length > 0) {
    draftError.value = "已有未提交的评审草稿，请先提交或移除后再重新评审";
    return;
  }
  if (!props.diff) {
    error.value = "没有 diff 数据";
    return;
  }
  if (!props.headSha) {
    error.value = "当前 PR 缺少提交版本，无法开始 AI 评审";
    return;
  }
  if (reviewMode.value === "incremental" && !hasIncrementalBase.value) {
    error.value = incrementalDisabledReason.value || "当前无法生成增量评审 Diff";
    return;
  }

  const sequence = ++reviewSequence;
  const reviewPlatform = props.platform;
  const reviewOwner = props.owner;
  const reviewRepo = props.repo;
  const reviewStorageKeySnapshot = reviewStorageKey.value;
  const reviewHeadSha = props.headSha;
  const reviewDiff = props.diff;
  const reviewContext = reviewContextWithRules(props.context);

  await cancelActiveReview();
  if (disposed || sequence !== reviewSequence) return;
  cleanupListeners();
  loading.value = true;
  error.value = "";
  draftError.value = "";
  streamReceivedData.value = false;
  activeReviewHeadSha = reviewHeadSha;
  activeReviewStorageKey = reviewStorageKeySnapshot;
  activeReviewFocus = focus.value;
  activeReviewContext = reviewContext;
  activeReviewMode = reviewMode.value;
  activeReviewBaseSha = reviewMode.value === "incremental" ? lastSuccessfulHeadSha.value : "";
  activeReviewModel = "未知模型";
  try {
    activeReviewModel = (await aiGetConfig()).model || "未知模型";
  } catch {
    // Model metadata is informative and must not block a configured review request.
  }
  if (disposed || sequence !== reviewSequence) return;

  try {
    if (activeReviewMode === "incremental") {
      const compared = await prCompareDiff(
        reviewPlatform,
        reviewOwner,
        reviewRepo,
        activeReviewBaseSha,
        activeReviewHeadSha,
      );
      if (!compared.diff.trim()) {
        throw new Error("平台未返回可评审的新增文本 Diff，未自动改用完整评审");
      }
      activeReviewDiff = compared.diff;
    } else {
      activeReviewDiff = reviewDiff;
    }
  } catch (e) {
    if (disposed || sequence !== reviewSequence) return;
    loading.value = false;
    error.value = getErrorMessage(e, "获取增量评审 Diff 失败");
    return;
  }
  if (disposed || sequence !== reviewSequence) return;
  activeReviewTruncated = new TextEncoder().encode(activeReviewDiff).length > 65_536;

  result.value = null;
  resultHeadSha.value = "";
  resultFocus.value = null;
  resultMode.value = activeReviewMode;
  resultBaseSha.value = activeReviewBaseSha;
  resultModel.value = activeReviewModel;
  resultTruncated.value = activeReviewTruncated;
  currentHistoryId.value = "";

  if (useStreaming.value) {
    await startStreamingReview();
  } else {
    await startNonStreamingReview();
  }
}

defineExpose({ startReview });

async function startNonStreamingReview() {
  try {
    const reviewHeadSha = activeReviewHeadSha;
    const reviewFocus = activeReviewFocus;
    result.value = await aiReview({
      diff: activeReviewDiff,
      context: activeReviewContext,
      file_filter: null,
      focus: reviewFocus,
    });
    resultHeadSha.value = reviewHeadSha;
    resultFocus.value = reviewFocus;
    resultMode.value = activeReviewMode;
    resultBaseSha.value = activeReviewBaseSha;
    resultModel.value = activeReviewModel;
    resultTruncated.value = activeReviewTruncated;
    saveLastSuccessfulHeadSha(reviewHeadSha, activeReviewStorageKey);
    recordSuccessfulReview(result.value);
  } catch (e) {
    error.value = getErrorMessage(e, "AI 评审失败");
  } finally {
    loading.value = false;
  }
}

async function startStreamingReview() {
  const requestId = crypto.randomUUID();
  activeRequestId = requestId;
  try {
    unlistenChunk = await listen<AiStreamEvent<string>>("ai-review-chunk", (event) => {
      if (event.payload.request_id !== activeRequestId) return;
      streamReceivedData.value = true;
    });

    unlistenDone = await listen<AiStreamEvent<AiReviewResult>>("ai-review-done", (event) => {
      if (event.payload.request_id !== activeRequestId) return;
      result.value = event.payload.payload;
      resultHeadSha.value = activeReviewHeadSha;
      resultFocus.value = activeReviewFocus;
      resultMode.value = activeReviewMode;
      resultBaseSha.value = activeReviewBaseSha;
      resultModel.value = activeReviewModel;
      resultTruncated.value = activeReviewTruncated;
      saveLastSuccessfulHeadSha(activeReviewHeadSha, activeReviewStorageKey);
      recordSuccessfulReview(result.value);
      activeRequestId = null;
      loading.value = false;
      cleanupListeners();
    });

    unlistenError = await listen<AiStreamEvent<string>>("ai-review-error", (event) => {
      if (event.payload.request_id !== activeRequestId) return;
      error.value = event.payload.payload;
      activeRequestId = null;
      loading.value = false;
      cleanupListeners();
    });

    await aiReviewStream(requestId, {
      diff: activeReviewDiff,
      context: activeReviewContext,
      file_filter: null,
      focus: activeReviewFocus,
    });
  } catch (e) {
    if (activeRequestId === requestId) {
      activeRequestId = null;
      error.value = getErrorMessage(e, "AI 流式评审启动失败");
      loading.value = false;
      cleanupListeners();
    }
  }
}

async function cancelActiveReview() {
  const requestId = activeRequestId;
  activeRequestId = null;
  if (!requestId) return;
  try {
    await aiReviewCancel(requestId);
  } catch {
    // Cancellation is best-effort and must not be presented as an AI review error.
  }
}

function cleanupListeners() {
  unlistenChunk?.();
  unlistenDone?.();
  unlistenError?.();
  unlistenChunk = null;
  unlistenDone = null;
  unlistenError = null;
}

onUnmounted(() => {
  disposed = true;
  reviewSequence += 1;
  if (resultPersistenceTimer) {
    clearTimeout(resultPersistenceTimer);
    resultPersistenceTimer = null;
    persistCurrentResult();
  }
  reviewDrafts.flushPersistence();
  void cancelActiveReview().finally(cleanupListeners);
});

function draftBody(index: number): string {
  const suggestion = result.value?.suggestions[index];
  if (!suggestion) return "";
  return suggestion.suggestion
    ? `${suggestion.description}\n\n建议修改：\n${suggestion.suggestion}`
    : suggestion.description;
}

function onAction(index: number, action: AiSuggestionAction) {
  if (!result.value || isResultOutdated.value) return;
  if (action === "reject") {
    result.value.suggestions[index].action = action;
    drafts.value = drafts.value.filter((draft) => draft.suggestionIndex !== index);
    persistCurrentResult();
    return;
  }

  const suggestion = result.value.suggestions[index];
  if (!drafts.value.some((draft) => draft.suggestionIndex === index)) {
    drafts.value.push({
      id: `${resultHeadSha.value}:${index}`,
      source: "ai",
      suggestionIndex: index,
      path: suggestion.file,
      startLine: suggestion.line_start,
      endLine: suggestion.line_end ?? suggestion.line_start,
      body: draftBody(index),
      headSha: resultHeadSha.value,
      event: "comment",
      historyId: currentHistoryId.value || null,
      touchedAt: Date.now(),
    });
  }
  suggestion.action = typeof action === "object" ? { edit: draftBody(index) } : "accept";
  draftStatus.value = "";
  draftError.value = "";
  persistCurrentResult();
}

function removeDraft(index: number) {
  const [removed] = drafts.value.splice(index, 1);
  if (removed && result.value?.suggestions[removed.suggestionIndex]) {
    result.value.suggestions[removed.suggestionIndex].action = undefined;
    persistCurrentResult();
  }
}

function recordDraftEdit(draft: ReviewDraft): void {
  draft.touchedAt = Date.now();
  const suggestion = result.value?.suggestions[draft.suggestionIndex];
  if (!suggestion) return;
  suggestion.action = draft.body.trim() ? { edit: draft.body } : "accept";
  scheduleCurrentResultPersistence();
}

async function validateDraftsAgainstCurrentRevision(): Promise<boolean> {
  try {
    const latestDetail = await prDetail(props.platform, props.owner, props.repo, props.prNumber);
    if (
      latestDetail.head_sha !== props.headSha ||
      drafts.value.some((draft) => draft.headSha !== latestDetail.head_sha)
    ) {
      draftError.value = "PR 已有新提交，草稿版本校验失败，请刷新 Diff 并重新评审";
      return false;
    }
    const inlineDrafts = drafts.value.filter((draft) => draft.path && draft.endLine);
    if (inlineDrafts.length === 0) return true;
    const latestDiff = await prDiff(props.platform, props.owner, props.repo, props.prNumber);
    const invalidDraft = inlineDrafts.find(
      (draft) => !draftPositionIsCurrent(draft, latestDiff.patches),
    );
    if (invalidDraft) {
      draftError.value = `草稿位置已失效：${invalidDraft.path}:${invalidDraft.endLine}，请刷新 Diff 后重新定位`;
      return false;
    }
    return true;
  } catch (cause) {
    draftError.value = `提交前校验失败：${getErrorMessage(cause, "无法读取最新 PR 版本和 Diff")}`;
    return false;
  }
}

async function submitDrafts() {
  if (submittingDrafts.value || drafts.value.length === 0) return;
  if (drafts.value.some((draft) => draft.headSha !== props.headSha)) {
    draftError.value = "PR 已有新提交，旧版本评审草稿不能提交，请重新评审后确认";
    return;
  }
  if (drafts.value.some((draft) => !draft.body.trim())) {
    draftError.value = "评审草稿内容不能为空";
    return;
  }
  if (!(await validateDraftsAgainstCurrentRevision())) return;

  submittingDrafts.value = true;
  draftStatus.value = "";
  draftError.value = "";
  const failed: ReviewDraft[] = [];
  const submittedSuggestionIndexes: number[] = [];
  let submitted = 0;
  let firstError = "";
  for (const draft of drafts.value) {
    try {
      if (draft.path && draft.endLine && draft.endLine > 0) {
        await reviewCommentAdd(
          props.platform,
          props.owner,
          props.repo,
          props.prNumber,
          draft.headSha,
          draft.path,
          draft.startLine && draft.startLine !== draft.endLine ? draft.startLine : null,
          draft.endLine,
          "right",
          draft.body.trim(),
        );
      } else {
        await reviewSubmit(
          props.platform,
          props.owner,
          props.repo,
          props.prNumber,
          draft.body.trim(),
          "comment",
          [],
        );
      }
      submitted++;
      submittedSuggestionIndexes.push(draft.suggestionIndex);
    } catch (cause) {
      failed.push(draft);
      if (!firstError) firstError = getErrorMessage(cause, "提交失败");
    }
  }
  drafts.value = failed;
  for (const suggestionIndex of submittedSuggestionIndexes) {
    if (result.value?.suggestions[suggestionIndex]) {
      result.value.suggestions[suggestionIndex].action = "submitted";
    }
  }
  persistCurrentResult();
  submittingDrafts.value = false;
  if (failed.length > 0) {
    draftError.value = `已提交 ${submitted} 条，${failed.length} 条失败：${firstError}`;
  } else {
    draftStatus.value = `已提交 ${submitted} 条评审意见`;
  }
}
</script>

<template>
  <div class="ai-panel">
    <div class="ai-toolbar">
      <div class="review-mode-select">
        <label for="ai-review-mode">范围:</label>
        <div class="review-mode-control">
          <AppSelect id="ai-review-mode" v-model="reviewMode" size="sm" :options="reviewModes" />
        </div>
      </div>
      <div v-if="!hasIncrementalBase" class="incremental-hint" role="status">
        增量评审：{{ incrementalDisabledReason }}
      </div>
      <div class="focus-select">
        <label>聚焦:</label>
        <AppSelect v-model="focus" :options="foci" />
      </div>

      <label class="stream-toggle" title="边生成边接收评审结果，不展示原始 JSON">
        <input type="checkbox" v-model="useStreaming" />
        实时生成
      </label>

      <button
        class="btn btn-primary"
        :disabled="(loading && !useStreaming) || !canStartReview"
        @click="startReview"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polygon points="5 3 19 12 5 21 5 3" />
        </svg>
        {{ loading ? (useStreaming ? "重新评审" : "评审中...") : "开始 AI 评审" }}
      </button>
    </div>

    <div class="ai-context-tools">
      <details class="repository-rules">
        <summary>仓库级 AI 规则</summary>
        <p>规则按平台和仓库保存在本机，并随每次 AI 评审发送给当前模型。</p>
        <textarea
          v-model="repositoryRules"
          class="input"
          rows="4"
          maxlength="12000"
          placeholder="例如：重点检查异步生命周期；禁止建议引入新的 UI 框架。"
          aria-label="仓库级 AI 评审规则"
          @input="rulesStatus = ''"
        />
        <div class="rules-actions">
          <button class="btn btn-sm" type="button" @click="saveRules">保存规则</button>
          <span v-if="rulesStatus" role="status">{{ rulesStatus }}</span>
        </div>
      </details>

      <details v-if="history.length > 0" class="review-history">
        <summary>评审历史（{{ history.length }}）</summary>
        <div class="history-list">
          <button
            v-for="entry in history"
            :key="entry.id"
            type="button"
            class="history-entry"
            :class="{ active: entry.id === currentHistoryId }"
            @click="loadHistoryEntry(entry)"
          >
            <span
              ><code>{{ entry.head_sha.slice(0, 12) }}</code> · {{ entry.model }}</span
            >
            <small>{{ new Date(entry.created_at).toLocaleString() }}</small>
          </button>
        </div>
      </details>
    </div>

    <!-- Streaming progress: keep the transport detail out of the user-facing review UI. -->
    <div v-if="loading && useStreaming" class="stream-preview" role="status" aria-live="polite">
      <div class="stream-label">
        <span class="stream-dot" />
        {{ streamStatusText }}
      </div>
      <div class="stream-progress" aria-hidden="true">
        <span :class="{ active: streamReceivedData }" />
      </div>
      <p class="stream-hint">评审完成后将显示结构化摘要和代码建议</p>
    </div>

    <!-- Non-streaming loading -->
    <div v-if="loading && !useStreaming" class="loading-state">
      <div class="spinner" />
      <p>AI 正在分析代码变更，请稍候...</p>
    </div>

    <div v-if="error" class="error-box">
      {{ error }}
    </div>

    <div v-if="result" class="ai-result">
      <div v-if="isResultOutdated" class="outdated-warning" role="alert">
        当前 PR 已有新提交，此 AI 评审基于旧版本，建议重新评审后再处理建议。
      </div>
      <div class="review-metadata">
        <span
          >评审版本：<code>{{ resultHeadSha.slice(0, 12) }}</code></span
        >
        <span
          >评审范围：{{
            resultMode === "incremental" ? "上次成功评审后的新增改动" : "完整变更"
          }}</span
        >
        <span v-if="resultBaseSha"
          >基线版本：<code>{{ resultBaseSha.slice(0, 12) }}</code></span
        >
        <span v-if="resultFocus"
          >聚焦范围：{{ foci.find((item) => item.value === resultFocus)?.label }}</span
        >
        <span
          >模型：<code>{{ resultModel || "未知模型" }}</code></span
        >
        <span>输入状态：{{ resultTruncated ? "Diff 已截断至 64 KiB" : "完整 Diff" }}</span>
      </div>
      <div class="summary-card">
        <h4>
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M12 2a4 4 0 0 1 4 4c0 2-2 4-4 4s-4-2-4-4a4 4 0 0 1 4-4z" />
            <path d="M12 14c-4.42 0-8 1.79-8 4v2h16v-2c0-2.21-3.58-4-8-4z" />
          </svg>
          AI 评审总览
        </h4>
        <p>{{ result.summary }}</p>
      </div>

      <div v-if="result.suggestions.length > 0" class="suggestions">
        <h4>发现 {{ result.suggestions.length }} 个问题</h4>
        <AiSuggestionCard
          v-for="(s, idx) in result.suggestions"
          :key="idx"
          :suggestion="s"
          :disabled="isResultOutdated"
          @action="(a: AiSuggestionAction) => onAction(idx, a)"
          @locate="emit('locateSuggestion', s)"
        />
      </div>

      <section v-if="drafts.length > 0" class="draft-panel">
        <div class="draft-header">
          <div>
            <h4>评审草稿</h4>
            <p>确认并编辑后统一提交到 {{ platform }}，提交前不会写入远端。</p>
          </div>
          <button
            class="btn btn-primary"
            :disabled="submittingDrafts || isResultOutdated"
            @click="submitDrafts"
          >
            {{ submittingDrafts ? "提交中..." : `提交 ${drafts.length} 条草稿` }}
          </button>
        </div>
        <article v-for="(draft, index) in drafts" :key="draft.id" class="draft-item">
          <div class="draft-location">
            <span v-if="draft.path">
              {{ draft.path
              }}<template v-if="draft.endLine"
                >:{{ draft.startLine ?? draft.endLine
                }}<template v-if="draft.startLine && draft.startLine !== draft.endLine"
                  >-{{ draft.endLine }}</template
                ></template
              >
            </span>
            <span v-else>整体评审意见</span>
            <button class="btn btn-sm" :disabled="submittingDrafts" @click="removeDraft(index)">
              移除
            </button>
          </div>
          <textarea
            v-model="draft.body"
            class="input"
            rows="5"
            aria-label="评审草稿内容"
            @input="recordDraftEdit(draft)"
          />
        </article>
      </section>

      <p v-if="draftStatus" class="draft-success" role="status">{{ draftStatus }}</p>
      <p v-if="draftError" class="error-box" role="alert">{{ draftError }}</p>

      <div v-if="result.suggestions.length === 0" class="no-issues">
        <svg
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
          <polyline points="22 4 12 14.01 9 11.01" />
        </svg>
        <p>AI 未发现明显问题</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ai-panel {
  padding: var(--space-1) 0;
}

.ai-toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin-bottom: var(--space-4);
  flex-wrap: wrap;
  padding: var(--space-3);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
}

.ai-context-tools {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
  margin-bottom: var(--space-4);
}

.repository-rules,
.review-history {
  padding: var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-surface);
}

.repository-rules summary,
.review-history summary {
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 13px;
  font-weight: 650;
}

.repository-rules p {
  margin: var(--space-2) 0;
  color: var(--color-text-tertiary);
  font-size: 12px;
  line-height: 1.5;
}

.repository-rules textarea {
  width: 100%;
  resize: vertical;
}

.rules-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-top: var(--space-2);
  color: var(--color-success);
  font-size: 12px;
}

.history-list {
  display: flex;
  max-height: 220px;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-2);
  overflow-y: auto;
}

.history-entry {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-2);
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: var(--color-bg);
  color: var(--color-text-secondary);
  font-size: 12px;
  text-align: left;
}

.history-entry:hover,
.history-entry.active {
  border-color: var(--color-primary-border);
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.history-entry code {
  font-family: var(--font-mono);
}

.history-entry small {
  flex: 0 0 auto;
  color: var(--color-text-tertiary);
}

.review-mode-select {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: 13px;
}

.review-mode-control {
  width: 124px;
}

.incremental-hint {
  max-width: 260px;
  color: var(--color-text-tertiary);
  font-size: 12px;
  line-height: 1.35;
}

.focus-select {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: 13px;
}

.stream-toggle {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: 12px;
  color: var(--color-text-secondary);
  cursor: pointer;
  user-select: none;
}

.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-10);
  color: var(--color-text-tertiary);
}

.spinner {
  width: 24px;
  height: 24px;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.stream-preview {
  margin-bottom: var(--space-4);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.stream-label {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: var(--color-surface-hover);
  font-size: 12px;
  color: var(--color-text-secondary);
  border-bottom: 1px solid var(--color-border);
}

.stream-dot {
  width: 8px;
  height: 8px;
  background: var(--color-primary);
  border-radius: 50%;
  animation: pulse-dot 1.5s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.3;
  }
}

.stream-progress {
  height: 4px;
  margin: 0 var(--space-3);
  overflow: hidden;
  border-radius: var(--radius-sm);
  background: var(--color-border-light);
}

.stream-progress span {
  display: block;
  width: 30%;
  height: 100%;
  border-radius: inherit;
  background: var(--color-primary);
  animation: stream-progress 1.6s ease-in-out infinite;
}

.stream-progress span.active {
  width: 62%;
}

.stream-hint {
  margin: 0;
  padding: var(--space-2) var(--space-3) var(--space-3);
  color: var(--color-text-tertiary);
  font-size: 12px;
}

@keyframes stream-progress {
  0% {
    transform: translateX(-120%);
  }
  100% {
    transform: translateX(180%);
  }
}

.error-box {
  padding: var(--space-3);
  background: var(--color-danger-light);
  color: var(--color-danger);
  border: 1px solid var(--color-danger-border);
  border-radius: var(--radius-md);
  margin-bottom: var(--space-4);
  font-size: 13px;
}

.ai-result {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.draft-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--color-primary-border);
  border-radius: var(--radius-lg);
  background: var(--color-primary-light);
}

.draft-header,
.draft-location {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.draft-header h4 {
  margin-bottom: 2px;
}

.draft-header p,
.draft-location {
  color: var(--color-text-secondary);
  font-size: 12px;
}

.draft-item {
  padding: var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface);
}

.draft-location {
  margin-bottom: var(--space-2);
  font-family: var(--font-mono);
}

.draft-item textarea {
  width: 100%;
  resize: vertical;
}

.draft-success {
  color: var(--color-success);
  font-size: 13px;
}

.outdated-warning {
  padding: var(--space-3);
  border: 1px solid var(--color-warning-border);
  border-radius: var(--radius-md);
  background: var(--color-warning-light);
  color: var(--color-warning);
  font-size: 13px;
}

.review-metadata {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2) var(--space-4);
  color: var(--color-text-secondary);
  font-size: 12px;
}

.review-metadata code {
  font-family: var(--font-mono);
}

.summary-card {
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
}

.summary-card h4 {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
  font-size: 15px;
  font-weight: 600;
}

.summary-card p {
  font-size: 14px;
  line-height: 1.6;
  color: var(--color-text-secondary);
}

.suggestions h4 {
  margin-bottom: var(--space-3);
  font-size: 15px;
  font-weight: 600;
}

.no-issues {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-10);
  color: var(--color-success);
  font-size: 15px;
}

.no-issues svg {
  opacity: 0.6;
}

@media (max-width: 760px) {
  .ai-context-tools {
    grid-template-columns: 1fr;
  }
}
</style>
