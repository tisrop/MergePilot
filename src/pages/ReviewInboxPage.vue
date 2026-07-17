<script setup lang="ts">
import { computed, watch } from "vue";
import { useRouter } from "vue-router";
import AppLayout from "@/components/layout/AppLayout.vue";
import ReviewInboxCard from "@/components/inbox/ReviewInboxCard.vue";
import AppSelect from "@/components/shared/AppSelect.vue";
import { useAuthStore } from "@/stores/useAuthStore";
import { usePrStore } from "@/stores/usePrStore";
import { useRepoStore } from "@/stores/useRepoStore";
import { useReviewInboxStore } from "@/stores/useReviewInboxStore";
import type { Platform, ReviewInboxItem } from "@/types";

const router = useRouter();
const auth = useAuthStore();
const repo = useRepoStore();
const pr = usePrStore();
const inbox = useReviewInboxStore();

const platformLabels: Record<Platform, string> = {
  github: "GitHub",
  gitlab: "GitLab",
  gitee: "Gitee",
};
const categoryOptions = [
  { value: "review_requested", label: "待我处理" },
  { value: "authored", label: "我创建的" },
];
const relationshipOptions = [
  { value: "all", label: "全部角色" },
  { value: "reviewer", label: "评审人" },
  { value: "assignee", label: "负责人" },
  { value: "tester", label: "测试人" },
];
const readinessOptions = [
  { value: "all", label: "全部合并状态" },
  { value: "ready", label: "可合并" },
  { value: "blocked", label: "被阻塞" },
  { value: "pending", label: "检查中" },
  { value: "unknown", label: "状态未知" },
];
const loggedInPlatforms = computed<Platform[]>(() =>
  (Object.keys(auth.platforms) as Platform[]).filter(
    (platform) => auth.platforms[platform].isLoggedIn,
  ),
);
const availablePlatforms = computed<Platform[]>(() =>
  loggedInPlatforms.value.filter((platform) => auth.platformVisibility[platform]),
);
const platformOptions = computed(() => [
  { value: "all", label: "全部已启用平台" },
  ...availablePlatforms.value.map((platform) => ({
    value: platform,
    label: platformLabels[platform],
  })),
]);
const visibleErrors = computed(() =>
  inbox.visiblePlatforms
    .filter((platform) => inbox.errors[platform])
    .map((platform) => ({
      platform,
      label: platformLabels[platform],
      message: inbox.errors[platform] ?? "未知错误",
    })),
);
const hasLoadedItems = computed(() =>
  inbox.visiblePlatforms.some((platform) => inbox.itemsByPlatform[platform].length > 0),
);

watch(
  [() => inbox.filters.category, () => availablePlatforms.value.join(",")],
  () => {
    if (
      inbox.filters.platform !== "all" &&
      !availablePlatforms.value.includes(inbox.filters.platform)
    ) {
      inbox.filters.platform = "all";
      return;
    }
    if (availablePlatforms.value.length > 0) {
      void inbox.refresh(availablePlatforms.value);
    } else {
      inbox.clear();
    }
  },
  { immediate: true },
);

watch(
  () => inbox.filters.platform,
  () => {
    if (availablePlatforms.value.length > 0) {
      void inbox.refresh(availablePlatforms.value);
    } else {
      inbox.clear();
    }
  },
);

function refresh(): void {
  void inbox.refresh(availablePlatforms.value);
}

function openItem(item: ReviewInboxItem): void {
  auth.setActivePlatform(item.platform);
  repo.setActiveRepo(item.owner, item.repo);
  repo.setForkContext(null);
  pr.clearContext();
  void router.push({
    name: "pr-detail",
    params: {
      platform: item.platform,
      owner: item.owner,
      repo: item.repo,
      number: item.summary.number,
    },
  });
}
</script>

<template>
  <AppLayout>
    <template #header>
      <div class="header-row">
        <div>
          <h2>PR 收件箱</h2>
          <p class="subtitle">汇总需要你评审、负责或测试的 PR/MR</p>
        </div>
        <div class="header-actions">
          <span v-if="inbox.items.length" class="result-count"
            >{{ inbox.items.length }} 条结果</span
          >
          <button
            type="button"
            class="refresh-button"
            :disabled="inbox.loading || availablePlatforms.length === 0"
            aria-label="刷新 PR 收件箱"
            title="刷新 PR 收件箱"
            @click="refresh"
          >
            <svg
              :class="{ spinning: inbox.loading }"
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
              <polyline points="23 4 23 10 17 10" />
              <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
            </svg>
          </button>
        </div>
      </div>
      <div class="filter-bar" aria-label="PR 收件箱筛选">
        <div class="filter-field">
          <span>范围</span>
          <AppSelect
            v-model="inbox.filters.category"
            size="sm"
            :options="categoryOptions"
            aria-label="PR 收件箱分类"
          />
        </div>
        <div class="filter-field">
          <span>平台</span>
          <AppSelect
            v-model="inbox.filters.platform"
            size="sm"
            :options="platformOptions"
            aria-label="代码托管平台"
          />
        </div>
        <div class="filter-field">
          <span>角色</span>
          <AppSelect
            v-model="inbox.filters.relationship"
            size="sm"
            :options="relationshipOptions"
            aria-label="收件箱角色"
          />
        </div>
        <div class="filter-field">
          <span>合并状态</span>
          <AppSelect
            v-model="inbox.filters.readiness"
            size="sm"
            :options="readinessOptions"
            aria-label="收件箱合并状态"
          />
        </div>
        <label class="repository-filter">
          <span>仓库</span>
          <input
            v-model="inbox.filters.repository"
            type="search"
            placeholder="筛选 owner/repo"
            autocomplete="off"
          />
        </label>
      </div>
    </template>

    <div v-if="availablePlatforms.length === 0" class="empty-state">
      <svg
        width="32"
        height="32"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        aria-hidden="true"
      >
        <path d="M4 4h16v16H4z" />
        <path d="m4 8 8 5 8-5" />
      </svg>
      <p v-if="loggedInPlatforms.length === 0">请先登录至少一个代码托管平台</p>
      <p v-else>当前没有已登录且启用的平台</p>
    </div>

    <div v-else>
      <div v-if="visibleErrors.length" class="platform-errors" aria-live="polite">
        <div v-for="error in visibleErrors" :key="error.platform" class="platform-error">
          <div>
            <strong>{{ error.label }} 加载失败</strong>
            <span>{{ error.message }}</span>
          </div>
          <button type="button" @click="inbox.retry(error.platform)">重试</button>
        </div>
      </div>

      <div v-if="inbox.loading && !hasLoadedItems" class="loading-skeleton" aria-label="正在加载">
        <div v-for="index in 5" :key="index" class="skeleton skeleton-card" />
      </div>

      <div v-else-if="inbox.items.length" class="inbox-list">
        <ReviewInboxCard
          v-for="item in inbox.items"
          :key="`${item.platform}:${item.repository_full_name}:${item.summary.number}`"
          :item="item"
          @click="openItem(item)"
        />
      </div>

      <div v-else-if="!inbox.loading" class="empty-state">
        <svg
          width="32"
          height="32"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          aria-hidden="true"
        >
          <path d="M4 4h16v16H4z" />
          <path d="m4 8 8 5 8-5" />
        </svg>
        <p
          v-if="
            inbox.filters.repository ||
            inbox.filters.relationship !== 'all' ||
            inbox.filters.readiness !== 'all'
          "
        >
          当前筛选条件下没有结果
        </p>
        <p v-else-if="visibleErrors.length">暂未加载到可展示的 Pull Request</p>
        <p v-else>当前没有需要处理的 Pull Request</p>
      </div>

      <button
        v-if="inbox.hasMore"
        type="button"
        class="load-more-button"
        :disabled="inbox.loading"
        @click="inbox.loadMore"
      >
        {{ inbox.loading ? "加载中..." : "加载更多" }}
      </button>
    </div>
  </AppLayout>
</template>

<style scoped>
.header-row,
.header-actions,
.filter-bar,
.filter-field,
.repository-filter,
.platform-error,
.platform-error > div {
  display: flex;
  align-items: center;
}

.header-row {
  justify-content: space-between;
  gap: var(--space-4);
}

.header-row h2 {
  margin: 0;
  font-size: 20px;
}

.subtitle {
  margin: var(--space-1) 0 0;
  color: var(--color-text-secondary);
  font-size: 12px;
}

.header-actions {
  gap: var(--space-2);
}

.result-count {
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.refresh-button {
  display: inline-flex;
  width: 30px;
  height: 30px;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface);
  color: var(--color-text-secondary);
  cursor: pointer;
}

.refresh-button:hover:not(:disabled) {
  border-color: var(--color-primary-border);
  color: var(--color-primary);
}

.refresh-button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.filter-bar {
  flex-wrap: wrap;
  gap: var(--space-3);
  margin-top: var(--space-4);
}

.filter-field,
.repository-filter {
  gap: var(--space-2);
  color: var(--color-text-secondary);
  font-size: 12px;
}

.filter-field :deep(.app-select) {
  min-width: 128px;
}

.repository-filter input {
  width: 210px;
  height: 30px;
  padding: 0 var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface);
  color: var(--color-text);
  font: inherit;
}

.repository-filter input:focus {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px var(--color-primary-light);
}

.platform-errors {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-bottom: var(--space-4);
}

.platform-error {
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  border: 1px solid var(--color-danger-border);
  border-radius: var(--radius-md);
  background: var(--color-danger-light);
  color: var(--color-danger);
  font-size: 12px;
}

.platform-error > div {
  min-width: 0;
  gap: var(--space-2);
}

.platform-error span {
  overflow: hidden;
  color: var(--color-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.platform-error button,
.load-more-button {
  flex: 0 0 auto;
  padding: 5px 10px;
  border: 1px solid currentColor;
  border-radius: var(--radius-md);
  background: transparent;
  color: inherit;
  font-size: 12px;
  cursor: pointer;
}

.inbox-list,
.loading-skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.skeleton-card {
  height: 108px;
  border-radius: var(--radius-lg);
}

.load-more-button {
  display: block;
  margin: var(--space-4) auto 0;
  border-color: var(--color-border);
  background: var(--color-surface);
  color: var(--color-text-secondary);
}

.load-more-button:hover:not(:disabled) {
  border-color: var(--color-primary-border);
  color: var(--color-primary);
}

.empty-state {
  display: flex;
  min-height: 280px;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: var(--space-3);
  border: 1px dashed var(--color-border);
  border-radius: var(--radius-lg);
  background: rgba(255, 255, 255, 0.72);
  color: var(--color-text-tertiary);
}

.empty-state p {
  margin: 0;
  font-size: 14px;
}

.empty-state svg {
  opacity: 0.45;
}

.spinning {
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .spinning {
    animation: none;
  }
}
</style>
