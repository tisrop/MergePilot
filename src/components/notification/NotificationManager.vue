<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { usePrStore } from "@/stores/usePrStore";
import { useRepoStore } from "@/stores/useRepoStore";
import {
  NOTIFICATION_POLL_INTERVAL_MS,
  useNotificationStore,
  type InboxNotificationEvent,
} from "@/stores/useNotificationStore";
import {
  initializeNotificationActions,
  notificationPermissionGranted,
  showInboxNotification,
  type NotificationTarget,
} from "@/services/desktopNotifications";
import type { Platform } from "@/types";
import { getErrorMessage } from "@/utils/error";

const router = useRouter();
const auth = useAuthStore();
const pr = usePrStore();
const repo = useRepoStore();
const notifications = useNotificationStore();

const availablePlatforms = computed<Platform[]>(() =>
  (["github", "gitlab", "gitee"] as Platform[]).filter(
    (platform) =>
      auth.platforms[platform].isLoggedIn &&
      auth.platformVisibility[platform] &&
      notifications.preferences.platforms[platform],
  ),
);

async function openNotificationTarget(target: NotificationTarget): Promise<void> {
  if (!auth.platforms[target.platform].isLoggedIn) return;
  auth.setActivePlatform(target.platform);
  repo.setActiveRepo(target.owner, target.repo, target.platform);
  repo.setForkContext(null, target.platform);
  pr.clearContext();
  await router.push({
    name: "pr-detail",
    params: {
      platform: target.platform,
      owner: target.owner,
      repo: target.repo,
      number: target.number,
    },
  });
}

async function pollAndNotify(): Promise<void> {
  if (!notifications.preferences.enabled || availablePlatforms.value.length === 0) {
    return;
  }
  if (navigator.onLine === false) {
    notifications.setManagerError("network", "桌面通知暂停：当前网络不可用");
    return;
  }
  notifications.clearManagerError("network");

  let permissionGranted: boolean;
  try {
    permissionGranted = await notificationPermissionGranted();
  } catch (error) {
    notifications.setManagerError(
      "permission",
      `检查桌面通知权限失败：${getErrorMessage(error, "系统通知服务暂不可用")}`,
    );
    return;
  }
  if (!permissionGranted) {
    notifications.setEnabled(false);
    notifications.setManagerError("permission", "桌面通知权限不可用或已被撤销，请前往设置重新授权");
    return;
  }
  notifications.clearManagerError("permission");

  let events: InboxNotificationEvent[];
  try {
    events = await notifications.poll(availablePlatforms.value);
    notifications.clearManagerError("poll");
  } catch (error) {
    notifications.setManagerError(
      "poll",
      `桌面通知轮询失败：${getErrorMessage(error, "请稍后重试")}`,
    );
    return;
  }
  if (!notifications.preferences.enabled) return;
  let deliveryFailed = false;
  for (const event of events) {
    if (!auth.platforms[event.platform].isLoggedIn) continue;
    const repository = repo.reposCache[event.platform].find(
      (candidate) => candidate.full_name === event.repository_full_name,
    );
    const revealDetails =
      !notifications.preferences.hide_private_content || repository?.private === false;
    try {
      showInboxNotification(event, revealDetails);
    } catch (error) {
      deliveryFailed = true;
      notifications.setManagerError(
        "delivery",
        `桌面通知发送失败：${getErrorMessage(error, "系统通知服务暂不可用")}`,
      );
    }
  }
  if (!deliveryFailed) notifications.clearManagerError("delivery");
}

let timer: ReturnType<typeof setInterval> | null = null;
let clockTimer: ReturnType<typeof setInterval> | null = null;
let removeActionListener: (() => Promise<void>) | null = null;
let disposed = false;

onMounted(() => {
  timer = setInterval(() => void pollAndNotify(), NOTIFICATION_POLL_INTERVAL_MS);
  clockTimer = setInterval(() => notifications.updateClock(), 1000);
  window.addEventListener("online", pollAndNotify);
  void pollAndNotify();
  void initializeNotificationActions(openNotificationTarget)
    .then(async (removeListener) => {
      if (disposed) {
        await removeListener();
        return;
      }
      removeActionListener = removeListener;
      notifications.clearManagerError("actions");
    })
    .catch((error) => {
      if (disposed) return;
      notifications.setManagerError(
        "actions",
        `桌面通知点击操作初始化失败：${getErrorMessage(error, "无法监听通知操作")}`,
      );
    });
});

watch(
  [() => notifications.preferences.enabled, () => availablePlatforms.value.join(",")],
  () => void pollAndNotify(),
);

onUnmounted(() => {
  disposed = true;
  if (timer) clearInterval(timer);
  if (clockTimer) clearInterval(clockTimer);
  window.removeEventListener("online", pollAndNotify);
  if (removeActionListener) {
    void removeActionListener().catch((error) => {
      notifications.setManagerError(
        "actions",
        `桌面通知点击监听清理失败：${getErrorMessage(error, "无法停止监听通知操作")}`,
      );
    });
  }
});
</script>

<template><span class="notification-manager" aria-hidden="true" /></template>

<style scoped>
.notification-manager {
  display: none;
}
</style>
