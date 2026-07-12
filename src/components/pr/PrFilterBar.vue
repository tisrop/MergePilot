<script setup lang="ts">
import { usePrStore } from "@/stores/usePrStore";
import type { PrState } from "@/types";

const pr = usePrStore();

const states: { value: PrState; label: string }[] = [
  { value: "open", label: "开放" },
  { value: "closed", label: "已关闭" },
  { value: "merged", label: "已合并" },
  { value: "all", label: "全部" },
];
</script>

<template>
  <div class="pr-filter-bar">
    <div class="filters">
      <button
        v-for="s in states"
        :key="s.value"
        :class="{ active: pr.filters.state === s.value }"
        :aria-pressed="pr.filters.state === s.value"
        @click="pr.setFilter(s.value)"
      >
        {{ s.label }}
        <span v-if="s.value !== 'all'" class="count">{{ pr.stateCounts[s.value] }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.pr-filter-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: var(--space-3);
}

.filters {
  display: flex;
  gap: 2px;
  padding: 3px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-bg);
}

.filters button {
  min-height: 32px;
  padding: 5px 12px;
  border: none;
  border-radius: 6px;
  background: none;
  font-size: 13px;
  font-weight: 500;
  transition:
    background-color var(--transition-fast),
    color var(--transition-fast),
    box-shadow var(--transition-fast);
  color: var(--color-text-secondary);
}

.filters button.active {
  background: var(--color-surface);
  color: var(--color-primary);
  box-shadow: var(--shadow-sm);
}

.filters button:hover:not(.active) {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  margin-left: 5px;
  border-radius: 10px;
  font-size: 11px;
  font-weight: 600;
  line-height: 1;
  background: var(--color-surface-hover);
  color: var(--color-text-secondary);
}

.active .count {
  background: var(--color-primary-light);
  color: var(--color-primary);
}
</style>
