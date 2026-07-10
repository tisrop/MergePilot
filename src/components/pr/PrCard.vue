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
    <div class="pr-card-header">
      <span class="pr-number">#{{ pr.number }}</span>
      <span class="pr-title">{{ pr.title }}</span>
      <span class="pr-state" :class="`state-${pr.state}`">{{ pr.state }}</span>
    </div>
    <div class="pr-card-meta">
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
  padding: 14px 16px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  cursor: pointer;
  transition: box-shadow 0.15s;
}

.pr-card:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
}

.pr-card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.pr-number {
  color: var(--color-text-secondary);
  font-size: 13px;
}

.pr-title {
  flex: 1;
  font-weight: 600;
  font-size: 15px;
}

.pr-state {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 12px;
  font-weight: 600;
  text-transform: uppercase;
}

.state-open {
  background: #d1f5e0;
  color: #116329;
}

.state-closed {
  background: #ffdce0;
  color: #86181d;
}

.state-merged {
  background: #e5d9fc;
  color: #4c2889;
}

.pr-card-meta {
  display: flex;
  gap: 12px;
  font-size: 12px;
  color: var(--color-text-secondary);
}

.pr-labels {
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
