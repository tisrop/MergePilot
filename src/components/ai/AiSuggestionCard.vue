<script setup lang="ts">
import type { AiSuggestion, AiSuggestionAction, Severity } from "@/types";

defineProps<{
  suggestion: AiSuggestion;
}>();

const emit = defineEmits<{
  action: [action: AiSuggestionAction];
}>();

const severityIcon: Record<Severity, string> = {
  critical: "#dc2626",
  major: "#f59e0b",
  minor: "#3b82f6",
  info: "#06b6d4",
};

const severityLabel: Record<Severity, string> = {
  critical: "Critical",
  major: "Major",
  minor: "Minor",
  info: "Info",
};
</script>

<template>
  <div class="suggestion-card" :class="`severity-${suggestion.severity}`">
    <div class="card-header">
      <span class="severity font-mono" :style="{ color: severityIcon[suggestion.severity] }">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="12" r="10"/></svg>
        {{ severityLabel[suggestion.severity] }}
      </span>
      <span class="category">{{ suggestion.category }}</span>
      <span class="file-loc">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
        {{ suggestion.file }}
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
      <button class="btn btn-sm btn-accept" @click="emit('action', 'accept')">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
        采纳
      </button>
      <button class="btn btn-sm btn-edit" @click="emit('action', { edit: '' })">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
        编辑
      </button>
      <button class="btn btn-sm btn-reject" @click="emit('action', 'reject')">忽略</button>
    </div>

    <div v-else class="action-status">
      <span v-if="suggestion.action === 'accept'" class="accepted">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
        已采纳
      </span>
      <span v-else-if="suggestion.action === 'reject'" class="rejected">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="4 4 20 20"/></svg>
        已忽略
      </span>
    </div>
  </div>
</template>

<style scoped>
.suggestion-card {
  padding: var(--space-4) var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-left: 4px solid var(--color-border);
  border-radius: 0 var(--radius-lg) var(--radius-lg) 0;
  margin-bottom: var(--space-2);
  transition: box-shadow var(--transition-base);
}

.suggestion-card:hover {
  box-shadow: var(--shadow-sm);
}

.severity-critical { border-left-color: var(--severity-critical); }
.severity-major { border-left-color: var(--severity-major); }
.severity-minor { border-left-color: var(--severity-minor); }
.severity-info { border-left-color: var(--severity-info); }

.card-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
  flex-wrap: wrap;
}

.severity {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-weight: 700;
  font-size: 12px;
}

.category {
  font-size: 11px;
  padding: 1px 6px;
  background: var(--color-surface-hover);
  border-radius: var(--radius-sm);
  color: var(--color-text-secondary);
}

.file-loc {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--color-text-secondary);
  margin-left: auto;
  font-family: var(--font-mono);
}

.description {
  font-size: 13px;
  line-height: 1.6;
  margin-bottom: var(--space-2);
}

.suggestion-code {
  margin: var(--space-2) 0;
  padding: var(--space-2);
  background: var(--color-surface-hover);
  border-radius: var(--radius-md);
  overflow-x: auto;
}

.suggestion-code pre {
  font-family: var(--font-mono);
  font-size: 12px;
  white-space: pre-wrap;
  margin: 0;
}

.card-actions {
  display: flex;
  gap: var(--space-1);
  margin-top: var(--space-2);
}

.btn-accept {
  color: var(--color-success) !important;
  border-color: var(--color-success) !important;
}
.btn-accept:hover { background: var(--color-success-light) !important; }

.btn-edit {
  color: var(--color-primary) !important;
  border-color: var(--color-primary) !important;
}
.btn-edit:hover { background: var(--color-primary-light) !important; }

.btn-reject {
  color: var(--color-text-tertiary) !important;
}
.btn-reject:hover { background: var(--color-surface-hover) !important; }

.action-status {
  margin-top: var(--space-2);
  font-size: 12px;
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.accepted {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--color-success);
  font-weight: 500;
}

.rejected {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--color-text-tertiary);
}
</style>
