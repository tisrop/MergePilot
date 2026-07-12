<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import type { Platform, RepoSummary } from "@/types";

interface OwnerGroup {
  owner: string;
  isOrganization: boolean;
  repos: RepoSummary[];
}

const repoGroups = computed(() => {
  const groups = new Map<string, OwnerGroup>();
  for (const r of repo.repos) {
    const key = r.owner;
    if (!groups.has(key)) {
      groups.set(key, {
        owner: r.owner_display_name || r.owner,
        isOrganization:
          r.owner_type === "organization" ||
          r.owner_type === "group" ||
          r.owner_type === "enterprise",
        repos: [],
      });
    }
    groups.get(key)!.repos.push(r);
  }
  // Sort: organizations first, then personal, alphabetically within each
  const sorted = Array.from(groups.values());
  sorted.sort((a, b) => {
    if (a.isOrganization !== b.isOrganization) return a.isOrganization ? -1 : 1;
    return a.owner.localeCompare(b.owner);
  });
  return sorted;
});

const router = useRouter();
const route = useRoute();
const auth = useAuthStore();
const repo = useRepoStore();

const platforms: { value: Platform; label: string }[] = [
  { value: "github", label: "GitHub" },
  { value: "gitlab", label: "GitLab" },
  { value: "gitee", label: "Gitee" },
];

const visiblePlatforms = computed(() => platforms.filter((p) => auth.platformVisibility[p.value]));

onMounted(async () => {
  const activePlatform = auth.activePlatform;
  if (!auth.platforms[activePlatform].isLoggedIn) {
    await auth.checkAuth(activePlatform);
  }
  if (auth.platforms[activePlatform].isLoggedIn && repo.reposCache[activePlatform].length === 0) {
    void repo.fetchRepos(activePlatform);
  }

  for (const item of platforms) {
    if (item.value !== activePlatform && !auth.platforms[item.value].isLoggedIn) {
      void auth.checkAuth(item.value);
    }
  }
});

function selectPlatform(p: Platform) {
  auth.setActivePlatform(p);
  if (auth.platforms[p].isLoggedIn && repo.reposCache[p].length === 0) {
    void repo.fetchRepos(p);
  }
}

function selectRepo(r: { owner: string; repo: string }) {
  repo.setActiveRepo(r.owner, r.repo);
  router.push({ path: "/pr", query: { _t: Date.now().toString() } });
}

function isActive(nav: string) {
  return String(route.name).startsWith(nav);
}

function getRepoOwner(fullName: string): { owner: string; repo: string } {
  const parts = fullName.split("/");
  return { owner: parts[0], repo: parts.slice(1).join("/") };
}

function effectiveRepo(r: RepoSummary): { owner: string; repo: string } {
  if (r.fork && r.parent_full_name && r.parent_owner) {
    return { owner: r.parent_owner, repo: r.parent_full_name.split("/").slice(1).join("/") };
  }
  return getRepoOwner(r.full_name);
}

function selectForkRepo(r: RepoSummary, useUpstream: boolean) {
  const target = useUpstream ? effectiveRepo(r) : getRepoOwner(r.full_name);
  selectRepo(target);
  const forkInfo = getRepoOwner(r.full_name);
  if (r.fork) {
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

    <div class="platform-selector">
      <button
        v-for="p in visiblePlatforms"
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
      <router-link
        v-else
        :to="{ path: '/login', query: { platform: auth.activePlatform } }"
        class="login-link"
      >
        登录
      </router-link>
    </div>

    <!-- Navigation -->
    <nav class="nav">
      <router-link to="/pr" :class="{ active: isActive('pr') }">
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="18" cy="18" r="3" />
          <circle cx="6" cy="6" r="3" />
          <path d="M18 15V9" />
          <path d="M6 9v9" />
          <path d="M13 6h3a2 2 0 0 1 2 2v3" />
        </svg>
        Pull Requests
      </router-link>
      <router-link to="/issue" :class="{ active: isActive('issue') }">
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="12" cy="12" r="10" />
          <line x1="12" y1="8" x2="12" y2="12" />
          <line x1="12" y1="16" x2="12.01" y2="16" />
        </svg>
        Issues
      </router-link>
    </nav>

    <!-- Repo list -->
    <div class="repo-section" v-if="auth.isLoggedIn">
      <div class="repo-header">
        <h4>仓库</h4>
        <button
          class="refresh-btn"
          title="刷新仓库列表"
          :disabled="repo.loading"
          @click="repo.refreshRepos(auth.activePlatform)"
        >
          <svg
            :class="{ spinning: repo.loading }"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <polyline points="23 4 23 10 17 10"></polyline>
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"></path>
          </svg>
        </button>
      </div>
      <div v-if="repo.loading && repo.repos.length === 0" class="repo-list">
        <div class="loading-hint">加载中...</div>
      </div>
      <div v-else class="repo-list">
        <template v-for="group in repoGroups" :key="group.owner">
          <div class="repo-group-header">
            <svg
              v-if="group.isOrganization"
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <rect x="2" y="8" width="6" height="14" rx="1" />
              <rect x="16" y="8" width="6" height="14" rx="1" />
              <path d="M8 15h8" />
              <path d="M12 22V8" />
              <circle cx="12" cy="4" r="2" />
            </svg>
            <svg
              v-else
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
              <circle cx="12" cy="7" r="4" />
            </svg>
            <span>{{ group.owner }}</span>
          </div>
          <button
            v-for="r in group.repos"
            :key="r.id"
            :class="{
              active:
                repo.activeFullName === r.full_name ||
                (repo.activeFullName !== null && repo.activeFullName === r.parent_full_name),
              'is-fork': r.fork,
            }"
            :title="r.fork && r.parent_full_name ? 'Fork from ' + r.parent_full_name : r.full_name"
            @click="
              r.fork
                ? selectForkRepo(r, true)
                : (selectRepo(getRepoOwner(r.full_name)), repo.setForkContext(null))
            "
          >
            <svg
              v-if="r.fork"
              class="fork-icon"
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <line x1="6" y1="3" x2="6" y2="15" />
              <circle cx="18" cy="6" r="3" />
              <circle cx="6" cy="6" r="3" />
              <circle cx="18" cy="18" r="3" />
            </svg>
            {{ r.name }}
          </button>
        </template>
      </div>
      <div v-if="repo.error" class="repo-load-error">
        <span>加载失败：{{ repo.error }}</span>
        <button @click="repo.retry(auth.activePlatform)">重试</button>
      </div>
      <button
        v-else-if="repo.hasMore"
        class="load-more-btn"
        :disabled="repo.loadingMore"
        @click="repo.loadMore(auth.activePlatform)"
      >
        {{ repo.loadingMore ? "加载中..." : "加载更多" }}
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: var(--sidebar-width);
  background: var(--color-surface);
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  overflow-y: auto;
}

.sidebar-header {
  padding: var(--space-4) var(--space-4) var(--space-3);
  border-bottom: 1px solid var(--color-border);
}

.logo {
  font-size: 18px;
  font-weight: 700;
  color: var(--color-text);
  letter-spacing: -0.02em;
}

.platform-selector {
  display: flex;
  padding: var(--space-2);
  gap: var(--space-1);
}

.platform-selector button {
  flex: 1;
  padding: 6px 4px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: none;
  font-size: 11px;
  font-weight: 500;
  transition: all var(--transition-fast);
}

.platform-selector button.active {
  background: var(--color-primary);
  color: #fff;
  border-color: var(--color-primary);
}

.platform-selector button:hover:not(.active) {
  background: var(--color-surface-hover);
}

.auth-status {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
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
  padding: var(--space-2);
  gap: 2px;
}

.nav a {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-secondary);
  transition: all var(--transition-fast);
}

.nav a:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.nav a.active {
  background: var(--color-primary-light);
  color: var(--color-primary);
  font-weight: 600;
}

.repo-section {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  padding: var(--space-2);
}

.repo-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-2) var(--space-1) var(--space-1);
}

.repo-header h4 {
  font-size: 11px;
  text-transform: uppercase;
  color: var(--color-text-tertiary);
  letter-spacing: 0.05em;
  font-weight: 600;
  padding: 0;
  margin: 0;
}

.refresh-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  background: none;
  border-radius: var(--radius-sm);
  color: var(--color-text-tertiary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.refresh-btn:hover:not(:disabled) {
  color: var(--color-text);
  background: var(--color-surface-hover);
}

.refresh-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.spinning {
  animation: spin 0.8s linear infinite;
}

.loading-hint {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6) var(--space-2);
  font-size: 13px;
  color: var(--color-text-tertiary);
}

.repo-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.repo-group-header {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: 6px var(--space-2) 2px;
  margin-top: var(--space-1);
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-text-tertiary);
}

.repo-group-header:first-child {
  margin-top: 0;
}

.repo-group-header svg {
  flex-shrink: 0;
}

.repo-list button {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  text-align: left;
  padding: 4px var(--space-2) 4px var(--space-4);
  border: none;
  background: none;
  font-size: 12px;
  border-radius: var(--radius-md);
  color: var(--color-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  transition: background var(--transition-fast);
  cursor: pointer;
}

.repo-list button:hover {
  background: var(--color-surface-hover);
}

.repo-list button.active {
  background: var(--color-primary-light);
  color: var(--color-primary);
  font-weight: 600;
}

.repo-list button.is-fork {
  font-size: 12px;
  color: var(--color-text-secondary);
}

.fork-icon {
  flex-shrink: 0;
}

.repo-load-error {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-2);
  color: var(--color-danger);
  font-size: 11px;
}

.repo-load-error button,
.load-more-btn {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  background: var(--color-surface);
  color: var(--color-text-secondary);
  padding: 6px;
  font-size: 11px;
}

.load-more-btn {
  margin-top: var(--space-2);
}
</style>
