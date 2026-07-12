<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import type { Platform } from "@/types";
import AppSelect from "@/components/shared/AppSelect.vue";
import { authLogin } from "@/api";
import { open } from "@tauri-apps/plugin-shell";

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();

function parsePlatform(value: unknown): Platform | undefined {
  return value === "github" || value === "gitlab" || value === "gitee" ? value : undefined;
}

const platform = ref<Platform>(parsePlatform(route.query.platform) ?? "github");
const token = ref("");
const gitlabUrl = ref("");
const error = ref("");
const loading = ref(false);

watch(
  () => route.query.platform,
  (value) => {
    const requestedPlatform = parsePlatform(value);
    if (requestedPlatform) platform.value = requestedPlatform;
  },
);

const platforms: { value: Platform; label: string }[] = [
  { value: "github", label: "GitHub" },
  { value: "gitlab", label: "GitLab" },
  { value: "gitee", label: "Gitee" },
];

const needsCustomUrl = computed(() => platform.value === "gitlab" || platform.value === "gitee");
const usesInsecureHttp = computed(() => getCustomUrl()?.startsWith("http://") ?? false);

function getCustomUrl(): string | undefined {
  if (!needsCustomUrl.value) return undefined;
  const url = gitlabUrl.value.trim();
  if (!url) return undefined;
  if (!url.startsWith("http://") && !url.startsWith("https://")) {
    return `https://${url}`;
  }
  return url;
}

async function handleLogin() {
  if (!token.value.trim()) {
    error.value = "请输入 Token";
    return;
  }
  loading.value = true;
  error.value = "";
  try {
    const result = await authLogin(platform.value, token.value.trim(), getCustomUrl());
    auth.platforms[platform.value] = { user: result.user, isLoggedIn: true };
    auth.activePlatform = platform.value;
    await router.replace("/pr");
  } catch (e: any) {
    error.value = e?.toString() || "登录失败，请检查 Token 是否正确";
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="login-page">
    <div class="login-card">
      <div class="login-brand">
        <svg
          width="40"
          height="40"
          viewBox="0 0 24 24"
          fill="none"
          stroke="var(--color-primary)"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="18" cy="18" r="3" />
          <circle cx="6" cy="6" r="3" />
          <path d="M13 6h3a2 2 0 0 1 2 2v7" />
          <line x1="6" y1="6" x2="6" y2="15" />
        </svg>
        <h1>MergePilot</h1>
      </div>
      <p class="subtitle">跨平台 Code Merge 工具</p>

      <div class="form-group">
        <label>平台</label>
        <AppSelect v-model="platform" :options="platforms" />
      </div>

      <div v-if="needsCustomUrl" class="form-group">
        <label>服务器地址（可选）</label>
        <input
          v-model="gitlabUrl"
          class="input"
          type="text"
          :placeholder="
            platform === 'gitlab'
              ? 'https://gitlab.com（留空使用官方）'
              : 'https://gitee.com（留空使用官方）'
          "
        />
        <p class="hint">私有化部署请填写完整地址，如 https://gitlab.example.com</p>
        <p v-if="usesInsecureHttp" class="http-warning">
          HTTP 连接不会加密 Token，请仅用于可信内网。
        </p>
      </div>

      <div class="form-group">
        <label>Personal Access Token</label>
        <input
          v-model="token"
          class="input"
          type="password"
          placeholder="输入你的 Token..."
          @keyup.enter="handleLogin"
        />
        <p class="hint">Token 优先保存到系统凭证库；不可用时保存到本地加密文件。</p>
      </div>

      <div v-if="error" class="error-box">
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <circle cx="12" cy="12" r="10" />
          <line x1="15" y1="9" x2="9" y2="15" />
          <line x1="9" y1="9" x2="15" y2="15" />
        </svg>
        {{ error }}
      </div>

      <button class="btn btn-primary login-btn" :disabled="loading" @click="handleLogin">
        <div v-if="loading" class="btn-spinner" />
        {{ loading ? "登录中..." : "登录" }}
      </button>

      <div class="help-links">
        <span class="token-link" @click="open('https://github.com/settings/tokens')"
          >GitHub Token</span
        >
        <span
          class="token-link"
          @click="open('https://gitlab.com/-/user_settings/personal_access_tokens')"
          >GitLab Token</span
        >
        <span class="token-link" @click="open('https://gitee.com/profile/personal_access_tokens')"
          >Gitee Token</span
        >
      </div>

      <p class="skip">
        <router-link to="/settings">跳过登录，先去设置 →</router-link>
      </p>
    </div>
  </div>
</template>

<style scoped>
.login-page {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  background: var(--color-bg);
}

.login-card {
  width: 440px;
  padding: var(--space-10);
  background: var(--color-surface);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-lg);
}

.login-brand {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-1);
}

.login-brand svg {
  opacity: 0.8;
}

h1 {
  font-size: 28px;
  font-weight: 700;
  text-align: center;
  letter-spacing: -0.02em;
}

.subtitle {
  text-align: center;
  color: var(--color-text-secondary);
  margin-bottom: var(--space-6);
  font-size: 14px;
}

.form-group {
  margin-bottom: var(--space-4);
}

label {
  display: block;
  font-weight: 600;
  margin-bottom: var(--space-1);
  font-size: 13px;
}

select {
  width: 100%;
}

.hint {
  font-size: 12px;
  color: var(--color-text-tertiary);
  margin-top: var(--space-1);
}

.error-box {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2);
  background: var(--color-danger-light);
  color: var(--color-danger);
  border: 1px solid var(--color-danger-border);
  border-radius: var(--radius-md);
  margin-bottom: var(--space-4);
  font-size: 13px;
}

.login-btn {
  width: 100%;
  padding: var(--space-3);
  font-size: 15px;
  font-weight: 600;
}

.btn-spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.help-links {
  display: flex;
  justify-content: space-between;
  margin-top: var(--space-4);
  font-size: 12px;
}

.skip {
  text-align: center;
  margin-top: var(--space-4);
  font-size: 13px;
}

.token-link {
  color: var(--color-primary);
  cursor: pointer;
  font-weight: 500;
}

.token-link:hover {
  text-decoration: underline;
}

.skip a {
  color: var(--color-text-secondary);
}

.skip a:hover {
  color: var(--color-primary);
}
.http-warning {
  margin-top: var(--space-2);
  color: var(--color-warning);
  font-size: 12px;
}
</style>
