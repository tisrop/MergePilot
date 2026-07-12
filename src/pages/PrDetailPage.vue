<script setup lang="ts">
import { onMounted, ref, computed, watch } from "vue";
import { useRoute } from "vue-router";
import { useAuthStore } from "@/stores/useAuthStore";
import { usePrStore } from "@/stores/usePrStore";
import { reviewCommentAdd } from "@/api";
import AppLayout from "@/components/layout/AppLayout.vue";
import DiffViewer from "@/components/diff/DiffViewer.vue";
import ReviewForm from "@/components/review/ReviewForm.vue";
import ReviewList from "@/components/review/ReviewList.vue";
import AiReviewPanel from "@/components/ai/AiReviewPanel.vue";
import type { Platform, MergeStrategy, PrFile } from "@/types";

function extractDiffHunk(files: PrFile[], path: string, line: number): string | undefined {
  const file = files.find((f) => f.filename === path);
  if (!file?.patch) return undefined;
  const patchLines = file.patch.split("\n");
  let currentLine = 0;
  let inHunk = false;
  let result: string[] = [];
  for (const pl of patchLines) {
    const hunkMatch = pl.match(/^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (hunkMatch) {
      if (inHunk && result.length > 0) {
        const rangeStart = parseInt(hunkMatch[1], 10);
        if (rangeStart > line + 5) break;
      }
      currentLine = parseInt(hunkMatch[1], 10) - 1;
      inHunk = false;
      result = [pl];
      continue;
    }
    if (result.length > 0) {
      if (pl.startsWith("+")) {
        currentLine += 1;
      } else if (pl.startsWith("-")) {
        // skip old content line counting
      } else if (pl.startsWith("\\")) {
        // no-op for no newline markers
      } else {
        currentLine += 1;
      }
      result.push(pl);
      if (currentLine >= line && !inHunk) {
        inHunk = true;
      }
      if (inHunk && currentLine > line + 5) break;
    }
  }
  return result.length > 0 ? result.join("\n") : undefined;
}

const route = useRoute();
const pr = usePrStore();

const platform = route.params.platform as Platform;
const owner = route.params.owner as string;
const repo = route.params.repo as string;
const number = Number(route.params.number);

const activeTab = ref<"diff" | "reviews" | "ai">("diff");

const reviewListRef = ref<InstanceType<typeof ReviewList> | null>(null);
const commentError = ref("");
const commentSuccess = ref(false);

const selectedStrategy = ref<MergeStrategy>("merge");
const closeRelatedIssues = ref(false);
const dropdownOpen = ref(false);
const operating = ref(false);
const statusMsg = ref("");
const mergeWarning = ref("");

const defaultCommitMessage = computed(
  () => `Merge pull request #${number} from ${pr.currentPr?.source_branch ?? ""}`,
);
const commitMessage = ref("");

const STRATEGIES: Record<Platform, { value: MergeStrategy; label: string }[]> = {
  github: [
    { value: "merge", label: "Merge commit" },
    { value: "squash", label: "Squash and merge" },
    { value: "rebase", label: "Rebase and merge" },
  ],
  gitlab: [
    { value: "merge", label: "Merge commit" },
    { value: "squash", label: "Squash and merge" },
  ],
  gitee: [
    { value: "merge", label: "Merge commit" },
    { value: "squash", label: "Squash and merge" },
    { value: "rebase", label: "Rebase and merge" },
  ],
};

const availableStrategies = computed(() => STRATEGIES[platform] ?? STRATEGIES.github);

const mergeButtonLabel = computed(() => {
  const s = availableStrategies.value.find((s) => s.value === selectedStrategy.value);
  return s ? s.label : "Merge";
});

watch(
  () => pr.currentPr,
  (val) => {
    if (val) commitMessage.value = defaultCommitMessage.value;
  },
  { immediate: true },
);

const isOpen = computed(() => pr.currentPr?.summary.state === "open");
const isClosed = computed(() => pr.currentPr?.summary.state === "closed");
const isMerged = computed(() => pr.currentPr?.summary.state === "merged");
const canMerge = computed(() => isOpen.value && pr.currentPr?.mergeable !== false);
const canClose = computed(() => isOpen.value);
const canReopen = computed(() => isClosed.value && !isMerged.value);

async function handleMerge() {
  if (!pr.currentPr || !canMerge.value) return;
  operating.value = true;
  statusMsg.value = "正在合并 PR...";
  try {
    const outcome = await pr.mergePr(
      platform,
      owner,
      repo,
      number,
      selectedStrategy.value,
      undefined,
      commitMessage.value.trim() || undefined,
      closeRelatedIssues.value || undefined,
    );
    const failedIssues = outcome.issue_close_failures.map((failure) => `#${failure.number}`);
    mergeWarning.value =
      failedIssues.length > 0
        ? `PR 已合并，但以下关联 Issue 关闭失败：${failedIssues.join("、")}`
        : "";
    statusMsg.value = "";
  } catch (e) {
    statusMsg.value = "";
  } finally {
    operating.value = false;
    dropdownOpen.value = false;
  }
}

async function handleClose() {
  if (!pr.currentPr) return;
  operating.value = true;
  statusMsg.value = "正在关闭 PR...";
  try {
    await pr.closePr(platform, owner, repo, number);
    statusMsg.value = "";
  } catch (e) {
    statusMsg.value = "";
  } finally {
    operating.value = false;
  }
}

async function handleReopen() {
  if (!pr.currentPr) return;
  operating.value = true;
  statusMsg.value = "正在重新打开 PR...";
  try {
    await pr.reopenPr(platform, owner, repo, number);
    statusMsg.value = "";
  } catch (e) {
    statusMsg.value = "";
  } finally {
    operating.value = false;
  }
}

async function handleAddComment(
  path: string,
  startLine: number,
  endLine: number,
  side: string,
  body?: string,
) {
  if (!pr.currentPr?.head_sha || !body) return;
  commentError.value = "";
  commentSuccess.value = false;
  try {
    const sl = startLine !== endLine ? startLine : null;
    const targetLine = endLine;
    const diffHunk = pr.diff?.files ? extractDiffHunk(pr.diff.files, path, targetLine) : undefined;
    await reviewCommentAdd(
      platform,
      owner,
      repo,
      number,
      pr.currentPr.head_sha,
      path,
      sl,
      targetLine,
      side,
      body,
      diffHunk,
    );
    commentSuccess.value = true;
    setTimeout(() => {
      commentSuccess.value = false;
    }, 3000);
    if (reviewListRef.value) {
      reviewListRef.value.refresh();
    }
  } catch (e: any) {
    commentError.value = e?.toString() || "提交行内评论失败";
  }
}

onMounted(async () => {
  await Promise.all([
    pr.fetchPrDetail(platform, owner, repo, number),
    pr.fetchPrDiff(platform, owner, repo, number),
  ]);
});
</script>

<template>
  <AppLayout>
    <template #header>
      <div class="pr-header">
        <div class="pr-header-top">
          <h2 v-if="pr.currentPr">{{ pr.currentPr.summary.title }}</h2>
          <div class="pr-header-skeleton" v-else>
            <div class="skeleton skeleton-title" />
            <div class="skeleton skeleton-subtitle" />
          </div>
        </div>
        <div class="pr-meta" v-if="pr.currentPr">
          <span class="branch">
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
              <line x1="6" y1="3" x2="6" y2="15" />
              <circle cx="18" cy="6" r="3" />
              <circle cx="6" cy="6" r="3" />
              <circle cx="18" cy="18" r="3" />
            </svg>
            {{ pr.currentPr.source_branch }} → {{ pr.currentPr.target_branch }}
          </span>
          <span class="author">by {{ pr.currentPr.summary.author.login }}</span>
          <span :class="['pr-state-badge', pr.currentPr.summary.state]">
            {{
              { open: "Open", closed: "Closed", merged: "Merged", all: "" }[
                pr.currentPr.summary.state
              ]
            }}
          </span>
        </div>

        <div v-if="pr.currentPr" class="pr-actions">
          <div v-if="isOpen" class="merge-group">
            <div class="merge-btn-wrapper">
              <button
                class="btn btn-primary merge-main"
                :disabled="!canMerge || operating"
                @click="handleMerge"
              >
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
                  <circle cx="18" cy="18" r="3" />
                  <circle cx="6" cy="6" r="3" />
                  <path d="M6 21V9a9 9 0 0 0 9 9" />
                </svg>
                {{ mergeButtonLabel }}
              </button>
              <button
                class="btn btn-primary merge-caret"
                :disabled="!canMerge || operating"
                @click="dropdownOpen = !dropdownOpen"
              >
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
              <div v-if="dropdownOpen" class="merge-dropdown">
                <button
                  v-for="s in availableStrategies"
                  :key="s.value"
                  class="dropdown-item"
                  :class="{ active: selectedStrategy === s.value }"
                  @click="
                    selectedStrategy = s.value;
                    dropdownOpen = false;
                  "
                >
                  {{ s.label }}
                </button>
              </div>
            </div>
            <input
              v-model="commitMessage"
              class="merge-commit-message"
              type="text"
              :disabled="operating"
              placeholder="Commit message"
            />
            <label class="close-issues-checkbox">
              <input v-model="closeRelatedIssues" type="checkbox" :disabled="operating" />
              合并后关闭关联 Issue
            </label>
          </div>

          <div v-if="isOpen" class="close-btn-wrapper">
            <button
              class="btn btn-outline btn-danger"
              :disabled="!canClose || operating"
              @click="handleClose"
            >
              Close
            </button>
          </div>

          <div v-if="canReopen" class="close-btn-wrapper">
            <button class="btn btn-outline btn-reopen" :disabled="operating" @click="handleReopen">
              Reopen
            </button>
          </div>

          <span v-if="statusMsg" class="status-msg">{{ statusMsg }}</span>
        </div>

        <div v-if="pr.error" class="error-box">
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
            操作失败
          </p>
          <p class="error-msg">{{ pr.error }}</p>
        </div>
        <div v-if="mergeWarning" class="merge-warning" role="alert">
          {{ mergeWarning }}
        </div>
      </div>
    </template>

    <div v-if="pr.loading" class="loading-state">
      <div class="skeleton skeleton-tabs" />
      <div class="skeleton skeleton-content" />
    </div>

    <div v-else-if="pr.currentPr" class="pr-detail">
      <div class="tabs">
        <button :class="{ active: activeTab === 'diff' }" @click="activeTab = 'diff'">
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
            <path d="M12 3v18M3 12h18" />
          </svg>
          Diff
        </button>
        <button :class="{ active: activeTab === 'reviews' }" @click="activeTab = 'reviews'">
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
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
          </svg>
          评审意见
        </button>
        <button :class="{ active: activeTab === 'ai' }" @click="activeTab = 'ai'">
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
            <path d="M12 2a4 4 0 0 1 4 4c0 2-2 4-4 4s-4-2-4-4a4 4 0 0 1 4-4z" />
            <path d="M12 14c-4.42 0-8 1.79-8 4v2h16v-2c0-2.21-3.58-4-8-4z" />
          </svg>
          AI 评审
        </button>
      </div>

      <div class="tab-content">
        <div v-if="activeTab === 'diff'">
          <DiffViewer :diff="pr.diff" @add-comment="handleAddComment" />
          <p v-if="commentError" class="error-msg">{{ commentError }}</p>
          <p v-if="commentSuccess" class="success-msg">✓ 行内评论已提交</p>
          <ReviewForm :platform="platform" :owner="owner" :repo="repo" :pr-number="number" />
        </div>
        <div v-else-if="activeTab === 'reviews'">
          <ReviewList
            :platform="platform"
            :owner="owner"
            :repo="repo"
            :pr-number="number"
            :head-sha="pr.currentPr?.head_sha ?? null"
            :diff-files="pr.diff?.files"
          />
        </div>
        <div v-else-if="activeTab === 'ai'">
          <AiReviewPanel
            :platform="platform"
            :owner="owner"
            :repo="repo"
            :pr-number="number"
            :diff="pr.diff?.diff ?? ''"
            :context="
              pr.currentPr ? { title: pr.currentPr.summary.title, body: pr.currentPr.body } : null
            "
          />
        </div>
      </div>
    </div>
  </AppLayout>
</template>

<style scoped>
.pr-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.pr-header-top h2 {
  font-size: 18px;
  font-weight: 700;
}

.pr-meta {
  font-size: 12px;
  color: var(--color-text-secondary);
  display: flex;
  gap: var(--space-3);
  align-items: center;
}

.pr-state-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: var(--radius-full, 999px);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.pr-state-badge.open {
  background: var(--color-success-light, #d4edda);
  color: var(--color-success, #155724);
}

.pr-state-badge.closed {
  background: var(--color-danger-light, #f8d7da);
  color: var(--color-danger, #721c24);
}

.pr-state-badge.merged {
  background: var(--color-primary-light, #cce5ff);
  color: var(--color-primary, #004085);
}

.branch {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-family: var(--font-mono);
  font-size: 12px;
}

.pr-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-top: var(--space-2);
  flex-wrap: wrap;
}

.merge-group {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 1;
}

.merge-btn-wrapper {
  position: relative;
  display: flex;
}

.merge-main {
  width: 180px;
  border-radius: var(--radius-md) 0 0 var(--radius-md);
}

.merge-caret {
  border-radius: 0 var(--radius-md) var(--radius-md) 0;
  padding-left: var(--space-1);
  padding-right: var(--space-1);
  border-left: 1px solid rgba(255, 255, 255, 0.2);
}

.merge-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  margin-top: 2px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  z-index: 100;
  min-width: 180px;
}

.dropdown-item {
  display: block;
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border: none;
  background: none;
  text-align: left;
  font-size: 13px;
  cursor: pointer;
  color: var(--color-text);
}

.dropdown-item:hover {
  background: var(--color-surface-hover);
}

.dropdown-item.active {
  background: var(--color-primary-light);
  color: var(--color-primary);
  font-weight: 600;
}

.status-msg {
  font-size: 12px;
  color: var(--color-text-secondary);
  font-style: italic;
}

.merge-commit-message {
  flex: 1;
  max-width: 420px;
  padding: var(--space-1) var(--space-2);
  font-size: 12px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-bg);
  color: var(--color-text);
}

.close-issues-checkbox {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: 12px;
  color: var(--color-text-secondary);
  cursor: pointer;
  white-space: nowrap;
  user-select: none;
}

.close-issues-checkbox input {
  margin: 0;
}

.loading-state {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.skeleton-tabs {
  height: 40px;
  border-radius: var(--radius-md);
}

.skeleton-content {
  height: 400px;
  border-radius: var(--radius-lg);
}

.pr-header-skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.skeleton-title {
  height: 24px;
  width: 60%;
  border-radius: var(--radius-sm);
}

.skeleton-subtitle {
  height: 16px;
  width: 40%;
  border-radius: var(--radius-sm);
}

.tabs {
  display: flex;
  gap: 0;
  border-bottom: 1px solid var(--color-border);
  margin-bottom: var(--space-4);
}

.tabs button {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-5);
  border: none;
  background: none;
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-secondary);
  border-bottom: 2px solid transparent;
  transition:
    background-color var(--transition-fast),
    border-color var(--transition-fast),
    color var(--transition-fast);
}

.tabs button.active {
  color: var(--color-primary);
  border-bottom-color: var(--color-primary);
}

.tabs button:hover:not(.active) {
  color: var(--color-text);
  background: var(--color-surface-hover);
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

.success-msg {
  color: var(--color-success);
  margin: 0;
  font-size: 12px;
  font-weight: 500;
}

.merge-warning {
  margin-top: var(--space-3);
  padding: var(--space-3);
  border: 1px solid var(--color-warning);
  border-radius: var(--radius-md);
  color: var(--color-warning);
  font-size: 13px;
}
</style>
