<script setup lang="ts">
import { computed } from "vue";
import Sidebar from "./Sidebar.vue";
import { useNotificationStore } from "@/stores/useNotificationStore";
import type { Platform } from "@/types";

withDefaults(
  defineProps<{
    isDiffFocusMode?: boolean;
    compactSidebar?: boolean;
  }>(),
  {
    isDiffFocusMode: false,
    compactSidebar: false,
  },
);

const notifications = useNotificationStore();
const platformLabels: Record<Platform, string> = {
  github: "GitHub",
  gitlab: "GitLab",
  gitee: "Gitee",
};
const retryPlatforms = computed(() =>
  (Object.keys(platformLabels) as Platform[]).filter(
    (platform) => notifications.retryCountdown[platform] > 0,
  ),
);

function formatCountdown(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes}:${String(remainingSeconds).padStart(2, "0")}`;
}
</script>

<template>
  <div class="app-layout">
    <a class="skip-link" href="#main-content">跳到主要内容</a>
    <Sidebar :is-diff-focus-mode="isDiffFocusMode" :compact-sidebar="compactSidebar" />
    <main id="main-content" class="main-content" tabindex="-1">
      <section
        v-if="notifications.showNotificationError"
        class="notification-error-banner"
        role="alert"
        aria-live="assertive"
      >
        <div class="notification-error-copy">
          <strong>桌面通知异常</strong>
          <span>{{ notifications.notificationError }}</span>
          <span v-for="platform in retryPlatforms" :key="platform" class="retry-countdown">
            {{ platformLabels[platform] }} 将在
            {{ formatCountdown(notifications.retryCountdown[platform]) }} 后重试
          </span>
        </div>
        <RouterLink class="notification-settings-link" to="/settings">打开通知设置</RouterLink>
      </section>
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

.notification-error-banner {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-8);
  border-bottom: 1px solid var(--color-warning-border);
  background: var(--color-warning-light);
  color: var(--color-warning);
  flex-shrink: 0;
  font-size: 12px;
}

.notification-error-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
  line-height: 1.45;
}

.notification-error-copy strong {
  font-size: 13px;
}

.retry-countdown {
  font-variant-numeric: tabular-nums;
}

.notification-settings-link {
  flex: 0 0 auto;
  color: var(--color-warning);
  font-weight: 650;
  text-decoration: underline;
  text-underline-offset: 2px;
}

.notification-settings-link:hover {
  color: var(--color-warning);
  text-decoration-thickness: 2px;
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
  .content-body,
  .notification-error-banner {
    padding-left: var(--space-5);
    padding-right: var(--space-5);
  }
}
</style>
