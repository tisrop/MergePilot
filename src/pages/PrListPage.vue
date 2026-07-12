<script setup lang="ts">
import { onMounted, watch } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import { usePrStore } from "@/stores/usePrStore";
import AppLayout from "@/components/layout/AppLayout.vue";
import PrFilterBar from "@/components/pr/PrFilterBar.vue";
import PrCard from "@/components/pr/PrCard.vue";
import AppSelect from "@/components/shared/AppSelect.vue";

const router = useRouter();
const route = useRoute();
const auth = useAuthStore();
const repo = useRepoStore();
const pr = usePrStore();

async function fetchPrs() {
  if (!auth.isLoggedIn || !repo.activeRepo) return;
  const { owner, repo: repoName } = repo.activeRepo;
  const platform = auth.activePlatform;
  await pr.fetchStateCounts(platform, owner, repoName);
  await pr.fetchPrList(platform, owner, repoName);
}

function switchToFork() {
  repo.switchForkView();
}

onMounted(() => {
  if (auth.isLoggedIn) {
    fetchPrs();
  }
});

watch(
  () => auth.isLoggedIn,
  (loggedIn) => {
    if (loggedIn) fetchPrs();
  },
);

watch(
  () => [auth.activePlatform, repo.activeRepo] as const,
  () => {
    pr.clearContext();
    fetchPrs();
  },
);
watch(
  () => pr.filters,
  () => fetchPrs(),
  { deep: true },
);
watch(
  () => pr.perPage,
  () => fetchPrs(),
);
watch(
  () => route.query._t,
  () => fetchPrs(),
);

function onSelectPr(prNumber: number) {
  if (!repo.activeRepo) return;
  router.push({
    name: "pr-detail",
    params: {
      platform: auth.activePlatform,
      owner: repo.activeRepo.owner,
      repo: repo.activeRepo.repo,
      number: prNumber,
    },
  });
}
</script>

<template>
  <AppLayout>
    <template #header>
      <div class="header-row">
        <h2>Pull Requests</h2>
        <span v-if="repo.activeFullName" class="repo-name">{{ repo.activeFullName }}</span>
      </div>
      <div v-if="repo.forkContext" class="fork-banner">
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <line x1="6" y1="3" x2="6" y2="15" />
          <circle cx="18" cy="6" r="3" />
          <circle cx="6" cy="6" r="3" />
          <circle cx="18" cy="18" r="3" />
        </svg>
        <template v-if="!repo.hasUpstreamInfo">
          这是一个 Fork 仓库，但未获取到上游仓库信息 （请确认 Token
          有足够的仓库权限，或检查终端日志中的 parent 数据）
        </template>
        <template v-else-if="repo.viewingUpstream">
          正在查看上游仓库 <strong>{{ repo.forkContext.upstreamFullName }}</strong> 的 PR
          <button class="fork-switch" @click="switchToFork">查看本仓库 PR</button>
        </template>
        <template v-else>
          正在查看本仓库 PR
          <button class="fork-switch" @click="switchToFork">
            查看上游 {{ repo.forkContext.upstreamFullName }}
          </button>
        </template>
      </div>
      <PrFilterBar />
    </template>

    <div v-if="pr.loading" class="loading-skeleton">
      <div class="skeleton skeleton-card" v-for="i in 5" :key="i" />
    </div>

    <div v-else-if="pr.error" class="error-box">
      <p class="error-title">
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
          <line x1="15" y1="9" x2="9" y2="15" />
          <line x1="9" y1="9" x2="15" y2="15" />
        </svg>
        获取 PR 列表失败
      </p>
      <p class="error-msg">{{ pr.error }}</p>
    </div>

    <div v-else-if="!repo.activeRepo" class="empty-state">
      <svg
        width="32"
        height="32"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4M10 17l5-5-5-5M13 12H3" />
      </svg>
      <p>请先在左侧选择一个仓库</p>
    </div>

    <div v-else-if="pr.list.length === 0" class="empty-state">
      <svg
        width="32"
        height="32"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <circle cx="18" cy="18" r="3" />
        <circle cx="6" cy="6" r="3" />
        <path d="M18 15V9" />
        <path d="M6 9v9" />
        <path d="M13 6h3a2 2 0 0 1 2 2v3" />
      </svg>
      <p>暂无 Pull Request</p>
      <p v-if="repo.activeFullName" class="empty-repo text-secondary font-mono">
        当前仓库：{{ repo.activeFullName }}
      </p>
    </div>

    <div v-else class="pr-list">
      <PrCard
        v-for="item in pr.list"
        :key="item.number"
        :pr="item"
        @click="onSelectPr(item.number)"
      />
    </div>

    <div v-if="pr.list.length > 0 && pr.totalPages > 1" class="pagination">
      <button class="btn btn-sm" :disabled="pr.filters.page <= 1" @click="pr.prevPage()">
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="15 18 9 12 15 6" />
        </svg>
        上一页
      </button>
      <span class="page-info">{{ pr.filters.page }} / {{ pr.totalPages }}</span>
      <button
        class="btn btn-sm"
        :disabled="pr.filters.page >= pr.totalPages"
        @click="pr.nextPage()"
      >
        下一页
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="9 18 15 12 9 6" />
        </svg>
      </button>
      <AppSelect
        size="sm"
        :modelValue="String(pr.perPage)"
        :options="pr.pageSizes.map((s: number) => ({ value: String(s), label: s + ' 条/页' }))"
        @update:modelValue="(v: string) => pr.setPerPage(Number(v))"
      />
    </div>
  </AppLayout>
</template>

<style scoped>
.header-row {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
}

.header-row h2 {
  font-size: 20px;
  font-weight: 700;
}

.repo-name {
  font-size: 13px;
  color: var(--color-text-secondary);
  font-family: var(--font-mono);
}

.fork-banner {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  margin-top: var(--space-1);
  background: var(--color-primary-light);
  border: 1px solid var(--color-primary-border);
  border-radius: var(--radius-md);
  font-size: 13px;
  color: var(--color-primary);
}

.fork-switch {
  padding: 2px 8px;
  border: 1px solid var(--color-primary-border);
  border-radius: var(--radius-sm);
  background: var(--color-surface);
  color: var(--color-primary);
  font-size: 12px;
  cursor: pointer;
  transition: background var(--transition-fast);
}

.fork-switch:hover {
  background: var(--color-primary-light);
}

.loading-skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.skeleton-card {
  height: 76px;
  border-radius: var(--radius-lg);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 240px;
  gap: var(--space-3);
  color: var(--color-text-tertiary);
}

.empty-state p {
  font-size: 14px;
}

.empty-state svg {
  opacity: 0.4;
}

.empty-repo {
  margin-top: var(--space-1);
  font-size: 11px;
}

.error-box {
  margin: var(--space-3) 0;
  padding: var(--space-3) var(--space-4);
  background: var(--color-danger-light);
  border: 1px solid var(--color-danger-border);
  border-radius: var(--radius-lg);
}

.error-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-weight: 600;
  color: var(--color-danger);
  margin: 0 0 var(--space-1) 0;
  font-size: 14px;
}

.error-msg {
  color: var(--color-danger);
  margin: 0;
  font-size: 12px;
  word-break: break-all;
  opacity: 0.8;
}

.pr-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  padding: var(--space-5) 0;
}

.page-info {
  font-size: 13px;
  color: var(--color-text-secondary);
}
</style>
