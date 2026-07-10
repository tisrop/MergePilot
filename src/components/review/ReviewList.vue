<script setup lang="ts">
import { ref, onMounted } from "vue";
import type { Platform, Review, PrComment } from "@/types";
import { reviewList, reviewCommentsList } from "@/api";

const props = defineProps<{
  platform: Platform;
  owner: string;
  repo: string;
  prNumber: number;
}>();

interface MergedItem {
  id: string;
  author: { login: string; avatar_url: string };
  body: string;
  time: string;
  kind: "review" | "comment";
  path?: string;
  line?: number;
}

const items = ref<MergedItem[]>([]);
const loading = ref(false);
const expanded = ref(new Set<string>());

function toggle(id: string) {
  if (expanded.value.has(id)) {
    expanded.value.delete(id);
  } else {
    expanded.value.add(id);
  }
}

onMounted(async () => {
  loading.value = true;
  try {
    const [reviews, comments] = await Promise.all([
      reviewList(props.platform, props.owner, props.repo, props.prNumber),
      reviewCommentsList(props.platform, props.owner, props.repo, props.prNumber),
    ]);
    const merged: MergedItem[] = [
      ...reviews.map((r: Review) => ({
        id: `review-${r.id}`,
        author: r.author,
        body: r.body,
        time: r.submitted_at,
        kind: "review" as const,
      })),
      ...comments.map((c: PrComment) => ({
        id: `comment-${c.id}`,
        author: c.author,
        body: c.body,
        time: c.created_at,
        kind: "comment" as const,
        path: c.path,
        line: c.line ?? undefined,
      })),
    ].sort((a, b) => new Date(b.time).getTime() - new Date(a.time).getTime());
    items.value = merged;
  } catch {
    // ignore
  } finally {
    loading.value = false;
  }
});

const PREVIEW_LEN = 120;
function needsExpand(body: string) {
  return body.length > PREVIEW_LEN;
}
</script>

<template>
  <div class="review-list">
    <h4>评审意见 ({{ items.length }})</h4>

    <div v-if="loading" class="loading">加载中...</div>
    <div v-else-if="items.length === 0" class="empty">暂无评审意见</div>

    <div v-else class="reviews">
      <div
        v-for="item in items"
        :key="item.id"
        class="review-item"
        @click="toggle(item.id)"
      >
        <div class="review-header">
          <img :src="item.author.avatar_url" class="avatar" />
          <span class="review-author">{{ item.author.login }}</span>
          <span class="review-kind">{{ item.kind === "comment" ? "行级评论" : "整体评审" }}</span>
          <span v-if="item.path" class="review-path">{{ item.path }}:{{ item.line }}</span>
          <span class="review-time">{{ new Date(item.time).toLocaleString() }}</span>
        </div>
        <div class="review-body" :class="{ collapsed: needsExpand(item.body) && !expanded.has(item.id) }">
          {{ needsExpand(item.body) && !expanded.has(item.id) ? item.body.slice(0, PREVIEW_LEN) + '...' : item.body }}
        </div>
        <div v-if="needsExpand(item.body)" class="expand-hint">
          {{ expanded.has(item.id) ? '▲ 收起' : '▼ 展开' }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.review-list { padding: 16px 0; }
h4 { margin-bottom: 16px; }
.loading, .empty { text-align: center; padding: 40px; color: var(--color-text-secondary); }
.reviews { display: flex; flex-direction: column; gap: 12px; }

.review-item {
  padding: 14px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  cursor: pointer;
  transition: border-color 0.15s;
}
.review-item:hover { border-color: var(--color-primary); }

.review-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.avatar { width: 20px; height: 20px; border-radius: 50%; }
.review-author { font-weight: 600; font-size: 13px; }
.review-kind { font-size: 11px; padding: 1px 6px; background: #e8f0fe; color: var(--color-primary); border-radius: 4px; }
.review-path { font-size: 11px; font-family: monospace; color: var(--color-text-secondary); }
.review-time { margin-left: auto; font-size: 11px; color: var(--color-text-secondary); }

.review-body {
  font-size: 13px;
  white-space: pre-wrap;
  line-height: 1.5;
}
.review-body.collapsed {
  max-height: 3em;
  overflow: hidden;
}

.expand-hint {
  font-size: 11px;
  color: var(--color-primary);
  margin-top: 4px;
}
</style>
