<script setup lang="ts">
import { ref, onUnmounted } from "vue";
import type { Platform, AiReviewFocus, AiReviewResult, PrContext, AiSuggestionAction } from "@/types";
import { aiReview, aiReviewStream } from "@/api";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import AiSuggestionCard from "./AiSuggestionCard.vue";

const props = defineProps<{
  platform: Platform;
  owner: string;
  repo: string;
  prNumber: number;
  diff: string;
  context: PrContext | null;
}>();

const focus = ref<AiReviewFocus>("all");
const useStreaming = ref(true); // toggle for streaming vs non-streaming
const loading = ref(false);
const error = ref("");
const result = ref<AiReviewResult | null>(null);

// Streaming state
const streamText = ref(""); // accumulated raw streaming text
let unlistenChunk: UnlistenFn | null = null;
let unlistenDone: UnlistenFn | null = null;
let unlistenError: UnlistenFn | null = null;

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
  // Listen for streaming events
  try {
    // Listen for chunk events (each token)
    unlistenChunk = await listen<string>("ai-review-chunk", (event) => {
      streamText.value += event.payload;
    });

    // Listen for done event (final result)
    unlistenDone = await listen<AiReviewResult>("ai-review-done", (event) => {
      result.value = event.payload;
      loading.value = false;
      cleanupListeners();
    });

    // Listen for error event
    unlistenError = await listen<string>("ai-review-error", (event) => {
      error.value = event.payload;
      loading.value = false;
      cleanupListeners();
    });

    // Start the streaming review (returns immediately)
    await aiReviewStream({
      diff: props.diff,
      context: props.context,
      file_filter: null,
      focus: focus.value,
    });
  } catch (e: any) {
    error.value = e?.toString() || "AI 流式评审启动失败";
    loading.value = false;
    cleanupListeners();
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
  cleanupListeners();
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
        <select v-model="focus">
          <option v-for="f in foci" :key="f.value" :value="f.value">
            {{ f.label }}
          </option>
        </select>
      </div>

      <label class="stream-toggle">
        <input type="checkbox" v-model="useStreaming" />
        流式输出
      </label>

      <button class="start-btn" :disabled="loading || !diff" @click="startReview">
        {{ loading ? "评审中..." : "▶ 开始 AI 评审" }}
      </button>
    </div>

    <!-- Streaming live preview -->
    <div v-if="loading && useStreaming && streamText" class="stream-preview">
      <div class="stream-label">🔄 实时输出中...</div>
      <pre class="stream-content">{{ streamText }}</pre>
    </div>

    <!-- Non-streaming loading -->
    <div v-if="loading && !useStreaming && !streamText" class="loading">
      <p>AI 正在分析代码变更，请稍候...</p>
    </div>

    <!-- Streaming loading without content yet -->
    <div v-if="loading && useStreaming && !streamText" class="loading">
      <p>AI 正在连接，请稍候...</p>
    </div>

    <div v-if="error" class="error">{{ error }}</div>

    <div v-if="result" class="ai-result">
      <!-- Summary -->
      <div class="summary-card">
        <h4>📊 AI 评审总览</h4>
        <p>{{ result.summary }}</p>
      </div>

      <!-- Suggestions -->
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
        <p>✅ AI 未发现明显问题</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ai-panel {
  padding: 4px 0;
}

.ai-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  padding: 12px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
}

.focus-select {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}

.focus-select select {
  padding: 6px 10px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  font-size: 13px;
}

.stream-toggle {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--color-text-secondary);
  cursor: pointer;
}

.start-btn {
  margin-left: auto;
  padding: 8px 20px;
  background: var(--color-primary);
  color: #fff;
  border: none;
  border-radius: 6px;
  font-size: 14px;
}

.start-btn:disabled {
  opacity: 0.5;
}

.loading {
  text-align: center;
  padding: 40px;
  color: var(--color-text-secondary);
}

/* Streaming live preview */
.stream-preview {
  margin-bottom: 16px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  overflow: hidden;
}

.stream-label {
  padding: 8px 12px;
  background: #f6f8fa;
  font-size: 12px;
  color: var(--color-text-secondary);
  border-bottom: 1px solid var(--color-border);
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.stream-content {
  padding: 16px;
  font-family: "SF Mono", "Fira Code", monospace;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 400px;
  overflow-y: auto;
  background: #fafbfc;
}

.error {
  padding: 12px;
  background: #ffeaea;
  color: var(--color-danger);
  border-radius: 6px;
  margin-bottom: 16px;
}

.ai-result {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.summary-card {
  padding: 16px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
}

.summary-card h4 {
  margin-bottom: 8px;
}

.summary-card p {
  font-size: 14px;
  line-height: 1.6;
}

.suggestions h4 {
  margin-bottom: 12px;
}

.no-issues {
  text-align: center;
  padding: 40px;
  color: var(--color-success);
  font-size: 15px;
}
</style>
