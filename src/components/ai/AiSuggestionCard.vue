<script setup lang="ts">
import type { AiSuggestion, AiSuggestionAction, Severity } from "@/types";

defineProps<{
  suggestion: AiSuggestion;
}>();

const emit = defineEmits<{
  action: [action: AiSuggestionAction];
}>();

const severityLabel: Record<Severity, string> = {
  critical: "🔴 Critical",
  major: "🟠 Major",
  minor: "🟡 Minor",
  info: "🔵 Info",
};
</script>

<template>
  <div class="suggestion-card" :class="`severity-${suggestion.severity}`">
    <div class="card-header">
      <span class="severity">{{ severityLabel[suggestion.severity] }}</span>
      <span class="category">{{ suggestion.category }}</span>
      <span class="file-loc">
        📄 {{ suggestion.file }}
        <template v-if="suggestion.line_start">
          :{{ suggestion.line_start }}
          <template v-if="suggestion.line_end && suggestion.line_end !== suggestion.line_start">
            -{{ suggestion.line_end }}
          </template>
        </template>
      </span>
    </div>

    <p class="description">{{ suggestion.description }}</p>

    <div v-if="suggestion.suggestion" class="suggestion-code">
      <pre>{{ suggestion.suggestion }}</pre>
    </div>

    <div class="card-actions" v-if="!suggestion.action">
      <button class="btn-accept" @click="emit('action', 'accept')">采纳</button>
      <button class="btn-edit" @click="emit('action', { edit: '' })">编辑</button>
      <button class="btn-reject" @click="emit('action', 'reject')">忽略</button>
    </div>

    <div v-else class="action-status">
      <span v-if="suggestion.action === 'accept'" class="accepted">✅ 已采纳</span>
      <span v-else-if="suggestion.action === 'reject'" class="rejected">↩ 已忽略</span>
    </div>
  </div>
</template>

<style scoped>
.suggestion-card {
  padding: 14px 16px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-left: 4px solid var(--color-border);
  border-radius: 0 8px 8px 0;
  margin-bottom: 8px;
}

.severity-critical { border-left-color: var(--severity-critical); }
.severity-major { border-left-color: var(--severity-major); }
.severity-minor { border-left-color: var(--severity-minor); }
.severity-info { border-left-color: var(--severity-info); }

.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  flex-wrap: wrap;
}

.severity { font-weight: 700; font-size: 12px; }

.category {
  font-size: 11px;
  padding: 1px 6px;
  background: #f0f0f0;
  border-radius: 4px;
}

.file-loc {
  font-size: 12px;
  color: var(--color-text-secondary);
  margin-left: auto;
}

.description {
  font-size: 13px;
  line-height: 1.6;
  margin-bottom: 8px;
}

.suggestion-code {
  margin: 8px 0;
  padding: 10px;
  background: #f6f8fa;
  border-radius: 4px;
  overflow-x: auto;
}

.suggestion-code pre {
  font-family: "SF Mono", "Fira Code", monospace;
  font-size: 12px;
  white-space: pre-wrap;
}

.card-actions {
  display: flex;
  gap: 6px;
  margin-top: 10px;
}

.card-actions button {
  padding: 4px 14px;
  border-radius: 4px;
  font-size: 12px;
  border: 1px solid var(--color-border);
  background: none;
}

.btn-accept { color: var(--color-success); border-color: var(--color-success); }
.btn-accept:hover { background: #d1f5e0; }
.btn-edit { color: var(--color-primary); border-color: var(--color-primary); }
.btn-edit:hover { background: #e8f0fe; }
.btn-reject { color: var(--color-text-secondary); }
.btn-reject:hover { background: #f0f0f0; }

.action-status {
  margin-top: 8px;
  font-size: 12px;
}

.accepted { color: var(--color-success); }
.rejected { color: var(--color-text-secondary); }
</style>
