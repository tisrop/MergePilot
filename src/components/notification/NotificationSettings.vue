<script setup lang="ts">
import { onMounted, ref } from "vue";
import {
  notificationPermissionGranted,
  requestNotificationPermission,
} from "@/services/desktopNotifications";
import { useNotificationStore, type NotificationEventType } from "@/stores/useNotificationStore";
import type { Platform } from "@/types";
import { getErrorMessage } from "@/utils/error";

const notifications = useNotificationStore();
const permissionGranted = ref(false);
const requestingPermission = ref(false);
const permissionError = ref("");

const platforms: Array<{ value: Platform; label: string }> = [
  { value: "github", label: "GitHub" },
  { value: "gitlab", label: "GitLab" },
  { value: "gitee", label: "Gitee" },
];
const events: Array<{ value: NotificationEventType; label: string; hint: string }> = [
  { value: "review_request", label: "评审请求", hint: "有新的 PR/MR 需要你评审时通知" },
  { value: "checks_completed", label: "CI/测试完成", hint: "检查从进行中变为成功或失败时通知" },
  { value: "new_commits", label: "新提交", hint: "已跟踪的 PR/MR 推送新提交时通知" },
  { value: "new_comments", label: "新评论", hint: "已跟踪的 PR/MR 评论数量增加时通知" },
  { value: "mergeable", label: "可合并", hint: "PR/MR 从阻塞状态变为可合并时通知" },
];

onMounted(async () => {
  try {
    permissionGranted.value = await notificationPermissionGranted();
    if (!permissionGranted.value && notifications.preferences.enabled) {
      notifications.setEnabled(false);
      permissionError.value = "桌面通知权限已被撤销，请重新授权后再启用通知。";
      notifications.setManagerError("permission", permissionError.value);
    } else if (!permissionGranted.value && notifications.permissionError) {
      permissionError.value = notifications.permissionError;
    } else {
      notifications.clearManagerError("permission");
    }
  } catch (error) {
    const message = `检查桌面通知权限失败：${getErrorMessage(error, "系统通知服务暂不可用")}`;
    permissionError.value = message;
    notifications.setManagerError("permission", message);
  }
});

async function setEnabled(event: Event): Promise<void> {
  const enabled = (event.target as HTMLInputElement).checked;
  permissionError.value = "";
  if (!enabled) {
    notifications.setEnabled(false);
    notifications.clearManagerError("permission");
    return;
  }
  requestingPermission.value = true;
  try {
    permissionGranted.value = await requestNotificationPermission();
    if (permissionGranted.value) {
      notifications.clearManagerError("permission");
      notifications.setEnabled(true);
    } else {
      permissionError.value = "系统未授予通知权限，请在系统设置中允许 MergeBeacon 发送通知。";
      notifications.setManagerError("permission", permissionError.value);
    }
  } catch (error) {
    permissionError.value = `请求桌面通知权限失败：${getErrorMessage(
      error,
      "系统通知服务暂不可用",
    )}`;
    notifications.setManagerError("permission", permissionError.value);
  } finally {
    requestingPermission.value = false;
  }
}

function setPlatform(platform: Platform, event: Event): void {
  notifications.setPlatformEnabled(platform, (event.target as HTMLInputElement).checked);
}

function setEvent(type: NotificationEventType, event: Event): void {
  notifications.setEventEnabled(type, (event.target as HTMLInputElement).checked);
}
</script>

<template>
  <div class="notification-settings">
    <div class="setting-row primary-row">
      <span>
        <span class="setting-label">启用桌面通知</span>
        <span class="setting-hint"
          >应用运行期间每 10 分钟低频检查一次，不会在首次同步时产生通知。</span
        >
      </span>
      <label class="toggle">
        <input
          type="checkbox"
          aria-label="启用桌面通知"
          :checked="notifications.preferences.enabled && permissionGranted"
          :disabled="requestingPermission"
          @change="setEnabled"
        />
        <span class="toggle-slider" />
      </label>
    </div>

    <p v-if="permissionError" class="permission-error" role="alert">{{ permissionError }}</p>

    <fieldset>
      <legend>通知平台</legend>
      <div class="setting-grid">
        <label v-for="platform in platforms" :key="platform.value" class="choice-row">
          <span>{{ platform.label }}</span>
          <input
            type="checkbox"
            :checked="notifications.preferences.platforms[platform.value]"
            @change="setPlatform(platform.value, $event)"
          />
        </label>
      </div>
    </fieldset>

    <fieldset>
      <legend>事件类型</legend>
      <div class="event-list">
        <label v-for="event in events" :key="event.value" class="choice-row event-row">
          <span>
            <strong>{{ event.label }}</strong>
            <small>{{ event.hint }}</small>
          </span>
          <input
            type="checkbox"
            :checked="notifications.preferences.events[event.value]"
            @change="setEvent(event.value, $event)"
          />
        </label>
      </div>
    </fieldset>

    <label class="privacy-row">
      <input
        type="checkbox"
        :checked="notifications.preferences.hide_private_content"
        @change="notifications.setHidePrivateContent(($event.target as HTMLInputElement).checked)"
      />
      <span>
        <strong>隐藏私有仓库通知内容</strong>
        <small>默认不显示仓库名、PR 标题或代码信息；无法确认可见性的仓库也按私有处理。</small>
      </span>
    </label>
  </div>
</template>

<style scoped>
.notification-settings,
.event-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.setting-row,
.choice-row,
.privacy-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.primary-row {
  padding-bottom: var(--space-3);
  border-bottom: 1px solid var(--color-border-light);
}

.setting-label,
.choice-row strong,
.privacy-row strong {
  display: block;
  color: var(--color-text);
  font-size: 13px;
  font-weight: 600;
}

.setting-hint,
.choice-row small,
.privacy-row small {
  display: block;
  margin-top: 2px;
  color: var(--color-text-tertiary);
  font-size: 11px;
  line-height: 1.45;
}

fieldset {
  min-width: 0;
  padding: var(--space-3);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-md);
}

legend {
  padding: 0 var(--space-2);
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}

.setting-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-2);
}

.choice-row {
  padding: var(--space-2);
  border-radius: var(--radius-sm);
  color: var(--color-text-secondary);
  font-size: 12px;
}

.choice-row:hover,
.privacy-row:hover {
  background: var(--color-surface-hover);
}

.event-row {
  align-items: flex-start;
}

.event-row > span {
  min-width: 0;
}

.privacy-row {
  align-items: flex-start;
  justify-content: flex-start;
  padding: var(--space-3);
  border-radius: var(--radius-md);
  background: var(--color-bg);
}

.choice-row input,
.privacy-row input {
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
  accent-color: var(--color-primary);
}

.toggle {
  position: relative;
  display: inline-block;
  width: 38px;
  height: 22px;
  flex-shrink: 0;
}

.toggle input {
  width: 0;
  height: 0;
  opacity: 0;
}

.toggle-slider {
  position: absolute;
  border-radius: 22px;
  background: var(--color-border-strong);
  cursor: pointer;
  inset: 0;
  transition: background var(--transition-fast);
}

.toggle-slider::before {
  position: absolute;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #fff;
  content: "";
  left: 3px;
  top: 3px;
  transition: transform var(--transition-fast);
}

.toggle input:checked + .toggle-slider {
  background: var(--color-primary);
}

.toggle input:checked + .toggle-slider::before {
  transform: translateX(16px);
}

.toggle input:focus-visible + .toggle-slider {
  box-shadow: var(--shadow-control-focus);
}

.toggle input:disabled + .toggle-slider {
  cursor: not-allowed;
  opacity: 0.55;
}

.permission-error {
  margin: 0;
  color: var(--color-danger);
  font-size: 12px;
}

@media (max-width: 640px) {
  .setting-grid {
    grid-template-columns: 1fr;
  }
}
</style>
