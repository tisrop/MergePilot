<script setup lang="ts">
import Sidebar from "./Sidebar.vue";
</script>

<template>
  <div class="app-layout">
    <a class="skip-link" href="#main-content">跳到主要内容</a>
    <Sidebar />
    <main id="main-content" class="main-content" tabindex="-1">
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
  position: relative;
}

.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background:
    radial-gradient(circle at 88% 0%, rgba(113, 135, 255, 0.09), transparent 30%),
    radial-gradient(circle at 20% 100%, rgba(85, 224, 204, 0.05), transparent 28%), var(--color-bg);
}

.content-header {
  min-height: var(--header-height);
  padding: var(--space-4) var(--space-8);
  background: rgba(255, 255, 255, 0.94);
  border-bottom: 1px solid var(--color-border);
  box-shadow: 0 1px 0 rgba(255, 255, 255, 0.8);
  flex-shrink: 0;
}

.content-body {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-6) var(--space-8) var(--space-8);
}

.skip-link {
  position: fixed;
  top: var(--space-3);
  left: var(--space-3);
  z-index: 100;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background: var(--color-primary);
  color: #fff;
  transform: translateY(-160%);
  transition: transform var(--transition-fast);
}

.skip-link:focus {
  color: #fff;
  transform: translateY(0);
}

@media (max-width: 900px) {
  .content-header,
  .content-body {
    padding-left: var(--space-5);
    padding-right: var(--space-5);
  }
}
</style>
