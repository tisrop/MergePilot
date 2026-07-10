<script setup lang="ts">
import { ref, computed } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import type { Platform } from "@/types";
import { authLogin } from "@/api";

const router = useRouter();
const auth = useAuthStore();
const repo = useRepoStore();

const platform = ref<Platform>("github");
const token = ref("");
const gitlabUrl = ref("");
const error = ref("");
const loading = ref(false);

const platforms: { value: Platform; label: string }[] = [
  { value: "github", label: "GitHub" },
  { value: "gitlab", label: "GitLab" },
  { value: "gitee", label: "Gitee" },
];

const needsCustomUrl = computed(() =>
  platform.value === "gitlab" || platform.value === "gitee"
);

function getCustomUrl(): string | undefined {
  if (!needsCustomUrl.value) return undefined;
  const url = gitlabUrl.value.trim();
  if (!url) return undefined;
  // Auto-prepend https:// if missing
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
    const user = await authLogin(platform.value, token.value.trim(), getCustomUrl());
    auth.platforms[platform.value] = { user, isLoggedIn: true };
    auth.activePlatform = platform.value;
    await repo.fetchRepos(platform.value);
    router.push("/pr");
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
      <h1>MergePilot</h1>
      <p class="subtitle">跨平台 Code Merge 工具</p>

      <div class="form-group">
        <label>平台</label>
        <select v-model="platform">
          <option v-for="p in platforms" :key="p.value" :value="p.value">
            {{ p.label }}
          </option>
        </select>
      </div>

      <!-- Custom URL for self-hosted instances -->
      <div v-if="needsCustomUrl" class="form-group">
        <label>服务器地址（可选）</label>
        <input
          v-model="gitlabUrl"
          type="text"
          :placeholder="platform === 'gitlab' ? 'https://gitlab.com（留空使用官方）' : 'https://gitee.com（留空使用官方）'"
        />
        <p class="hint">
          私有化部署请填写完整地址，如 https://gitlab.example.com
        </p>
      </div>

      <div class="form-group">
        <label>Personal Access Token</label>
        <input
          v-model="token"
          type="password"
          placeholder="输入你的 Token..."
          @keyup.enter="handleLogin"
        />
        <p class="hint">
          你的 Token 将被安全存储，不会上传到任何第三方。
        </p>
      </div>

      <div v-if="error" class="error">{{ error }}</div>

      <button :disabled="loading" @click="handleLogin">
        {{ loading ? "登录中..." : "登录" }}
      </button>

      <div class="help-links">
        <a href="https://github.com/settings/tokens" target="_blank">GitHub Token</a>
        <a href="https://gitlab.com/-/user_settings/personal_access_tokens" target="_blank">GitLab Token</a>
        <a href="https://gitee.com/profile/personal_access_tokens" target="_blank">Gitee Token</a>
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
  padding: 40px;
  background: var(--color-surface);
  border-radius: 12px;
  box-shadow: 0 2px 16px rgba(0, 0, 0, 0.08);
}

h1 {
  font-size: 28px;
  text-align: center;
  margin-bottom: 4px;
}

.subtitle {
  text-align: center;
  color: var(--color-text-secondary);
  margin-bottom: 24px;
}

.form-group {
  margin-bottom: 16px;
}

label {
  display: block;
  font-weight: 600;
  margin-bottom: 6px;
}

select, input {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  font-size: 14px;
  outline: none;
  transition: border-color 0.2s;
}

select:focus, input:focus {
  border-color: var(--color-primary);
}

.hint {
  font-size: 12px;
  color: var(--color-text-secondary);
  margin-top: 4px;
}

.error {
  padding: 10px;
  background: #ffeaea;
  color: var(--color-danger);
  border-radius: 6px;
  margin-bottom: 16px;
  font-size: 13px;
}

button {
  width: 100%;
  padding: 12px;
  background: var(--color-primary);
  color: #fff;
  border: none;
  border-radius: 6px;
  font-size: 16px;
  font-weight: 600;
  transition: background 0.2s;
}

button:hover:not(:disabled) {
  background: var(--color-primary-hover);
}

button:disabled {
  opacity: 0.6;
}

.help-links {
  display: flex;
  justify-content: space-between;
  margin-top: 16px;
  font-size: 12px;
}

.skip {
  text-align: center;
  margin-top: 16px;
  font-size: 13px;
}
</style>
