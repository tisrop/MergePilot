<script setup lang="ts">
import PrStatusSummary from "@/components/pr/PrStatusSummary.vue";
import type { PrSummary } from "@/types";

defineProps<{
  pr: PrSummary;
}>();

defineEmits<{
  click: [];
}>();
</script>

<template>
  <button type="button" class="pr-card" @click="$emit('click')">
    <span class="pr-icon" aria-hidden="true">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor">
        <circle cx="6" cy="5" r="2.5" />
        <circle cx="18" cy="19" r="2.5" />
        <path d="M6 7.5V16a3 3 0 0 0 3 3h6.5" />
        <path d="M12 5h3a3 3 0 0 1 3 3v8.5" />
      </svg>
    </span>
    <span class="pr-content">
      <div class="pr-card-top">
        <span class="pr-title">{{ pr.title }}</span>
        <span class="badge" :class="`badge-${pr.state}`">{{ pr.state }}</span>
      </div>
      <PrStatusSummary v-if="pr.state === 'open' && pr.status" :status="pr.status" />
      <div class="pr-card-meta">
        <span class="pr-number">#{{ pr.number }}</span>
        <span>由 {{ pr.author.login }} 更新</span>
        <span>{{ new Date(pr.updated_at).toLocaleDateString("zh-CN") }}</span>
        <span v-if="pr.labels.length" class="pr-labels">
          <span v-for="label in pr.labels" :key="label" class="label-tag">
            {{ label }}
          </span>
        </span>
      </div>
    </span>
    <svg
      class="chevron"
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      aria-hidden="true"
    >
      <path d="m9 18 6-6-6-6" />
    </svg>
  </button>
</template>

<style scoped>
.pr-card {
  display: flex;
  width: 100%;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  color: var(--color-text);
  text-align: left;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition:
    box-shadow var(--transition-base),
    border-color var(--transition-base),
    transform var(--transition-base);
}

.pr-card:hover {
  box-shadow: var(--shadow-md);
  border-color: var(--color-primary-border);
  transform: translateY(-1px);
}

.pr-card:active {
  transform: translateY(0);
}

.pr-icon {
  display: inline-flex;
  width: 34px;
  height: 34px;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
  color: var(--color-success);
  background: var(--color-success-light);
  border: 1px solid var(--color-success-border);
}

.pr-icon svg,
.chevron {
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.pr-content {
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  gap: var(--space-2);
}

.pr-card-top {
  display: flex;
  align-items: center;
  gap: var(--space-2);
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
  padding: 2px 7px;
  background: var(--color-surface-hover);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-sm);
  font-size: 11px;
  color: var(--color-text-secondary);
}

.chevron {
  flex-shrink: 0;
  color: var(--color-text-tertiary);
  transition: transform var(--transition-fast);
}

.pr-card:hover .chevron {
  color: var(--color-primary);
  transform: translateX(2px);
}
</style>
