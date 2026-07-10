<script setup lang="ts">
import AppLayout from "@/components/layout/AppLayout.vue";
import AiSettings from "@/components/ai/AiSettings.vue";
</script>

<template>
  <AppLayout>
    <template #header>
      <h2>设置</h2>
    </template>

    <div class="settings-page">
      <section class="section">
        <h3>AI 评审设置</h3>
        <AiSettings />
      </section>
    </div>
  </AppLayout>
</template>

<style scoped>
.settings-page {
  max-width: 640px;
}

.section {
  margin-bottom: var(--space-8);
}

.section h3 {
  font-size: 16px;
  margin-bottom: var(--space-4);
  padding-bottom: var(--space-2);
  border-bottom: 1px solid var(--color-border);
  font-weight: 600;
}
</style>
