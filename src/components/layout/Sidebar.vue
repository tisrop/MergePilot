<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import { usePrStore } from "@/stores/usePrStore";
import { useUiSettingsStore } from "@/stores/useUiSettingsStore";
import type { Platform, RepoSummary } from "@/types";
import BrandMark from "@/components/shared/BrandMark.vue";

const props = withDefaults(defineProps<{ isDiffFocusMode?: boolean }>(), {
  isDiffFocusMode: false,
});

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
const pr = usePrStore();
const uiSettings = useUiSettingsStore();

const platforms: { value: Platform; label: string }[] = [
  { value: "github", label: "GitHub" },
  { value: "gitlab", label: "GitLab" },
  { value: "gitee", label: "Gitee" },
];

const visiblePlatforms = computed(() => platforms.filter((p) => auth.platformVisibility[p.value]));
const activePlatformLabel = computed(
  () => platforms.find((item) => item.value === auth.activePlatform)?.label ?? auth.activePlatform,
);
const activePlatformShortLabel = computed(() => {
  const labels: Record<Platform, string> = { github: "GH", gitlab: "GL", gitee: "GE" };
  return labels[auth.activePlatform];
});
const isSidebarCollapsed = computed(
  () => props.isDiffFocusMode && !uiSettings.isDiffSidebarExpanded,
);
const compactRepoFullName = computed(() => {
  if (repo.activeFullName) return repo.activeFullName;
  const routeOwner = route.params.owner;
  const routeRepo = route.params.repo;
  if (
    route.name !== "pr-detail" ||
    typeof routeOwner !== "string" ||
    typeof routeRepo !== "string"
  ) {
    return null;
  }
  return `${routeOwner}/${routeRepo}`;
});
const compactRepoName = computed(() => compactRepoFullName.value?.split("/").at(-1) ?? null);

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

function toggleDiffSidebar() {
  uiSettings.setDiffSidebarExpanded(!uiSettings.isDiffSidebarExpanded);
}

function selectPlatform(p: Platform) {
  if (p === auth.activePlatform) return;
  auth.setActivePlatform(p);
  pr.clearContext();
  if (route.name === "pr-detail") {
    void router.push({ name: "pr-list" });
  }
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
  <aside
    id="app-sidebar"
    class="sidebar"
    :class="{ 'is-collapsed': isSidebarCollapsed, 'is-focus-mode': isDiffFocusMode }"
  >
    <div class="sidebar-header">
      <div class="sidebar-header-row">
        <router-link
          to="/"
          class="logo"
          aria-label="MergeBeacon 首页"
          :title="isSidebarCollapsed ? 'MergeBeacon 首页' : undefined"
        >
          <span class="logo-mark" aria-hidden="true">
            <BrandMark />
          </span>
          <span class="sidebar-copy">MergeBeacon</span>
        </router-link>
        <button
          v-if="isDiffFocusMode && !isSidebarCollapsed"
          class="sidebar-toggle"
          type="button"
          title="折叠侧栏"
          aria-label="折叠侧栏"
          aria-controls="app-sidebar"
          :aria-expanded="true"
          @click="toggleDiffSidebar"
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="m15 18-6-6 6-6" />
          </svg>
        </button>
      </div>
      <span class="app-caption">PR Review Workspace</span>
    </div>

    <div
      v-if="isSidebarCollapsed"
      class="compact-platform"
      :title="`当前平台：${activePlatformLabel}`"
      :aria-label="`当前平台：${activePlatformLabel}`"
    >
      <span aria-hidden="true">{{ activePlatformShortLabel }}</span>
    </div>

    <div v-else class="platform-selector">
      <button
        v-for="p in visiblePlatforms"
        :key="p.value"
        :class="{ active: auth.activePlatform === p.value }"
        :aria-pressed="auth.activePlatform === p.value"
        @click="selectPlatform(p.value)"
      >
        {{ p.label }}
      </button>
    </div>

    <!-- Auth status -->
    <div class="auth-status">
      <template v-if="auth.isLoggedIn && auth.activeUser">
        <img :src="auth.activeUser.avatar_url" class="avatar" alt="" />
        <span class="user-copy">
          <span class="user-label">当前账号</span>
          <span class="username">{{ auth.activeUser.login }}</span>
        </span>
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
    <nav class="nav" aria-label="主导航">
      <router-link
        to="/inbox"
        :class="{ active: isActive('review-inbox') }"
        aria-label="PR 收件箱"
        :title="isSidebarCollapsed ? 'PR 收件箱' : undefined"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M4 4h16v16H4z" />
          <path d="m4 8 8 5 8-5" />
        </svg>
        <span class="nav-label">PR 收件箱</span>
      </router-link>
      <router-link
        to="/pr"
        :class="{ active: isActive('pr') }"
        aria-label="拉取请求（PR）"
        :title="isSidebarCollapsed ? '拉取请求（PR）' : undefined"
      >
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
        <span class="nav-label">Pull Requests</span>
      </router-link>
      <router-link
        to="/issue"
        :class="{ active: isActive('issue') }"
        aria-label="Issues"
        :title="isSidebarCollapsed ? 'Issues' : undefined"
      >
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
        <span class="nav-label">Issues</span>
      </router-link>
    </nav>

    <div
      v-if="isSidebarCollapsed && compactRepoName"
      class="compact-repo"
      role="note"
      :title="`当前仓库：${compactRepoFullName}`"
      :aria-label="`当前仓库：${compactRepoFullName}`"
    >
      <svg
        width="15"
        height="15"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
        <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2Z" />
      </svg>
      <span class="compact-repo-name" aria-hidden="true">{{ compactRepoName }}</span>
    </div>

    <!-- Repo list -->
    <div class="repo-section" v-if="auth.isLoggedIn">
      <div class="repo-header">
        <h4>仓库</h4>
        <button
          class="refresh-btn"
          title="刷新仓库列表"
          aria-label="刷新仓库列表"
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
            <span class="repo-item-name">{{ r.name }}</span>
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

    <div class="sidebar-footer">
      <router-link
        to="/settings"
        class="settings-link"
        aria-label="设置"
        :title="isSidebarCollapsed ? '设置' : undefined"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="3" />
          <path
            d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21h-4v-.09A1.7 1.7 0 0 0 8.6 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H3v-4h.09A1.7 1.7 0 0 0 4.6 8.6a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V3h4v.09A1.7 1.7 0 0 0 15.4 4.6a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.4 9c.14.37.36.7.64.96.3.27.68.42 1.08.44H21v4h-.09A1.7 1.7 0 0 0 19.4 15Z"
          />
        </svg>
        <span class="nav-label">设置</span>
      </router-link>
      <button
        v-if="isSidebarCollapsed"
        class="sidebar-toggle"
        type="button"
        title="展开侧栏"
        aria-label="展开侧栏"
        aria-controls="app-sidebar"
        :aria-expanded="false"
        @click="toggleDiffSidebar"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="m9 18 6-6-6-6" />
        </svg>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: var(--sidebar-width);
  background:
    linear-gradient(180deg, rgba(113, 135, 255, 0.045), transparent 150px), var(--color-surface);
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  overflow: hidden;
  box-shadow: 1px 0 0 rgba(255, 255, 255, 0.8);
}

.sidebar-header {
  padding: var(--space-4) var(--space-4) var(--space-3);
}

.logo {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: 18px;
  font-weight: 700;
  color: var(--color-text);
  letter-spacing: -0.03em;
}

.logo:hover {
  color: var(--color-text);
}

.logo-mark {
  display: inline-flex;
  width: 32px;
  height: 32px;
  align-items: center;
  justify-content: center;
  border-radius: 9px;
  color: var(--color-brand-accent);
  background: var(--gradient-beacon);
  box-shadow: 0 5px 12px rgba(20, 43, 73, 0.22);
}

.logo-mark svg {
  width: 20px;
  height: 20px;
}

.app-caption {
  display: block;
  margin: 3px 0 0 40px;
  color: var(--color-text-tertiary);
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.platform-selector {
  display: flex;
  margin: var(--space-1) var(--space-3) var(--space-2);
  padding: 3px;
  gap: 2px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-bg);
}

.platform-selector button {
  flex: 1;
  min-height: 30px;
  padding: 5px 4px;
  border: none;
  border-radius: 6px;
  background: none;
  font-size: 11px;
  font-weight: 600;
  color: var(--color-text-secondary);
  transition:
    background var(--transition-fast),
    color var(--transition-fast),
    box-shadow var(--transition-fast);
}

.platform-selector button.active {
  background: var(--color-surface);
  color: var(--color-primary);
  box-shadow: var(--shadow-sm);
}

.platform-selector button:hover:not(.active) {
  background: var(--color-surface-hover);
}

.auth-status {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: 48px;
  padding: var(--space-2) var(--space-4);
  border-top: 1px solid var(--color-border-light);
  border-bottom: 1px solid var(--color-border-light);
}

.avatar {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 1px solid var(--color-border);
}

.user-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
}

.user-label {
  color: var(--color-text-tertiary);
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.username {
  font-size: 13px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.login-link {
  font-size: 13px;
}

.nav {
  display: flex;
  flex-direction: column;
  padding: var(--space-3);
  gap: var(--space-1);
  border-bottom: 1px solid var(--color-border-light);
}

.nav a {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: 38px;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-secondary);
  transition:
    background-color var(--transition-fast),
    color var(--transition-fast),
    box-shadow var(--transition-fast);
}

.nav a:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.nav a.active {
  background: var(--color-primary-light);
  color: var(--color-primary);
  font-weight: 600;
  box-shadow: inset 3px 0 0 var(--color-brand-accent-strong);
}

.nav a.active svg {
  color: var(--color-brand-accent-strong);
}

.repo-section {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  padding: var(--space-2) var(--space-3) var(--space-3);
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
  width: 30px;
  height: 30px;
  border: none;
  background: none;
  border-radius: var(--radius-sm);
  color: var(--color-text-tertiary);
  cursor: pointer;
  transition:
    background-color var(--transition-fast),
    color var(--transition-fast),
    opacity var(--transition-fast);
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
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 1px;
  overflow-y: auto;
  font-family:
    "Mona Sans VF",
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    "Noto Sans",
    Helvetica,
    Arial,
    sans-serif,
    "Apple Color Emoji",
    "Segoe UI Emoji";
}

.repo-group-header {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  margin-top: var(--space-1);
  padding: 6px var(--space-2) 2px;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
  line-height: 1.5;
  letter-spacing: normal;
}

.repo-group-header:first-child {
  margin-top: 0;
}

.repo-group-header svg {
  flex-shrink: 0;
}

.repo-list button {
  display: flex;
  min-height: 32px;
  align-items: center;
  gap: var(--space-1);
  padding: 5px var(--space-2) 5px var(--space-4);
  overflow: hidden;
  border: none;
  border-radius: var(--radius-md);
  background: none;
  color: #1f2328;
  font-size: 13px;
  font-weight: 400;
  line-height: 1.5;
  letter-spacing: normal;
  text-align: left;
  white-space: nowrap;
  transition: background var(--transition-fast);
}

.repo-item-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.repo-list button:hover {
  background: var(--color-surface-hover);
}

.repo-list button.active {
  background: var(--color-primary-light);
  color: var(--color-primary);
  font-weight: 600;
  box-shadow: inset 2px 0 0 var(--color-primary);
}

.repo-list button.is-fork {
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

.sidebar-header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.sidebar-toggle,
.settings-link {
  display: flex;
  width: 34px;
  height: 34px;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-secondary);
  transition:
    background-color var(--transition-fast),
    border-color var(--transition-fast),
    color var(--transition-fast);
}

.sidebar-toggle:hover,
.settings-link:hover {
  border-color: var(--color-border);
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.sidebar-footer {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  margin-top: auto;
  padding: var(--space-2) var(--space-3) var(--space-3);
  border-top: 1px solid var(--color-border-light);
}

.settings-link {
  width: auto;
  flex: 1;
  justify-content: flex-start;
  gap: var(--space-2);
  padding: 0 var(--space-3);
  font-size: 13px;
  font-weight: 500;
}

.settings-link.router-link-active {
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.compact-platform {
  display: flex;
  width: 32px;
  height: 24px;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  margin: 0 auto var(--space-2);
  border: 1px solid var(--color-primary-border);
  border-radius: var(--radius-sm);
  background: var(--color-primary-light);
  color: var(--color-primary);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.compact-repo {
  display: flex;
  min-height: 44px;
  flex: 1;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  overflow: hidden;
  padding: var(--space-3) var(--space-2);
  border-bottom: 1px solid var(--color-border-light);
  color: var(--color-text-secondary);
}

.compact-repo svg {
  flex-shrink: 0;
  color: var(--color-text-tertiary);
}

.compact-repo-name {
  min-height: 0;
  overflow: hidden;
  color: var(--color-text-secondary);
  font-family:
    "Mona Sans VF",
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    "Noto Sans",
    Helvetica,
    Arial,
    sans-serif;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.02em;
  line-height: 1;
  text-overflow: ellipsis;
  text-orientation: mixed;
  white-space: nowrap;
  writing-mode: vertical-rl;
}

.sidebar.is-collapsed {
  width: 56px;
}

.sidebar.is-collapsed .sidebar-header {
  padding: var(--space-3);
}

.sidebar.is-collapsed .sidebar-header-row {
  justify-content: center;
}

.sidebar.is-collapsed .sidebar-copy,
.sidebar.is-collapsed .app-caption,
.sidebar.is-collapsed .user-copy,
.sidebar.is-collapsed .nav-label {
  display: none;
}

.sidebar.is-collapsed .auth-status {
  min-height: 44px;
  justify-content: center;
  padding: var(--space-2);
}

.sidebar.is-collapsed .avatar {
  width: 28px;
  height: 28px;
}

.sidebar.is-collapsed .login-link {
  font-size: 11px;
}

.sidebar.is-collapsed .nav {
  padding: var(--space-2);
}

.sidebar.is-collapsed .nav a {
  justify-content: center;
  gap: 0;
  min-height: 40px;
  padding: var(--space-2);
}

.sidebar.is-collapsed .repo-section {
  display: none;
}

.sidebar.is-collapsed .sidebar-footer {
  flex: 0 0 auto;
  flex-direction: column;
  justify-content: flex-end;
  padding: var(--space-2) var(--space-2) var(--space-3);
}

.sidebar.is-collapsed .settings-link {
  width: 38px;
  min-height: 38px;
  flex: 0 0 auto;
  justify-content: center;
  padding: var(--space-2);
}

@media (max-width: 900px) {
  .sidebar {
    width: 224px;
  }

  .sidebar.is-collapsed {
    width: 56px;
  }
}
</style>
