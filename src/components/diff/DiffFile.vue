<script setup lang="ts">
defineProps<{
  newName: string;
  additions: number;
  deletions: number;
}>();
</script>

<template>
  <div class="diff-file">
    <div class="file-header">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
      <span class="file-name">{{ newName }}</span>
    </div>
    <div class="file-stats">
      <span class="stat-add">+{{ additions }}</span>
      <span class="stat-del">-{{ deletions }}</span>
    </div>
  </div>
</template>

<style scoped>
.diff-file {
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  margin-bottom: var(--space-3);
  overflow: hidden;
}

.file-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: var(--color-surface-hover);
  border-bottom: 1px solid var(--color-border);
}

.file-name {
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 500;
}

.file-stats {
  display: flex;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  font-size: 12px;
  font-family: var(--font-mono);
}

.stat-add { color: var(--color-success); }
.stat-del { color: var(--color-danger); }
</style>
