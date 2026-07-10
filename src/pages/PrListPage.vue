<script setup lang="ts">
import { onMounted, watch } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import { usePrStore } from "@/stores/usePrStore";
import AppLayout from "@/components/layout/AppLayout.vue";
import PrFilterBar from "@/components/pr/PrFilterBar.vue";
import PrCard from "@/components/pr/PrCard.vue";

const router = useRouter();
const route = useRoute();
const auth = useAuthStore();
const repo = useRepoStore();
const pr = usePrStore();

async function fetchPrs() {
  if (!auth.isLoggedIn || !repo.activeRepo) return;
  await pr.fetchPrList(auth.activePlatform, repo.activeRepo.owner, repo.activeRepo.repo);
}

function switchToFork() {
  repo.switchForkView();
}

onMounted(async () => {
  if (!auth.isLoggedIn) {
    router.push("/login");
    return;
  }
  await fetchPrs();
});

watch(() => repo.activeRepo, () => fetchPrs());
watch(() => pr.filters, () => fetchPrs(), { deep: true });
watch(() => pr.perPage, () => fetchPrs());
watch(() => route.query._t, () => fetchPrs());

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
        <template v-if="!repo.hasUpstreamInfo">
          ⚠ 这是一个 Fork 仓库，但未获取到上游仓库信息
          （请确认 Token 有足够的仓库权限，或检查终端日志中的 parent 数据）
        </template>
        <template v-else-if="repo.viewingUpstream">
          正在查看上游仓库 <strong>{{ repo.forkContext.upstreamFullName }}</strong> 的 PR
          <button class="fork-switch" @click="switchToFork">查看本仓库 PR</button>
        </template>
        <template v-else>
          正在查看本仓库 PR
          <button class="fork-switch" @click="switchToFork">查看上游 {{ repo.forkContext.upstreamFullName }}</button>
        </template>
      </div>
      <PrFilterBar />
    </template>

    <div v-if="pr.loading" class="loading">加载中...</div>

    <div v-else-if="pr.error" class="error-box">
      <p class="error-title">获取 PR 列表失败</p>
      <p class="error-msg">{{ pr.error }}</p>
    </div>

    <div v-else-if="!repo.activeRepo" class="empty">
      <p>请先在左侧选择一个仓库</p>
    </div>

    <div v-else-if="pr.list.length === 0" class="empty">
      <p>暂无 Pull Request</p>
      <p v-if="repo.activeFullName" class="empty-repo">当前仓库：{{ repo.activeFullName }}</p>
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
      <button :disabled="pr.filters.page <= 1" @click="pr.prevPage()">← 上一页</button>
      <span class="page-info">{{ pr.filters.page }} / {{ pr.totalPages }}</span>
      <button :disabled="pr.filters.page >= pr.totalPages" @click="pr.nextPage()">下一页 →</button>
      <select class="page-size-select" :value="pr.perPage" @change="pr.setPerPage(Number(($event.target as HTMLSelectElement).value))">
        <option v-for="s in pr.pageSizes" :key="s" :value="s">{{ s }} 条/页</option>
      </select>
    </div>
  </AppLayout>
</template>

<style scoped>
.header-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.repo-name {
  font-size: 13px;
  color: var(--color-text-secondary);
  font-family: monospace;
}

.fork-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  margin-top: 4px;
  background: #f0f7ff;
  border: 1px solid #b3d8ff;
  border-radius: 6px;
  font-size: 13px;
  color: #3a6fa0;
}

.fork-switch {
  padding: 2px 8px;
  border: 1px solid #b3d8ff;
  border-radius: 4px;
  background: #fff;
  color: #3a6fa0;
  font-size: 12px;
  cursor: pointer;
}

.fork-switch:hover { background: #e6f2ff; }

.loading, .empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--color-text-secondary);
}

.empty-repo {
  margin-top: 4px;
  font-size: 11px;
  font-family: monospace;
  color: var(--color-text-tertiary, #999);
}

.error-box {
  margin: 12px 0;
  padding: 12px 16px;
  background: #fff0f0;
  border: 1px solid #ffcccc;
  border-radius: 6px;
}

.error-title { font-weight: 600; color: #cc0000; margin: 0 0 4px 0; font-size: 14px; }
.error-msg { color: #660000; margin: 0; font-size: 12px; word-break: break-all; }

.pr-list { display: flex; flex-direction: column; gap: 8px; }

.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 20px 0;
}

.pagination button {
  padding: 6px 16px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-surface);
  font-size: 13px;
  cursor: pointer;
}

.pagination button:hover:not(:disabled) { background: #f0f0f0; }
.pagination button:disabled { opacity: 0.35; cursor: default; }

.page-info { font-size: 13px; color: var(--color-text-secondary); }

.page-size-select {
  margin-left: 12px;
  padding: 5px 8px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-surface);
  font-size: 12px;
  color: var(--color-text-secondary);
  cursor: pointer;
}
</style>
