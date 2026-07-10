<script setup lang="ts">
import type { PrSummary } from "@/types";

defineProps<{
  pr: PrSummary;
}>();

defineEmits<{
  click: [];
}>();
</script>

<template>
  <div class="pr-card" @click="$emit('click')">
    <div class="pr-card-top">
      <span class="pr-title">{{ pr.title }}</span>
      <span class="badge" :class="`badge-${pr.state}`">{{ pr.state }}</span>
    </div>
    <div class="pr-card-meta">
      <span class="pr-number">#{{ pr.number }}</span>
      <span>{{ pr.author.login }}</span>
      <span>{{ new Date(pr.updated_at).toLocaleDateString() }}</span>
      <span v-if="pr.labels.length" class="pr-labels">
        <span
          v-for="label in pr.labels"
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
.pr-card {
  padding: var(--space-4) var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  cursor: pointer;
  transition: box-shadow var(--transition-base), border-color var(--transition-base);
}

.pr-card:hover {
  box-shadow: var(--shadow-md);
  border-color: var(--color-primary-border);
}

.pr-card-top {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
}

.pr-title {
  flex: 1;
  font-weight: 600;
  font-size: 15px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pr-card-meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  font-size: 12px;
  color: var(--color-text-secondary);
}

.pr-number {
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: 12px;
}

.pr-labels {
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
