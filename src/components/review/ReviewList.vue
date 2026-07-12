<script setup lang="ts">
import { ref, onMounted } from "vue";
import type { Platform, Review, PrComment, PrFile } from "@/types";
import { reviewList, reviewCommentsList } from "@/api";
import MiniDiffView from "./MiniDiffView.vue";

function extractHunkFromPatch(patch: string, line: number): string | undefined {
  const lines = patch.split("\n");
  let currentLine = 0;
  let result: string[] = [];
  let inRange = false;
  for (const pl of lines) {
    const m = pl.match(/^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (m) {
      if (inRange) break;
      currentLine = parseInt(m[1], 10) - 1;
      result = [pl];
      continue;
    }
    if (result.length > 0) {
      result.push(pl);
      if (!pl.startsWith("-")) currentLine++;
      if (currentLine >= line && !inRange) inRange = true;
      if (inRange && currentLine > line + 8) break;
    }
  }
  return result.length > 0 ? result.join("\n") : undefined;
}

const props = defineProps<{
  platform: Platform;
  owner: string;
  repo: string;
  prNumber: number;
  headSha: string | null;
  diffFiles?: PrFile[];
}>();

interface MergedItem {
  id: string;
  author: { login: string; avatar_url: string };
  body: string;
  time: string;
  kind: "review" | "comment";
  path?: string;
  line?: number;
  startLine?: number;
  commit_id?: string | null;
  original_commit_id?: string | null;
  diff_hunk?: string | null;
}

const items = ref<MergedItem[]>([]);
const loading = ref(false);
const expanded = ref(new Set<string>());
const codeExpanded = ref(new Set<string>());

function toggle(id: string) {
  if (expanded.value.has(id)) {
    expanded.value.delete(id);
  } else {
    expanded.value.add(id);
  }
}

function toggleCode(id: string) {
  if (codeExpanded.value.has(id)) {
    codeExpanded.value.delete(id);
  } else {
    codeExpanded.value.add(id);
  }
}

function isOutdated(item: MergedItem): boolean {
  if (!item.original_commit_id || !props.headSha) return false;
  return item.original_commit_id !== props.headSha;
}

async function loadReviews() {
  loading.value = true;
  try {
    const [reviews, comments] = await Promise.all([
      reviewList(props.platform, props.owner, props.repo, props.prNumber),
      reviewCommentsList(props.platform, props.owner, props.repo, props.prNumber),
    ]);
    const filteredReviews = reviews.filter((r: Review) => r.body.trim().length > 0);
    const merged: MergedItem[] = [
      ...filteredReviews.map((r: Review) => ({
        id: `review-${r.id}`,
        author: r.author,
        body: r.body,
        time: r.submitted_at,
        kind: "review" as const,
      })),
      ...comments.map((c: PrComment) => {
        let hunk = c.diff_hunk;
        if (!hunk && c.path && c.line && props.diffFiles) {
          const file = props.diffFiles.find((f) => f.filename === c.path);
          if (file?.patch) {
            hunk = extractHunkFromPatch(file.patch, c.line) ?? null;
          }
        }
        return {
          id: `comment-${c.id}`,
          author: c.author,
          body: c.body,
          time: c.created_at,
          kind: "comment" as const,
          path: c.path,
          line: c.line ?? undefined,
          startLine: c.start_line ?? undefined,
          commit_id: c.commit_id,
          original_commit_id: c.original_commit_id,
          diff_hunk: hunk,
        };
      }),
    ].sort((a, b) => new Date(b.time).getTime() - new Date(a.time).getTime());
    items.value = merged;
  } catch {
    // ignore
  } finally {
    loading.value = false;
  }
}

onMounted(loadReviews);

defineExpose({
  refresh: loadReviews,
});

const PREVIEW_LEN = 120;
function needsExpand(body: string) {
  return body.length > PREVIEW_LEN;
}
</script>

<template>
  <div class="review-list">
    <h4>评审意见 ({{ items.length }})</h4>

    <div v-if="loading" class="loading-state">
      <div class="skeleton skeleton-review" v-for="i in 3" :key="i" />
    </div>
    <div v-else-if="items.length === 0" class="empty-state">
      <svg
        width="28"
        height="28"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
      </svg>
      <p>暂无评审意见</p>
    </div>

    <div v-else class="reviews">
      <div
        v-for="item in items"
        :key="item.id"
        class="review-item"
        :class="{ outdated: item.kind === 'comment' && isOutdated(item) }"
      >
        <div class="review-header" @click="toggle(item.id)">
          <img :src="item.author.avatar_url" class="avatar" />
          <span class="review-author">{{ item.author.login }}</span>
          <span class="review-kind">{{ item.kind === "comment" ? "行级评论" : "整体评审" }}</span>
          <span v-if="item.path" class="review-path">{{ item.path }}:{{ item.line }}</span>
          <span v-if="item.kind === 'comment' && isOutdated(item)" class="outdated-badge"
            >代码已过期</span
          >
          <span class="review-time">{{ new Date(item.time).toLocaleString() }}</span>
        </div>

        <div
          v-if="item.kind === 'comment' && item.diff_hunk"
          class="code-context"
          :class="{ collapsed: !codeExpanded.has(item.id) }"
        >
          <div class="code-hint" @click="toggleCode(item.id)">
            <span>{{ codeExpanded.has(item.id) ? "▾" : "▸" }} 查看当时代码</span>
            <span v-if="isOutdated(item)" class="outdated-hint">（代码已变更）</span>
          </div>
          <MiniDiffView
            v-if="codeExpanded.has(item.id) && item.diff_hunk"
            :diff-hunk="item.diff_hunk"
            :outdated="isOutdated(item)"
            :comment-line="item.line"
            :comment-start-line="item.startLine"
          />
        </div>

        <div
          class="review-body"
          :class="{ collapsed: needsExpand(item.body) && !expanded.has(item.id) }"
          @click="toggle(item.id)"
        >
          {{
            needsExpand(item.body) && !expanded.has(item.id)
              ? item.body.slice(0, PREVIEW_LEN) + "..."
              : item.body
          }}
        </div>
        <div v-if="needsExpand(item.body)" class="expand-hint" @click="toggle(item.id)">
          {{ expanded.has(item.id) ? "▲ 收起" : "▼ 展开" }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.review-list {
  padding: var(--space-4) 0;
}
h4 {
  margin-bottom: var(--space-4);
  font-size: 15px;
  font-weight: 600;
}

.loading-state {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.skeleton-review {
  height: 80px;
  border-radius: var(--radius-lg);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-10);
  color: var(--color-text-tertiary);
}

.reviews {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.review-item {
  padding: var(--space-4);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  transition:
    border-color var(--transition-base),
    box-shadow var(--transition-base);
}

.review-item:hover {
  border-color: var(--color-primary-border);
  box-shadow: var(--shadow-sm);
}

.review-item.outdated {
  opacity: 0.7;
  border-color: var(--color-danger-border, #f5c6cb);
}

.review-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
  cursor: pointer;
}

.avatar {
  width: 20px;
  height: 20px;
  border-radius: 50%;
}
.review-author {
  font-weight: 600;
  font-size: 13px;
}
.review-kind {
  font-size: 11px;
  padding: 1px 6px;
  background: var(--color-primary-light);
  color: var(--color-primary);
  border-radius: var(--radius-sm);
}
.review-path {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--color-text-secondary);
}
.outdated-badge {
  font-size: 11px;
  padding: 1px 6px;
  background: var(--color-danger-light, #f8d7da);
  color: var(--color-danger, #721c24);
  border-radius: var(--radius-sm);
  font-weight: 600;
}
.review-time {
  margin-left: auto;
  font-size: 11px;
  color: var(--color-text-tertiary);
}

.code-context {
  margin: var(--space-2) 0;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.code-context.collapsed .code-hint {
  border-bottom: none;
}

.code-hint {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-2);
  font-size: 11px;
  font-weight: 500;
  color: var(--color-primary);
  cursor: pointer;
  user-select: none;
  background: var(--color-surface-hover);
  border-bottom: 1px solid var(--color-border);
}

.outdated-hint {
  color: var(--color-danger, #721c24);
  font-weight: 400;
}

.review-body {
  font-size: 13px;
  white-space: pre-wrap;
  line-height: 1.5;
  cursor: pointer;
}
.review-body.collapsed {
  max-height: 3em;
  overflow: hidden;
}

.expand-hint {
  font-size: 11px;
  color: var(--color-primary);
  margin-top: var(--space-1);
  font-weight: 500;
  cursor: pointer;
}
</style>
