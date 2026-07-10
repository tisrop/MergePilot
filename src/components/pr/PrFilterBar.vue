<script setup lang="ts">
import { usePrStore } from "@/stores/usePrStore";
import type { PrState } from "@/types";

const pr = usePrStore();

const states: { value: PrState; label: string }[] = [
  { value: "open", label: "Open" },
  { value: "closed", label: "Closed" },
  { value: "merged", label: "Merged" },
  { value: "all", label: "All" },
];
</script>

<template>
  <div class="pr-filter-bar">
    <div class="filters">
      <button
        v-for="s in states"
        :key="s.value"
        :class="{ active: pr.filters.state === s.value }"
        @click="pr.setFilter(s.value)"
      >
        {{ s.label }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.pr-filter-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-2) 0;
}

.filters {
  display: flex;
  gap: var(--space-1);
}

.filters button {
  padding: 6px 14px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: none;
  font-size: 13px;
  font-weight: 500;
  transition: all var(--transition-fast);
  color: var(--color-text-secondary);
}

.filters button.active {
  background: var(--color-primary);
  color: #fff;
  border-color: var(--color-primary);
}

.filters button:hover:not(.active) {
  background: var(--color-surface-hover);
  color: var(--color-text);
}
</style>
