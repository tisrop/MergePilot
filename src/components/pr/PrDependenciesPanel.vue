<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { RouterLink } from "vue-router";
import { prDependencies } from "@/api";
import { getErrorMessage } from "@/utils/error";
import type { Platform, PrDependencyGraph, PrDependencyNode, PrState } from "@/types";

const props = defineProps<{
  platform: Platform;
  owner: string;
  repo: string;
  prNumber: number;
  revision: string;
}>();

const graph = ref<PrDependencyGraph | null>(null);
const loading = ref(false);
const error = ref("");
let requestSequence = 0;

const itemName = computed(() => (props.platform === "gitlab" ? "MR" : "PR"));
const nodesByNumber = computed(
  () => new Map((graph.value?.nodes ?? []).map((node) => [node.number, node])),
);
const currentNode = computed(() => nodesByNumber.value.get(graph.value?.current_number ?? 0));
const currentIsOpen = computed(() => currentNode.value?.state === "open");
const orderedNodes = computed(() =>
  (graph.value?.suggested_merge_order ?? [])
    .map((number) => nodesByNumber.value.get(number))
    .filter((node): node is PrDependencyNode => Boolean(node)),
);
const blockingParents = computed(() => new Set(graph.value?.blocking_parent_numbers ?? []));

const stateLabels: Record<PrState, string> = {
  open: "Open",
  closed: "Closed",
  merged: "Merged",
  all: "All",
};

function parentNumbers(number: number): number[] {
  return (graph.value?.edges ?? [])
    .filter((edge) => edge.child_number === number)
    .map((edge) => edge.parent_number);
}

async function loadDependencies(): Promise<void> {
  const sequence = ++requestSequence;
  loading.value = true;
  error.value = "";
  try {
    const result = await prDependencies(props.platform, props.owner, props.repo, props.prNumber);
    if (sequence === requestSequence) graph.value = result;
  } catch (cause) {
    if (sequence !== requestSequence) return;
    graph.value = null;
    error.value = getErrorMessage(cause, "无法读取 PR / MR 依赖关系");
  } finally {
    if (sequence === requestSequence) loading.value = false;
  }
}

watch(
  () => `${props.platform}:${props.owner}:${props.repo}:${props.prNumber}:${props.revision}`,
  () => void loadDependencies(),
  { immediate: true },
);

onUnmounted(() => {
  requestSequence += 1;
});
</script>

<template>
  <section class="dependency-panel" aria-labelledby="dependency-title" :aria-busy="loading">
    <header class="dependency-header">
      <div class="dependency-heading">
        <h3 id="dependency-title">依赖关系</h3>
        <span class="inferred-badge" title="根据同仓库 PR / MR 的源分支与目标分支关系推导">
          分支推导
        </span>
        <span v-if="loading && graph" class="refresh-status" role="status" aria-live="polite">
          刷新中
        </span>
      </div>
      <button
        :class="['refresh-button', { loading }]"
        type="button"
        title="刷新依赖关系"
        aria-label="刷新依赖关系"
        :disabled="loading"
        @click="loadDependencies"
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
          <path d="M20 11a8.1 8.1 0 0 0-15.5-2M4 4v5h5" />
          <path d="M4 13a8.1 8.1 0 0 0 15.5 2M20 20v-5h-5" />
        </svg>
      </button>
    </header>

    <div v-if="loading && !graph" class="dependency-loading" role="status">
      <div class="skeleton dependency-skeleton" />
      <div class="skeleton dependency-skeleton" />
      <div class="skeleton dependency-skeleton short" />
    </div>

    <div v-else-if="error" class="dependency-error" role="alert">
      <span>{{ error }}</span>
      <button class="btn btn-sm" type="button" @click="loadDependencies">重新加载</button>
    </div>

    <template v-else-if="graph">
      <div v-if="graph.truncated" class="dependency-warning" role="status">
        依赖候选数量较多，当前仅展示已发现的关系，结果可能不完整。
      </div>
      <div v-if="graph.has_cycle" class="dependency-warning" role="alert">
        检测到循环分支依赖，无法给出可靠的合并顺序。
      </div>
      <div v-if="!currentIsOpen" class="dependency-history" role="status">
        当前 {{ itemName }} 已结束，仅展示历史依赖关系。
      </div>
      <div
        v-else-if="!graph.has_cycle && blockingParents.size > 0"
        class="dependency-blocked"
        role="status"
      >
        当前 {{ itemName }} 仍依赖 {{ blockingParents.size }} 个尚未合并的父项。
      </div>

      <div v-if="orderedNodes.length <= 1" class="dependency-empty">
        未发现与当前 {{ itemName }} 相连的分支依赖。
      </div>

      <div v-else class="merge-order">
        <div class="order-heading">
          <h4>{{ currentIsOpen ? "建议合并顺序" : "历史依赖关系" }}</h4>
          <span>{{ orderedNodes.length }} 项</span>
        </div>
        <ol class="dependency-flow">
          <li
            v-for="(node, index) in orderedNodes"
            :key="node.number"
            :class="{
              current: node.number === graph.current_number,
              blocker: blockingParents.has(node.number),
            }"
          >
            <span class="order-index" aria-hidden="true">{{ index + 1 }}</span>
            <div class="dependency-node">
              <div class="node-main">
                <RouterLink
                  class="node-title"
                  :title="`#${node.number} ${node.title}`"
                  :to="{
                    name: 'pr-detail',
                    params: {
                      platform,
                      owner,
                      repo,
                      number: node.number,
                    },
                  }"
                >
                  <span>#{{ node.number }}</span>
                  {{ node.title }}
                </RouterLink>
                <div class="node-badges">
                  <span v-if="node.number === graph.current_number" class="current-badge"
                    >当前</span
                  >
                  <span v-if="blockingParents.has(node.number)" class="blocker-badge">
                    {{ node.state === "closed" ? "已关闭，依赖未合并" : "阻塞当前项" }}
                  </span>
                  <span :class="['state-badge', node.state]">{{ stateLabels[node.state] }}</span>
                </div>
              </div>
              <div class="branch-chain">
                <code>{{ node.source_branch }}</code>
                <span aria-hidden="true">→</span>
                <code>{{ node.target_branch }}</code>
              </div>
              <div v-if="parentNumbers(node.number).length > 0" class="parent-links">
                依赖
                <span v-for="parent in parentNumbers(node.number)" :key="parent"
                  >#{{ parent }}</span
                >
              </div>
            </div>
          </li>
        </ol>
      </div>
    </template>
  </section>
</template>

<style scoped>
.dependency-panel {
  min-height: 240px;
}

.dependency-header,
.dependency-heading,
.node-main,
.node-badges,
.branch-chain,
.parent-links,
.order-heading,
.dependency-error {
  display: flex;
  align-items: center;
}

.dependency-header {
  justify-content: space-between;
  margin-bottom: var(--space-4);
}

.dependency-heading {
  gap: var(--space-2);
}

.dependency-heading h3,
.order-heading h4 {
  margin: 0;
  color: var(--color-text);
}

.dependency-heading h3 {
  font-size: 16px;
}

.inferred-badge,
.current-badge,
.blocker-badge,
.state-badge {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 1px var(--space-2);
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}

.inferred-badge {
  border: 1px solid var(--color-border);
  color: var(--color-text-tertiary);
}

.refresh-status {
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.refresh-button {
  display: inline-flex;
  width: 32px;
  height: 32px;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface);
  color: var(--color-text-secondary);
}

.refresh-button:hover:not(:disabled) {
  border-color: var(--color-primary-border);
  color: var(--color-primary);
}

.refresh-button.loading svg {
  animation: dependency-refresh-spin 0.8s linear infinite;
}

@keyframes dependency-refresh-spin {
  to {
    transform: rotate(360deg);
  }
}

.dependency-loading {
  display: grid;
  gap: var(--space-3);
}

.dependency-skeleton {
  width: 100%;
  height: 76px;
  border-radius: var(--radius-md);
}

.dependency-skeleton.short {
  width: 72%;
}

.dependency-error,
.dependency-warning,
.dependency-blocked,
.dependency-history,
.dependency-empty {
  padding: var(--space-3);
  border-radius: var(--radius-md);
  font-size: 13px;
}

.dependency-error {
  justify-content: space-between;
  gap: var(--space-3);
  border: 1px solid var(--color-danger-border);
  background: var(--color-danger-light);
  color: var(--color-danger);
}

.dependency-warning,
.dependency-blocked,
.dependency-history {
  margin-bottom: var(--space-4);
  border: 1px solid var(--color-warning);
  background: var(--color-warning-light);
  color: var(--color-warning-text, var(--color-warning));
}

.dependency-empty {
  border: 1px dashed var(--color-border);
  color: var(--color-text-tertiary);
  text-align: center;
}

.order-heading {
  justify-content: space-between;
  margin-bottom: var(--space-3);
}

.order-heading h4 {
  font-size: 13px;
}

.order-heading span {
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.dependency-flow {
  margin: 0;
  padding: 0;
  list-style: none;
}

.dependency-flow li {
  position: relative;
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr);
  gap: var(--space-3);
  min-height: 92px;
}

.dependency-flow li:not(:last-child)::before {
  position: absolute;
  top: 32px;
  bottom: 0;
  left: 15px;
  width: 2px;
  background: var(--color-border);
  content: "";
}

.order-index {
  position: relative;
  z-index: 1;
  display: inline-flex;
  width: 32px;
  height: 32px;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--color-border);
  border-radius: 50%;
  background: var(--color-surface);
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 700;
}

.dependency-node {
  min-width: 0;
  margin-bottom: var(--space-3);
  padding: var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface);
}

.dependency-flow li.current .dependency-node {
  border-color: var(--color-primary-border);
  box-shadow: inset 3px 0 0 var(--color-primary);
}

.dependency-flow li.blocker .dependency-node {
  border-color: var(--color-warning);
}

.node-main {
  min-width: 0;
  justify-content: space-between;
  gap: var(--space-3);
}

.node-title {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text);
  font-size: 13px;
  font-weight: 600;
  text-decoration: none;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.node-title:hover {
  color: var(--color-primary);
  text-decoration: underline;
}

.node-title span {
  margin-right: var(--space-1);
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
}

.node-badges {
  flex-shrink: 0;
  gap: var(--space-1);
}

.current-badge {
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.blocker-badge {
  background: var(--color-warning-light);
  color: var(--color-warning-text, var(--color-warning));
}

.state-badge.open {
  background: var(--color-success-light);
  color: var(--color-success);
}

.state-badge.closed {
  background: var(--color-danger-light);
  color: var(--color-danger);
}

.state-badge.merged {
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.branch-chain {
  min-width: 0;
  gap: var(--space-2);
  margin-top: var(--space-2);
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.branch-chain code {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.parent-links {
  gap: var(--space-1);
  margin-top: var(--space-2);
  color: var(--color-text-tertiary);
  font-size: 11px;
}

.parent-links span {
  font-family: var(--font-mono);
}

@media (max-width: 900px) {
  .node-main {
    align-items: flex-start;
    flex-direction: column;
  }

  .node-title {
    width: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .refresh-button.loading svg {
    animation: none;
  }
}
</style>
