<script setup lang="ts">
import { ref, onUnmounted } from "vue";
import type {
  Platform,
  AiReviewFocus,
  AiReviewResult,
  PrContext,
  AiSuggestionAction,
  AiStreamEvent,
} from "@/types";
import { aiReview, aiReviewCancel, aiReviewStream } from "@/api";
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
}>();

const focus = ref<AiReviewFocus>("all");
const useStreaming = ref(true);
const loading = ref(false);
const error = ref("");
const result = ref<AiReviewResult | null>(null);

const streamText = ref("");
let unlistenChunk: UnlistenFn | null = null;
let unlistenDone: UnlistenFn | null = null;
let unlistenError: UnlistenFn | null = null;
let activeRequestId: string | null = null;

const foci: { value: AiReviewFocus; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "security", label: "安全" },
  { value: "performance", label: "性能" },
  { value: "logic", label: "逻辑" },
  { value: "code_style", label: "代码风格" },
];

async function startReview() {
  if (!props.diff) {
    error.value = "没有 diff 数据";
    return;
  }

  await cancelActiveReview();
  cleanupListeners();
  loading.value = true;
  error.value = "";
  result.value = null;
  streamText.value = "";

  if (useStreaming.value) {
    await startStreamingReview();
  } else {
    await startNonStreamingReview();
  }
}

async function startNonStreamingReview() {
  try {
    result.value = await aiReview({
      diff: props.diff,
      context: props.context,
      file_filter: null,
      focus: focus.value,
    });
  } catch (e: any) {
    error.value = e?.toString() || "AI 评审失败";
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
      diff: props.diff,
      context: props.context,
      file_filter: null,
      focus: focus.value,
    });
  } catch (e: any) {
    if (activeRequestId === requestId) {
      activeRequestId = null;
      error.value = e?.toString() || "AI 流式评审启动失败";
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

function onAction(index: number, action: AiSuggestionAction) {
  if (!result.value) return;
  result.value.suggestions[index].action = action;
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
          @action="(a: AiSuggestionAction) => onAction(idx, a)"
        />
      </div>

      <div v-else class="no-issues">
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
