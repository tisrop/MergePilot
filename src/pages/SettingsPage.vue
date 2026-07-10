<script setup lang="ts">
import { useAuthStore } from "@/stores/useAuthStore";
import AppLayout from "@/components/layout/AppLayout.vue";
import AiSettings from "@/components/ai/AiSettings.vue";
import type { Platform } from "@/types";

const auth = useAuthStore();

const platformList: { value: Platform; label: string }[] = [
  { value: "github", label: "GitHub" },
  { value: "gitlab", label: "GitLab" },
  { value: "gitee", label: "Gitee" },
];
</script>

<template>
  <AppLayout>
    <template #header>
      <h2>设置</h2>
    </template>

    <div class="settings-page">
      <section class="section">
        <h3>界面设置</h3>
        <div v-for="p in platformList" :key="p.value" class="setting-row">
          <span class="setting-label">{{ p.label }}</span>
          <label class="toggle">
            <input
              type="checkbox"
              :checked="auth.platformVisibility[p.value]"
              :disabled="auth.platformVisibility[p.value] && Object.values(auth.platformVisibility).filter(Boolean).length <= 1"
              @change="auth.setPlatformVisibility(p.value, ($event.target as HTMLInputElement).checked)"
            />
            <span class="toggle-slider" />
          </label>
        </div>
      </section>

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

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) 0;
}

.setting-label {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text);
}

.toggle {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 24px;
  cursor: pointer;
}

.toggle input {
  opacity: 0;
  width: 0;
  height: 0;
}

.toggle-slider {
  position: absolute;
  inset: 0;
  background: var(--color-border);
  border-radius: 24px;
  transition: background var(--transition-fast);
}

.toggle-slider::before {
  content: "";
  position: absolute;
  width: 18px;
  height: 18px;
  left: 3px;
  top: 3px;
  background: #fff;
  border-radius: 50%;
  transition: transform var(--transition-fast);
}

.toggle input:checked + .toggle-slider {
  background: var(--color-primary);
}

.toggle input:checked + .toggle-slider::before {
  transform: translateX(20px);
}

.toggle input:disabled + .toggle-slider {
  opacity: 0.5;
  cursor: not-allowed;
}

.toggle input:disabled ~ .toggle-slider {
  cursor: not-allowed;
}
</style>
