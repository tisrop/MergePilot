<script setup lang="ts">
import type { IssueSummary } from "@/types";

defineProps<{
  issue: IssueSummary;
}>();
</script>

<template>
  <div class="issue-card">
    <div class="issue-card-top">
      <span class="issue-title">{{ issue.title }}</span>
      <span class="badge" :class="`badge-${issue.state}`">{{ issue.state }}</span>
    </div>
    <div class="issue-meta">
      <span class="issue-number">#{{ issue.number }}</span>
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
  padding: var(--space-4) var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  transition: box-shadow var(--transition-base), border-color var(--transition-base);
}

.issue-card:hover {
  box-shadow: var(--shadow-md);
  border-color: var(--color-primary-border);
}

.issue-card-top {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
}

.issue-title {
  flex: 1;
  font-weight: 600;
  font-size: 15px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.issue-meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  font-size: 12px;
  color: var(--color-text-secondary);
}

.issue-number {
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: 12px;
}

.issue-labels {
  display: flex;
  gap: var(--space-1);
}

.label-tag {
  padding: 1px 6px;
  background: var(--color-surface-hover);
  border-radius: var(--radius-sm);
  font-size: 11px;
  color: var(--color-text-secondary);
}
</style>
