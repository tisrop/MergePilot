<script setup lang="ts">
import Sidebar from "./Sidebar.vue";
</script>

<template>
  <div class="app-layout">
    <Sidebar />
    <main class="main-content">
      <div class="content-header" v-if="$slots.header">
        <slot name="header" />
      </div>
      <div class="content-body">
        <slot />
      </div>
    </main>
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  height: 100%;
}

.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--color-bg);
}

.content-header {
  padding: var(--space-4) var(--space-6);
  background: var(--color-surface);
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}

.content-body {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-6);
}
</style>
