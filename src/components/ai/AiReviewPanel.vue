<script setup lang="ts">
import { computed, ref, onUnmounted } from "vue";
import type {
  Platform,
  AiReviewFocus,
  AiReviewResult,
  PrContext,
  AiSuggestionAction,
  AiStreamEvent,
} from "@/types";
import { aiReview, aiReviewCancel, aiReviewStream, reviewCommentAdd, reviewSubmit } from "@/api";
import { getErrorMessage } from "@/utils/error";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import AiSuggestionCard from "./AiSuggestionCard.vue";
import AppSelect from "@/components/shared/AppSelect.vue";

const props = defineProps<{
  platform: Platform;
  owner: string;
  repo: string;
  prNumber: number;
  diff: string;
  context: PrContext | null;
  headSha: string;
}>();

const focus = ref<AiReviewFocus>("all");
const useStreaming = ref(true);
const loading = ref(false);
const error = ref("");
const result = ref<AiReviewResult | null>(null);
const resultHeadSha = ref("");
const resultFocus = ref<AiReviewFocus | null>(null);
const isResultOutdated = computed(
  () => !!result.value && !!resultHeadSha.value && resultHeadSha.value !== props.headSha,
);

interface ReviewDraft {
  id: string;
  suggestionIndex: number;
  path: string;
  startLine: number | null;
  endLine: number | null;
  body: string;
  headSha: string;
}

const drafts = ref<ReviewDraft[]>([]);
const submittingDrafts = ref(false);
const draftStatus = ref("");
const draftError = ref("");

const streamText = ref("");
let unlistenChunk: UnlistenFn | null = null;
let unlistenDone: UnlistenFn | null = null;
let unlistenError: UnlistenFn | null = null;
let activeRequestId: string | null = null;
let activeReviewHeadSha = "";
let activeReviewFocus: AiReviewFocus | null = null;
let activeReviewDiff = "";
let activeReviewContext: PrContext | null = null;

const foci: { value: AiReviewFocus; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "security", label: "安全" },
  { value: "performance", label: "性能" },
  { value: "logic", label: "逻辑" },
  { value: "code_style", label: "代码风格" },
];

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

  await cancelActiveReview();
  cleanupListeners();
  loading.value = true;
  error.value = "";
  result.value = null;
  resultHeadSha.value = "";
  resultFocus.value = null;
  streamText.value = "";
  activeReviewHeadSha = props.headSha;
  activeReviewFocus = focus.value;
  activeReviewDiff = props.diff;
  activeReviewContext = props.context;

  if (useStreaming.value) {
    await startStreamingReview();
  } else {
    await startNonStreamingReview();
  }
}

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
      streamText.value += event.payload.payload;
    });

    unlistenDone = await listen<AiStreamEvent<AiReviewResult>>("ai-review-done", (event) => {
      if (event.payload.request_id !== activeRequestId) return;
      result.value = event.payload.payload;
      resultHeadSha.value = activeReviewHeadSha;
      resultFocus.value = activeReviewFocus;
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
    return;
  }

  const suggestion = result.value.suggestions[index];
  if (!drafts.value.some((draft) => draft.suggestionIndex === index)) {
    drafts.value.push({
      id: `${resultHeadSha.value}:${index}`,
      suggestionIndex: index,
      path: suggestion.file,
      startLine: suggestion.line_start,
      endLine: suggestion.line_end ?? suggestion.line_start,
      body: draftBody(index),
      headSha: resultHeadSha.value,
    });
  }
  suggestion.action = "accept";
  draftStatus.value = "";
  draftError.value = "";
}

function removeDraft(index: number) {
  const [removed] = drafts.value.splice(index, 1);
  if (removed && result.value?.suggestions[removed.suggestionIndex]) {
    result.value.suggestions[removed.suggestionIndex].action = undefined;
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
      <div class="focus-select">
        <label>聚焦:</label>
        <AppSelect v-model="focus" :options="foci" />
      </div>

      <label class="stream-toggle">
        <input type="checkbox" v-model="useStreaming" />
        流式输出
      </label>

      <button
        class="btn btn-primary"
        :disabled="(loading && !useStreaming) || !diff"
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

    <!-- Streaming live preview -->
    <div v-if="loading && useStreaming && streamText" class="stream-preview">
      <div class="stream-label">
        <span class="stream-dot" />
        实时输出中...
      </div>
      <pre class="stream-content">{{ streamText }}</pre>
    </div>

    <!-- Non-streaming loading -->
    <div v-if="loading && !useStreaming && !streamText" class="loading-state">
      <div class="spinner" />
      <p>AI 正在分析代码变更，请稍候...</p>
    </div>

    <!-- Streaming loading without content yet -->
    <div v-if="loading && useStreaming && !streamText" class="loading-state">
      <div class="spinner" />
      <p>AI 正在连接，请稍候...</p>
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
        <span v-if="resultFocus"
          >聚焦范围：{{ foci.find((item) => item.value === resultFocus)?.label }}</span
        >
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
          <textarea v-model="draft.body" class="input" rows="5" aria-label="评审草稿内容" />
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
  padding: var(--space-3);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
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

.stream-content {
  padding: var(--space-4);
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 400px;
  overflow-y: auto;
  background: var(--color-surface-hover);
  margin: 0;
  color: var(--color-text);
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
</style>
