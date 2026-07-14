import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import {
  checkForUpdates,
  downloadAndInstallUpdate,
  downloadAndReplacePortableUpdate,
  listenToUpdateProgress,
  restartAfterUpdate,
} from "@/api";
import type { UpdateCheckResult } from "@/types";
import { getErrorMessage } from "@/utils/error";

const AUTO_UPDATE_CHECK_KEY = "mergepilot:auto-update-check";
const LAST_UPDATE_CHECK_KEY = "mergepilot:last-update-check";
const UPDATE_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

function readUpdateStorage(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeUpdateStorage(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Storage may be unavailable in hardened webviews; update checks must remain usable.
  }
}

export const useUpdateStore = defineStore("update", () => {
  const isAutoUpdateCheckEnabled = ref(readUpdateStorage(AUTO_UPDATE_CHECK_KEY) !== "false");
  const isCheckingUpdate = ref(false);
  const updateResult = ref<UpdateCheckResult | null>(null);
  const updateError = ref("");
  const isInstallingUpdate = ref(false);
  const isRestartingUpdate = ref(false);
  const isUpdateInstalled = ref(false);
  const updateDownloaded = ref(0);
  const updateTotal = ref<number | null>(null);
  const updatePhase = ref<"downloading" | "installing" | null>(null);
  let unlistenUpdateProgress: (() => void) | null = null;
  let activeUpdateRequestId: string | null = null;
  let isDisposed = false;

  function clearUpdateProgressListener() {
    const unlisten = unlistenUpdateProgress;
    unlistenUpdateProgress = null;
    unlisten?.();
  }

  function isBackgroundCheckDue(now = Date.now()) {
    const lastCheck = Number(readUpdateStorage(LAST_UPDATE_CHECK_KEY));
    return (
      !Number.isFinite(lastCheck) ||
      lastCheck <= 0 ||
      lastCheck > now ||
      now - lastCheck >= UPDATE_CHECK_INTERVAL_MS
    );
  }

  async function checkUpdate(isBackground = false) {
    if (
      isCheckingUpdate.value ||
      isInstallingUpdate.value ||
      isUpdateInstalled.value ||
      isRestartingUpdate.value
    ) {
      return;
    }
    if (!isBackground) {
      writeUpdateStorage(LAST_UPDATE_CHECK_KEY, String(Date.now()));
      updateError.value = "";
      updateResult.value = null;
    }

    isCheckingUpdate.value = true;
    try {
      updateResult.value = await checkForUpdates();
    } catch (error) {
      if (!isBackground) {
        updateError.value = getErrorMessage(error, "检查更新失败，请稍后重试");
      }
    } finally {
      isCheckingUpdate.value = false;
    }
  }

  async function maybeCheckForUpdatesInBackground() {
    if (!isAutoUpdateCheckEnabled.value || !isBackgroundCheckDue()) return;
    writeUpdateStorage(LAST_UPDATE_CHECK_KEY, String(Date.now()));
    await checkUpdate(true);
  }

  async function setAutoUpdateCheckEnabled(enabled: boolean) {
    isAutoUpdateCheckEnabled.value = enabled;
    writeUpdateStorage(AUTO_UPDATE_CHECK_KEY, String(enabled));
    if (enabled) {
      await maybeCheckForUpdatesInBackground();
    }
  }

  async function installUpdate() {
    const expectedVersion = updateResult.value?.version;
    if (
      isInstallingUpdate.value ||
      isUpdateInstalled.value ||
      isRestartingUpdate.value ||
      !updateResult.value?.available ||
      !expectedVersion
    ) {
      return;
    }

    const requestId = crypto.randomUUID();
    const isPortable = updateResult.value.update_mode === "portable";
    activeUpdateRequestId = requestId;
    isInstallingUpdate.value = true;
    updateError.value = "";
    updateDownloaded.value = 0;
    updateTotal.value = null;
    updatePhase.value = "downloading";

    try {
      clearUpdateProgressListener();
      const unlisten = await listenToUpdateProgress((progress) => {
        if (progress.request_id !== activeUpdateRequestId) return;
        updatePhase.value = progress.phase;
        if (progress.phase === "downloading") {
          updateDownloaded.value = progress.downloaded;
          updateTotal.value = progress.total;
        }
      });
      if (isDisposed || activeUpdateRequestId !== requestId) {
        unlisten();
      } else {
        unlistenUpdateProgress = unlisten;
      }

      if (isPortable) {
        await downloadAndReplacePortableUpdate(requestId, expectedVersion);
        isRestartingUpdate.value = true;
      } else {
        await downloadAndInstallUpdate(requestId, expectedVersion);
        isUpdateInstalled.value = true;
      }
      updatePhase.value = null;
    } catch (error) {
      updateError.value = getErrorMessage(
        error,
        isPortable ? "自动更新 Windows 便携版失败，请稍后重试" : "下载安装更新失败，请稍后重试",
      );
      updatePhase.value = null;
    } finally {
      if (activeUpdateRequestId === requestId) {
        activeUpdateRequestId = null;
      }
      isInstallingUpdate.value = false;
      clearUpdateProgressListener();
    }
  }

  async function restartUpdate() {
    if (isRestartingUpdate.value || !isUpdateInstalled.value) return;

    isRestartingUpdate.value = true;
    updateError.value = "";
    try {
      await restartAfterUpdate();
    } catch (error) {
      updateError.value = getErrorMessage(error, "重启失败，请手动重新打开应用");
    } finally {
      isRestartingUpdate.value = false;
    }
  }

  onScopeDispose(() => {
    isDisposed = true;
    activeUpdateRequestId = null;
    clearUpdateProgressListener();
  });

  return {
    isAutoUpdateCheckEnabled,
    isCheckingUpdate,
    updateResult,
    updateError,
    isInstallingUpdate,
    isRestartingUpdate,
    isUpdateInstalled,
    updateDownloaded,
    updateTotal,
    updatePhase,
    checkUpdate,
    maybeCheckForUpdatesInBackground,
    setAutoUpdateCheckEnabled,
    installUpdate,
    restartUpdate,
  };
});
