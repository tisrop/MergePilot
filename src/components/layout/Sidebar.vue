<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import type { Platform, RepoSummary } from "@/types";

const router = useRouter();
const route = useRoute();
const auth = useAuthStore();
const repo = useRepoStore();

const platforms: { value: Platform; label: string }[] = [
  { value: "github", label: "GitHub" },
  { value: "gitlab", label: "GitLab" },
  { value: "gitee", label: "Gitee" },
];

onMounted(async () => {
  // try to restore auth from keychain on startup
  for (const p of platforms) {
    await auth.checkAuth(p.value);
  }
  // Only fetch repos if not already loaded (avoids flicker on page switches)
  if (auth.isLoggedIn && auth.activePlatform && repo.repos.length === 0) {
    repo.fetchRepos(auth.activePlatform);
  }
});

function selectPlatform(p: Platform) {
  auth.setActivePlatform(p);
  repo.fetchRepos(p);
}

function selectRepo(r: { owner: string; repo: string }) {
  repo.setActiveRepo(r.owner, r.repo);
  // Force navigation even if already on /pr (adds query param to trigger watch)
  router.push({ path: "/pr", query: { _t: Date.now().toString() } });
}

function isActive(nav: string) {
  return String(route.name).startsWith(nav);
}

function getRepoOwner(fullName: string): { owner: string; repo: string } {
  const parts = fullName.split("/");
  return { owner: parts[0], repo: parts.slice(1).join("/") };
}

/** For a fork repo, return its upstream; otherwise return the repo itself */
function effectiveRepo(r: RepoSummary): { owner: string; repo: string } {
  if (r.fork && r.parent_full_name && r.parent_owner) {
    return { owner: r.parent_owner, repo: r.parent_full_name.split("/").slice(1).join("/") };
  }
  return getRepoOwner(r.full_name);
}

/** Handle click on a fork repo or its upstream entry */
function selectForkRepo(r: RepoSummary, useUpstream: boolean) {
  const target = useUpstream ? effectiveRepo(r) : getRepoOwner(r.full_name);
  selectRepo(target);
  const forkInfo = getRepoOwner(r.full_name);
  if (r.fork) {
    // Always set forkContext for fork repos, even if parent info is missing
    repo.setForkContext({
      upstreamFullName: r.parent_full_name ?? null,
      upstreamOwner: r.parent_owner ?? null,
      forkOwner: forkInfo.owner,
      forkRepo: forkInfo.repo,
    });
  } else {
    repo.setForkContext(null);
  }
}
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header">
      <router-link to="/" class="logo">MergePilot</router-link>
    </div>

    <!-- Platform selector -->
    <div class="platform-selector">
      <button
        v-for="p in platforms"
        :key="p.value"
        :class="{ active: auth.activePlatform === p.value }"
        @click="selectPlatform(p.value)"
      >
        {{ p.label }}
      </button>
    </div>

    <!-- Auth status -->
    <div class="auth-status">
      <template v-if="auth.isLoggedIn && auth.activeUser">
        <img :src="auth.activeUser.avatar_url" class="avatar" />
        <span class="username">{{ auth.activeUser.login }}</span>
      </template>
      <router-link v-else to="/login" class="login-link">登录</router-link>
    </div>

    <!-- Navigation -->
    <nav class="nav">
      <router-link to="/pr" :class="{ active: isActive('pr') }">
        📋 Pull Requests
      </router-link>
      <router-link to="/issue" :class="{ active: isActive('issue') }">
        🐛 Issues
      </router-link>
    </nav>

    <!-- Repo list -->
    <div class="repo-section" v-if="auth.isLoggedIn">
      <h4>仓库</h4>
      <div v-if="repo.loading && repo.repos.length === 0" class="repo-loading">加载中...</div>
      <div v-else class="repo-list">
        <template v-for="r in repo.repos" :key="r.id">
          <!-- Fork repo: fork icon ⑂ + name, upstream info in PR page banner -->
          <button
            v-if="r.fork"
            :class="{ active: repo.activeFullName === r.parent_full_name || repo.activeFullName === r.full_name, 'is-fork': true }"
            :title="r.parent_full_name ? '⑂ Fork 仓库，默认查看上游 ' + r.parent_full_name + ' 的 PR' : r.full_name"
            @click="selectForkRepo(r, true)"
          >
            <span class="fork-icon">⑂</span> {{ r.full_name }}
          </button>
          <!-- Normal (non-fork) repo -->
          <button
            v-else
            :class="{ active: repo.activeFullName === r.full_name }"
            :title="r.full_name"
            @click="selectRepo(getRepoOwner(r.full_name)); repo.setForkContext(null)"
          >
            {{ r.full_name }}
          </button>
        </template>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 240px;
  background: var(--color-surface);
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  overflow-y: auto;
}

.sidebar-header {
  padding: 16px 16px 12px;
  border-bottom: 1px solid var(--color-border);
}

.logo {
  font-size: 18px;
  font-weight: 700;
  color: var(--color-text);
}

.platform-selector {
  display: flex;
  padding: 8px;
  gap: 4px;
}

.platform-selector button {
  flex: 1;
  padding: 6px 4px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: none;
  font-size: 11px;
  transition: all 0.2s;
}

.platform-selector button.active {
  background: var(--color-primary);
  color: #fff;
  border-color: var(--color-primary);
}

.auth-status {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--color-border);
}

.avatar {
  width: 24px;
  height: 24px;
  border-radius: 50%;
}

.username {
  font-size: 13px;
  font-weight: 600;
}

.login-link {
  font-size: 13px;
}

.nav {
  display: flex;
  flex-direction: column;
  padding: 8px;
  gap: 2px;
}

.nav a {
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--color-text);
  transition: background 0.15s;
}

.nav a:hover {
  background: #f0f0f0;
}

.nav a.active {
  background: #e8f0fe;
  color: var(--color-primary);
  font-weight: 600;
}

.repo-section {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  padding: 8px;
}

.repo-section h4 {
  font-size: 11px;
  text-transform: uppercase;
  color: var(--color-text-secondary);
  padding: 8px 4px 4px;
}

.repo-loading {
  padding: 16px;
  color: var(--color-text-secondary);
  font-size: 12px;
}

.repo-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.repo-list button {
  text-align: left;
  padding: 6px 8px;
  border: none;
  background: none;
  font-size: 12px;
  border-radius: 4px;
  color: var(--color-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  transition: background 0.15s;
  cursor: pointer;
}

.repo-list button:hover {
  background: #f0f0f0;
}

.repo-list button.active {
  background: #e8f0fe;
  color: var(--color-primary);
}

/* Fork repos: indent slightly, marked with fork icon */
.repo-list button.is-fork {
  padding: 6px 8px;
  font-size: 12px;
  color: var(--color-text);
}

.fork-icon {
  font-size: 11px;
  margin-right: 2px;
}
</style>
