<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import type { Platform } from "@/types";
import AppSelect from "@/components/shared/AppSelect.vue";
import BrandMark from "@/components/shared/BrandMark.vue";
import { authLogin } from "@/api";
import { getErrorMessage } from "@/utils/error";
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
  } catch (e) {
    error.value = getErrorMessage(e, "登录失败，请检查 Token 是否正确");
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="login-page">
    <div class="login-shell">
      <section class="login-intro" aria-label="产品介绍">
        <div class="intro-mark" aria-hidden="true">
          <BrandMark />
        </div>
        <p class="intro-kicker">REVIEW SIGNALS</p>
        <p class="intro-title">发现关键信号，放心完成每一次合并</p>
        <p>在一个工作台中管理多平台仓库、代码差异、评审意见与 AI 建议。</p>
        <ul>
          <li><span>01</span> GitHub、GitLab 与 Gitee 统一工作流</li>
          <li><span>02</span> 聚焦上下文的代码评审体验</li>
          <li><span>03</span> Token 安全保存，敏感信息不出本机</li>
        </ul>
      </section>

      <main class="login-card">
        <div class="login-brand">
          <span class="login-brand-mark" aria-hidden="true">
            <BrandMark />
          </span>
          <h1>MergeBeacon</h1>
        </div>
        <p class="subtitle">连接代码托管平台，开始评审与 Issue 管理</p>

        <div class="form-group">
          <label>平台</label>
          <AppSelect v-model="platform" :options="platforms" />
        </div>

        <div v-if="needsCustomUrl" class="form-group">
          <label for="server-url">服务器地址（可选）</label>
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
          <label for="access-token">Personal Access Token</label>
          <input
            v-model="token"
            class="input"
            type="password"
            placeholder="输入你的 Token..."
            @keyup.enter="handleLogin"
          />
          <p class="hint">Token 优先保存到系统凭证库；不可用时保存到本地加密文件。</p>
        </div>

        <div v-if="error" class="error-box" role="alert">
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

        <div class="help-links" aria-label="Token 获取链接">
          <button
            type="button"
            class="token-link"
            @click="open('https://github.com/settings/tokens')"
          >
            GitHub Token
          </button>
          <button
            type="button"
            class="token-link"
            @click="open('https://gitlab.com/-/user_settings/personal_access_tokens')"
          >
            GitLab Token
          </button>
          <button
            type="button"
            class="token-link"
            @click="open('https://gitee.com/profile/personal_access_tokens')"
          >
            Gitee Token
          </button>
        </div>

        <p class="skip">
          <router-link to="/settings">跳过登录，先去设置 →</router-link>
        </p>
      </main>
    </div>
  </div>
</template>

<style scoped>
.login-page {
  display: flex;
  min-height: 100%;
  align-items: center;
  justify-content: center;
  padding: var(--space-8);
  background:
    radial-gradient(circle at 15% 15%, rgba(113, 135, 255, 0.15), transparent 32%),
    radial-gradient(circle at 85% 90%, rgba(85, 224, 204, 0.12), transparent 28%), var(--color-bg);
}

.login-shell {
  display: grid;
  width: min(920px, 100%);
  min-height: 570px;
  grid-template-columns: minmax(0, 1fr) minmax(400px, 0.92fr);
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.7);
  border-radius: 20px;
  background: var(--color-surface);
  box-shadow: var(--shadow-xl);
}

.login-intro {
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: var(--space-12);
  overflow: hidden;
  color: #fff;
  background:
    linear-gradient(145deg, rgba(255, 255, 255, 0.07), transparent 40%), var(--gradient-beacon);
}

.login-intro::after {
  content: "";
  position: absolute;
  right: -90px;
  bottom: -110px;
  width: 280px;
  height: 280px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 50%;
  box-shadow: 0 0 0 40px rgba(255, 255, 255, 0.035);
}

.intro-mark {
  display: inline-flex;
  width: 50px;
  height: 50px;
  align-items: center;
  justify-content: center;
  margin-bottom: var(--space-8);
  border: 1px solid rgba(85, 224, 204, 0.42);
  border-radius: var(--radius-lg);
  color: var(--color-brand-accent);
  background: linear-gradient(145deg, rgba(85, 224, 204, 0.16), rgba(113, 135, 255, 0.1));
  box-shadow: 0 8px 22px rgba(7, 24, 43, 0.16);
}

.intro-mark svg {
  width: 28px;
  height: 28px;
}

.intro-kicker {
  margin-bottom: var(--space-3);
  color: rgba(255, 255, 255, 0.65);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.16em;
}

.intro-title {
  max-width: 360px;
  margin-bottom: var(--space-4);
  font-size: 28px;
  line-height: 1.3;
  letter-spacing: -0.035em;
}

.login-intro > p:not(.intro-kicker) {
  max-width: 380px;
  color: rgba(255, 255, 255, 0.72);
  line-height: 1.7;
}

.login-intro ul {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin-top: var(--space-8);
  list-style: none;
  color: rgba(255, 255, 255, 0.82);
  font-size: 12px;
}

.login-intro li {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.login-intro li span {
  color: var(--color-brand-accent);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
}

.login-card {
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: var(--space-10);
  background: var(--color-surface);
}

.login-brand {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin-bottom: var(--space-2);
}

.login-brand-mark {
  display: inline-flex;
  width: 38px;
  height: 38px;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border-radius: 10px;
  color: var(--color-brand-accent);
  background: var(--gradient-beacon);
  box-shadow: 0 6px 16px rgba(20, 43, 73, 0.18);
}

.login-brand-mark svg {
  width: 24px;
  height: 24px;
}

h1 {
  font-size: 26px;
  font-weight: 700;
  letter-spacing: -0.035em;
}

.subtitle {
  color: var(--color-text-secondary);
  margin-bottom: var(--space-6);
  font-size: 13px;
}

.form-group {
  margin-bottom: var(--space-4);
}

label {
  display: block;
  font-weight: 600;
  margin-bottom: 6px;
  font-size: 12px;
}

.hint {
  font-size: 11px;
  line-height: 1.5;
  color: var(--color-text-tertiary);
  margin-top: 6px;
}

.error-box {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: var(--color-danger-light);
  color: var(--color-danger);
  border: 1px solid var(--color-danger-border);
  border-radius: var(--radius-md);
  margin-bottom: var(--space-4);
  font-size: 12px;
}

.login-btn {
  width: 100%;
  min-height: 42px;
  font-size: 14px;
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
  justify-content: center;
  gap: var(--space-3);
  margin-top: var(--space-4);
}

.token-link {
  padding: 0;
  border: none;
  background: none;
  color: var(--color-primary);
  cursor: pointer;
  font-size: 11px;
  font-weight: 600;
}

.token-link:hover {
  text-decoration: underline;
}

.skip {
  text-align: center;
  margin-top: var(--space-4);
  font-size: 12px;
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
  font-size: 11px;
}

@media (max-width: 760px) {
  .login-page {
    align-items: flex-start;
    padding: var(--space-4);
    overflow-y: auto;
  }

  .login-shell {
    min-height: 0;
    grid-template-columns: 1fr;
  }

  .login-intro {
    padding: var(--space-8);
  }

  .login-intro ul {
    display: none;
  }

  .login-card {
    padding: var(--space-8);
  }
}
</style>
