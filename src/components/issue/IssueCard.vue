<script setup lang="ts">
import type { IssueSummary } from "@/types";

defineProps<{
  issue: IssueSummary;
}>();
</script>

<template>
  <div class="issue-card">
    <div class="issue-header">
      <span class="issue-number">#{{ issue.number }}</span>
      <span class="issue-title">{{ issue.title }}</span>
      <span class="issue-state" :class="issue.state">{{ issue.state }}</span>
    </div>
    <div class="issue-meta">
      <span>{{ issue.author.login }}</span>
      <span>{{ new Date(issue.created_at).toLocaleDateString() }}</span>
      <span v-if="issue.labels.length" class="issue-labels">
        <span
          v-for="label in issue.labels"
          :key="label"
          class="label-tag"
        >
          {{ label }}
        </span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.issue-card {
  padding: 14px 16px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
}

.issue-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.issue-number {
  color: var(--color-text-secondary);
  font-size: 13px;
}

.issue-title {
  flex: 1;
  font-weight: 600;
  font-size: 15px;
}

.issue-state {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 12px;
  font-weight: 600;
  text-transform: uppercase;
}

.open {
  background: #d1f5e0;
  color: #116329;
}

.closed {
  background: #ffdce0;
  color: #86181d;
}

.issue-meta {
  display: flex;
  gap: 12px;
  font-size: 12px;
  color: var(--color-text-secondary);
}

.issue-labels {
  display: flex;
  gap: 4px;
}

.label-tag {
  padding: 1px 6px;
  background: #f0f0f0;
  border-radius: 4px;
  font-size: 11px;
}
</style>
