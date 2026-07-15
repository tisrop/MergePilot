<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { storeToRefs } from "pinia";
import { copySupportInfo as copySupportInfoToClipboard, getAppVersion } from "@/api";
import { getErrorMessage } from "@/utils/error";
import { useAuthStore } from "@/stores/useAuthStore";
import { useUpdateStore } from "@/stores/useUpdateStore";
import AppLayout from "@/components/layout/AppLayout.vue";
import AiSettings from "@/components/ai/AiSettings.vue";
import type { Platform } from "@/types";

const auth = useAuthStore();
const updates = useUpdateStore();
const {
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
} = storeToRefs(updates);

const platformList: { value: Platform; label: string }[] = [
  { value: "github", label: "GitHub" },
  { value: "gitlab", label: "GitLab" },
  { value: "gitee", label: "Gitee" },
];

const isCopyingSupportInfo = ref(false);
const supportInfoStatus = ref("");
const isSupportInfoError = ref(false);
const appVersion = ref("");
const versionError = ref("");
const isConfirmingInstall = ref(false);

const updateProgressPercent = computed(() => {
  if (!updateTotal.value || updateTotal.value <= 0) return null;
  return Math.min(100, Math.round((updateDownloaded.value / updateTotal.value) * 100));
});

const isPortableUpdate = computed(() => updateResult.value?.update_mode === "portable");
const updateActionLabel = computed(() => {
  if (isInstallingUpdate.value) return isPortableUpdate.value ? "正在打开浏览器..." : "正在更新...";
  return isPortableUpdate.value ? "下载便携版 ZIP" : "下载并安装";
});

onMounted(async () => {
  try {
    appVersion.value = await getAppVersion();
  } catch (error) {
    versionError.value = getErrorMessage(error, "无法读取当前版本");
  }
});

async function checkUpdate(isBackground = false) {
  isConfirmingInstall.value = false;
  await updates.checkUpdate(isBackground);
}

async function setAutoUpdateCheckEnabled(event: Event) {
  const enabled = (event.target as HTMLInputElement).checked;
  await updates.setAutoUpdateCheckEnabled(enabled);
}

async function installUpdate() {
  if (
    isInstallingUpdate.value ||
    isUpdateInstalled.value ||
    isRestartingUpdate.value ||
    !updateResult.value?.available
  ) {
    return;
  }
  if (!isPortableUpdate.value && !isConfirmingInstall.value) {
    isConfirmingInstall.value = true;
    return;
  }

  isConfirmingInstall.value = false;
  await updates.installUpdate();
}

function cancelInstallConfirmation() {
  isConfirmingInstall.value = false;
}

async function restartApp() {
  await updates.restartUpdate();
}

async function copySupportInfo() {
  if (isCopyingSupportInfo.value) return;

  isCopyingSupportInfo.value = true;
  supportInfoStatus.value = "";
  isSupportInfoError.value = false;
  try {
    await copySupportInfoToClipboard(auth.activePlatform);
    supportInfoStatus.value = "诊断信息已复制，可直接粘贴到 Issue 中。";
  } catch (error) {
    isSupportInfoError.value = true;
    supportInfoStatus.value = `复制失败：${getErrorMessage(error, "诊断信息暂不可用")}`;
  } finally {
    isCopyingSupportInfo.value = false;
  }
}
</script>

<template>
  <AppLayout>
    <template #header>
      <div class="settings-header">
        <h2>设置</h2>
        <p>管理代码平台显示方式与 AI 评审服务</p>
      </div>
    </template>

    <div class="settings-page">
      <section class="section">
        <div class="section-heading">
          <span class="section-icon" aria-hidden="true">
            <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor">
              <rect x="3" y="4" width="18" height="16" rx="2" />
              <path d="M9 4v16" />
            </svg>
          </span>
          <div>
            <h3>界面设置</h3>
            <p>选择需要在侧边栏中显示的代码托管平台。</p>
          </div>
        </div>
        <div v-for="p in platformList" :key="p.value" class="setting-row">
          <span>
            <span class="setting-label">{{ p.label }}</span>
            <span class="setting-hint">在平台切换器中显示</span>
          </span>
          <label class="toggle">
            <input
              type="checkbox"
              :aria-label="`显示 ${p.label}`"
              :checked="auth.platformVisibility[p.value]"
              :disabled="
                auth.platformVisibility[p.value] &&
                Object.values(auth.platformVisibility).filter(Boolean).length <= 1
              "
              @change="
                auth.setPlatformVisibility(p.value, ($event.target as HTMLInputElement).checked)
              "
            />
            <span class="toggle-slider" />
          </label>
        </div>
      </section>

      <section class="section">
        <div class="section-heading">
          <span class="section-icon ai" aria-hidden="true">
            <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor">
              <path d="M12 3v3M12 18v3M3 12h3M18 12h3" />
              <circle cx="12" cy="12" r="5" />
              <path d="m15.5 8.5 2-2M6.5 17.5l2-2M8.5 8.5l-2-2M17.5 17.5l-2-2" />
            </svg>
          </span>
          <div>
            <h3>AI 评审设置</h3>
            <p>配置兼容 OpenAI 协议的模型服务与访问凭据。</p>
          </div>
        </div>
        <AiSettings />
      </section>

      <section class="section">
        <div class="section-heading update-heading">
          <span class="section-icon update" aria-hidden="true">
            <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor">
              <path d="M12 3v12" />
              <path d="m7 10 5 5 5-5" />
              <path d="M5 21h14" />
            </svg>
          </span>
          <div>
            <h3>应用更新</h3>
            <p>当前版本：{{ appVersion ? `v${appVersion}` : "读取中..." }}</p>
          </div>
          <button
            type="button"
            class="check-update-button"
            :disabled="
              isCheckingUpdate || isInstallingUpdate || isUpdateInstalled || isRestartingUpdate
            "
            @click="checkUpdate()"
          >
            {{ isCheckingUpdate ? "正在检查..." : "检查更新" }}
          </button>
        </div>
        <div class="auto-update-row">
          <span>
            <span class="setting-label">每日自动检查</span>
            <span class="setting-hint">启动时最多每天检查一次；失败不会打扰其他操作。</span>
          </span>
          <label class="toggle">
            <input
              type="checkbox"
              aria-label="每日自动检查更新"
              :checked="isAutoUpdateCheckEnabled"
              @change="setAutoUpdateCheckEnabled"
            />
            <span class="toggle-slider" />
          </label>
        </div>
        <p v-if="versionError" class="support-status error" role="status">{{ versionError }}</p>
        <p v-if="updateError" class="support-status error" role="status" aria-live="polite">
          {{ updateError }}
        </p>
        <div v-if="updateResult?.available" class="update-result" role="status">
          <strong>发现新版本 v{{ updateResult.version }}</strong>
          <p v-if="isPortableUpdate">
            当前为 Windows 便携版。将在浏览器中下载 ZIP；请退出应用，解压后用新版 MergeBeacon.exe
            覆盖旧文件，再重新启动。覆盖前建议备份旧文件。
          </p>
          <p v-else-if="!isUpdateInstalled">下载完成后将安装更新，并由你确认何时重启应用。</p>
          <p v-else>更新已安装，重启应用后生效。</p>
          <pre v-if="updateResult.notes" class="update-notes">{{ updateResult.notes }}</pre>
          <div v-if="isInstallingUpdate" class="update-progress" aria-live="polite">
            <progress
              v-if="updateProgressPercent !== null"
              :value="updateProgressPercent"
              max="100"
            />
            <span v-if="updatePhase === 'installing'"> 正在安装更新... </span>
            <span v-else-if="updateProgressPercent !== null">
              正在下载... {{ updateProgressPercent }}%
            </span>
            <span v-else>正在下载更新...</span>
          </div>
          <div class="update-actions">
            <template v-if="isUpdateInstalled">
              <button
                type="button"
                class="install-update-button"
                :aria-busy="isRestartingUpdate"
                :disabled="isRestartingUpdate"
                @click="restartApp"
              >
                {{ isRestartingUpdate ? "正在重启..." : "重启完成更新" }}
              </button>
            </template>
            <template v-else-if="isConfirmingInstall">
              <span class="install-warning"> 安装前请保存工作并结束正在进行的 AI 评审。 </span>
              <button
                type="button"
                class="install-update-button"
                :disabled="isInstallingUpdate"
                @click="installUpdate"
              >
                确认安装
              </button>
              <button
                type="button"
                class="cancel-install-button"
                @click="cancelInstallConfirmation"
              >
                取消
              </button>
            </template>
            <button
              v-else
              type="button"
              class="install-update-button"
              :disabled="isInstallingUpdate"
              @click="installUpdate"
            >
              {{ updateActionLabel }}
            </button>
          </div>
        </div>
        <p v-else-if="updateResult" class="support-status" role="status" aria-live="polite">
          当前已是最新版本。
        </p>
        <p v-else class="privacy-note">仅从 MergeBeacon 官方签名更新源读取元数据。</p>
      </section>

      <section class="section">
        <div class="section-heading support-heading">
          <span class="section-icon support" aria-hidden="true">
            <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor">
              <circle cx="12" cy="12" r="9" />
              <path d="M12 11v5M12 8h.01" />
            </svg>
          </span>
          <div>
            <h3>诊断信息</h3>
            <p>复制经过脱敏的版本、系统和配置状态，用于反馈问题。</p>
          </div>
          <button
            type="button"
            class="copy-support-button"
            :disabled="isCopyingSupportInfo"
            @click="copySupportInfo"
          >
            {{ isCopyingSupportInfo ? "正在复制..." : "复制诊断信息" }}
          </button>
        </div>
        <p class="privacy-note">不包含 Token、API Key、仓库信息、代码内容或完整的自托管地址。</p>
        <p
          v-if="supportInfoStatus"
          class="support-status"
          :class="{ error: isSupportInfoError }"
          role="status"
          aria-live="polite"
        >
          {{ supportInfoStatus }}
        </p>
      </section>
    </div>
  </AppLayout>
</template>

<style scoped>
.settings-page {
  max-width: 720px;
}

.section {
  margin-bottom: var(--space-6);
  padding: var(--space-5);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-surface);
  box-shadow: var(--shadow-sm);
}

.settings-header h2 {
  font-size: 20px;
  letter-spacing: -0.02em;
}

.settings-header p {
  margin-top: 2px;
  color: var(--color-text-secondary);
  font-size: 12px;
}

.section-heading {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  margin-bottom: var(--space-3);
  padding-bottom: var(--space-4);
  border-bottom: 1px solid var(--color-border);
}

.section-heading h3 {
  font-size: 15px;
  font-weight: 600;
}

.section-heading p,
.setting-hint {
  display: block;
  margin-top: 2px;
  color: var(--color-text-tertiary);
  font-size: 11px;
}

.section-icon {
  display: inline-flex;
  width: 34px;
  height: 34px;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
  color: var(--color-primary);
  background: var(--color-primary-light);
}

.section-icon.ai {
  color: var(--color-success);
  background: var(--color-success-light);
}

.section-icon svg {
  stroke-width: 1.7;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 54px;
  padding: var(--space-2) 0;
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

.toggle input:focus-visible + .toggle-slider {
  outline: 3px solid rgba(57, 120, 189, 0.28);
  outline-offset: 2px;
}

.toggle input:disabled + .toggle-slider {
  opacity: 0.5;
  cursor: not-allowed;
}

.toggle input:disabled ~ .toggle-slider {
  cursor: not-allowed;
}
.support-heading,
.update-heading {
  align-items: center;
}

.section-icon.support {
  color: var(--color-text-secondary);
  background: var(--color-surface-hover);
}

.copy-support-button,
.check-update-button {
  min-height: 36px;
  margin-left: auto;
  padding: 0 var(--space-4);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  color: var(--color-text);
  background: var(--color-surface);
  font: inherit;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition:
    border-color var(--transition-fast),
    background var(--transition-fast);
}

.copy-support-button:hover:not(:disabled),
.check-update-button:hover:not(:disabled) {
  border-color: var(--color-primary);
  background: var(--color-primary-light);
}

.copy-support-button:disabled,
.check-update-button:disabled {
  opacity: 0.6;
  cursor: wait;
}

.auto-update-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  margin-bottom: var(--space-3);
}

.update-result {
  margin-top: var(--space-3);
  color: var(--color-text);
  font-size: 13px;
  line-height: 1.6;
}

.update-result strong {
  color: var(--color-success);
}

.update-notes {
  max-height: 180px;
  margin-top: var(--space-2);
  padding: var(--space-3);
  overflow: auto;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface-hover);
  color: var(--color-text-secondary);
  font: inherit;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.update-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-top: var(--space-3);
  flex-wrap: wrap;
}

.install-warning {
  width: 100%;
  color: var(--color-warning, #a16207);
  font-size: 12px;
}

.install-update-button,
.cancel-install-button {
  min-height: 36px;
  padding: 0 var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  font: inherit;
  cursor: pointer;
}

.install-update-button {
  color: white;
  border-color: var(--color-primary);
  background: var(--color-primary);
}

.install-update-button:disabled {
  opacity: 0.6;
  cursor: wait;
}

.cancel-install-button {
  color: var(--color-text);
  background: var(--color-surface);
}

.update-progress {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-top: var(--space-3);
  color: var(--color-text-secondary);
  font-size: 12px;
}

.update-progress progress {
  width: 180px;
}

.privacy-note,
.support-status {
  color: var(--color-text-tertiary);
  font-size: 12px;
  line-height: 1.6;
}

.support-status {
  margin-top: var(--space-2);
  color: var(--color-success);
}

.support-status.error {
  color: var(--color-danger);
}
</style>
