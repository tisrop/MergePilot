<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import type {
  Platform,
  PrComment,
  PrFile,
  Review,
  ReviewThreadSummary,
  StandardPatchFile,
} from "@/types";
import {
  reviewCommentDelete,
  reviewCommentUpdate,
  reviewCommentsList,
  reviewList,
  reviewThreadReply,
  reviewThreadSetResolved,
} from "@/api";
import { getErrorMessage } from "@/utils/error";
import MiniDiffView from "./MiniDiffView.vue";

function extractHunkFromPatch(patch: string, line: number): string | undefined {
  const lines = patch.split("\n");
  let currentLine = 0;
  let result: string[] = [];
  let inRange = false;
  for (const patchLine of lines) {
    const match = patchLine.match(/^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (match) {
      if (inRange) break;
      currentLine = Number.parseInt(match[1], 10) - 1;
      result = [patchLine];
      continue;
    }
    if (result.length > 0) {
      result.push(patchLine);
      if (!patchLine.startsWith("-")) currentLine++;
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
  diffPatches?: StandardPatchFile[];
  canResolveThreads?: boolean;
}>();

const emit = defineEmits<{
  threadSummary: [summary: ReviewThreadSummary];
  locateComment: [path: string, line: number | null];
}>();

type ThreadFilter = "all" | "unresolved" | "resolved";

interface GeneralReviewItem {
  id: string;
  author: Review["author"];
  body: string;
  time: string;
  kind: "general_comment" | "overall_review";
  state: string;
}

interface ReviewThread {
  id: string;
  comments: PrComment[];
  path: string;
  line: number | null;
  startLine: number | null;
  diffHunk: string | null;
  contextLine: number | null;
  displayPath: string;
  canLocate: boolean;
  resolved: boolean | null;
  resolvable: boolean;
  updatedAt: string;
}

const generalItems = ref<GeneralReviewItem[]>([]);
const threads = ref<ReviewThread[]>([]);
const loading = ref(false);
const error = ref("");
const threadFilter = ref<ThreadFilter>("all");
const expandedBodies = ref(new Set<string>());
const codeExpanded = ref(new Set<string>());
const updatingThreads = ref(new Set<string>());
const threadErrors = ref<Record<string, string>>({});
const replyBodies = ref<Record<string, string>>({});
const editingComments = ref(new Set<string>());
const editingBodies = ref<Record<string, string>>({});
const deletingComments = ref(new Set<string>());
const deleteConfirmations = ref(new Set<string>());
const commentErrors = ref<Record<string, string>>({});
const activeThreadId = ref<string | null>(null);
const threadElements = new Map<string, HTMLElement>();
const loadedComments = ref<PrComment[]>([]);
let requestSequence = 0;
let resolutionOperationSequence = 0;
const activeResolutionOperations = new Map<string, number>();

const filteredThreads = computed(() => {
  if (threadFilter.value === "resolved")
    return threads.value.filter((thread) => thread.resolved === true);
  if (threadFilter.value === "unresolved") {
    return threads.value.filter((thread) => thread.resolved === false);
  }
  return threads.value;
});

const resolvedCount = computed(
  () => threads.value.filter((thread) => thread.resolved === true).length,
);
const unresolvedCount = computed(
  () => threads.value.filter((thread) => thread.resolved === false).length,
);

function itemId(threadId: string, comment: PrComment): string {
  return `${threadId}:${String(comment.id)}`;
}

function toggleBody(id: string): void {
  const next = new Set(expandedBodies.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  expandedBodies.value = next;
}

function toggleCode(id: string): void {
  const next = new Set(codeExpanded.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  codeExpanded.value = next;
}

function isOutdated(comment: PrComment): boolean {
  const originalCommit = comment.original_commit_id ?? comment.commit_id;
  if (!originalCommit || !props.headSha) return false;
  return originalCommit !== props.headSha;
}

function threadIsOutdated(thread: ReviewThread): boolean {
  return thread.comments.some(isOutdated);
}

function reviewKind(review: Review): GeneralReviewItem["kind"] {
  return props.platform === "github" ? "overall_review" : "general_comment";
}

function reviewContextKey(): string {
  return [props.platform, props.owner, props.repo, props.prNumber].join("\u0000");
}

function buildThreads(comments: PrComment[]): ReviewThread[] {
  const grouped = new Map<string, PrComment[]>();
  comments.forEach((comment) => {
    const threadId = comment.thread_id || String(comment.id);
    const current = grouped.get(threadId) ?? [];
    current.push(comment);
    grouped.set(threadId, current);
  });

  return [...grouped.entries()]
    .map(([id, threadComments]) => {
      const chronological = [...threadComments].sort(
        (left, right) => new Date(left.created_at).getTime() - new Date(right.created_at).getTime(),
      );
      const root =
        chronological.find((comment) => comment.reply_to_id === null) ?? chronological[0];
      const sorted = [root, ...chronological.filter((comment) => comment !== root)];
      const patch = props.diffPatches?.find(
        (candidate) =>
          candidate.filename === root.path ||
          candidate.old_path === root.path ||
          candidate.new_path === root.path,
      );
      const displayPath = patch?.filename ?? root.path;
      const file = props.diffFiles?.find((candidate) => candidate.filename === displayPath);
      let diffHunk = root.diff_hunk;
      if (!diffHunk && displayPath && root.line && file?.patch) {
        diffHunk = extractHunkFromPatch(file.patch, root.line) ?? null;
      }
      const latest = chronological.at(-1) ?? root;
      const canLocate =
        !props.diffFiles && !props.diffPatches
          ? true
          : Boolean(
              file &&
              (!root.line ||
                !patch ||
                patch.hunks.some((hunk) =>
                  hunk.lines.some(
                    (line) => line.new_line === root.line || line.old_line === root.line,
                  ),
                )),
            );
      return {
        id,
        comments: sorted,
        path: root.path,
        displayPath,
        line: root.line,
        startLine: root.start_line,
        diffHunk,
        contextLine: isOutdated(root) ? (root.original_line ?? root.line) : root.line,
        canLocate,
        resolved: root.resolved,
        resolvable: root.resolvable,
        updatedAt: latest.created_at,
      };
    })
    .sort(
      (left, right) => new Date(right.updatedAt).getTime() - new Date(left.updatedAt).getTime(),
    );
}

function emitSummary(): void {
  const summary: ReviewThreadSummary = {
    comments: threads.value.reduce((total, thread) => total + thread.comments.length, 0),
    threads: threads.value.length,
    unresolved: unresolvedCount.value,
    by_file: {},
  };
  threads.value.forEach((thread) => {
    if (!thread.displayPath) return;
    const current = summary.by_file[thread.displayPath] ?? { comments: 0, unresolved: 0 };
    current.comments += thread.comments.length;
    if (thread.resolved === false) current.unresolved += 1;
    summary.by_file[thread.displayPath] = current;
  });
  emit("threadSummary", summary);
}

function commentKey(thread: ReviewThread, comment: PrComment): string {
  return `${thread.id}:${String(comment.id)}`;
}

function setThreadElement(threadId: string, element: unknown): void {
  if (element instanceof HTMLElement) threadElements.set(threadId, element);
  else threadElements.delete(threadId);
}

function threadContextKey(): string {
  return reviewContextKey();
}

async function reloadAfterMutation(contextKey: string): Promise<void> {
  if (threadContextKey() !== contextKey) return;
  await loadReviews();
}

function setCommentError(key: string, message: string): void {
  commentErrors.value = { ...commentErrors.value, [key]: message };
}

async function replyToThread(thread: ReviewThread): Promise<void> {
  const body = replyBodies.value[thread.id]?.trim() ?? "";
  if (!body || thread.comments.length === 0) return;
  const contextKey = threadContextKey();
  const root = thread.comments[0];
  const operationKey = `${thread.id}:reply`;
  const nextUpdating = new Set(updatingThreads.value);
  nextUpdating.add(operationKey);
  updatingThreads.value = nextUpdating;
  setCommentError(operationKey, "");
  try {
    await reviewThreadReply(
      props.platform,
      props.owner,
      props.repo,
      props.prNumber,
      thread.id,
      String(root.id),
      body,
    );
    if (threadContextKey() === contextKey) {
      replyBodies.value = { ...replyBodies.value, [thread.id]: "" };
    }
    await reloadAfterMutation(contextKey);
  } catch (mutationError) {
    if (threadContextKey() === contextKey)
      setCommentError(operationKey, getErrorMessage(mutationError, "回复线程失败"));
  } finally {
    if (threadContextKey() === contextKey) {
      const after = new Set(updatingThreads.value);
      after.delete(operationKey);
      updatingThreads.value = after;
    }
  }
}

function beginEdit(comment: PrComment): void {
  const key = String(comment.id);
  editingComments.value = new Set(editingComments.value).add(key);
  editingBodies.value = { ...editingBodies.value, [key]: comment.body };
}

function cancelEdit(comment: PrComment): void {
  const key = String(comment.id);
  const next = new Set(editingComments.value);
  next.delete(key);
  editingComments.value = next;
}

async function saveEdit(thread: ReviewThread, comment: PrComment): Promise<void> {
  const commentId = String(comment.id);
  const body = editingBodies.value[commentId]?.trim() ?? "";
  if (!body) return;
  const contextKey = threadContextKey();
  const operationKey = `${thread.id}:${commentId}:edit`;
  const nextUpdating = new Set(updatingThreads.value);
  nextUpdating.add(operationKey);
  updatingThreads.value = nextUpdating;
  setCommentError(operationKey, "");
  try {
    await reviewCommentUpdate(
      props.platform,
      props.owner,
      props.repo,
      props.prNumber,
      thread.id,
      commentId,
      body,
    );
    await reloadAfterMutation(contextKey);
  } catch (mutationError) {
    if (threadContextKey() === contextKey)
      setCommentError(operationKey, getErrorMessage(mutationError, "编辑评论失败"));
  } finally {
    if (threadContextKey() === contextKey) {
      const after = new Set(updatingThreads.value);
      after.delete(operationKey);
      updatingThreads.value = after;
    }
  }
}

async function deleteComment(thread: ReviewThread, comment: PrComment): Promise<void> {
  const commentId = String(comment.id);
  const confirmationKey = commentKey(thread, comment);
  if (!deleteConfirmations.value.has(confirmationKey)) {
    deleteConfirmations.value = new Set(deleteConfirmations.value).add(confirmationKey);
    return;
  }
  const contextKey = threadContextKey();
  const operationKey = `${thread.id}:${commentId}:delete`;
  const nextDeleting = new Set(deletingComments.value);
  nextDeleting.add(commentId);
  deletingComments.value = nextDeleting;
  setCommentError(operationKey, "");
  try {
    await reviewCommentDelete(
      props.platform,
      props.owner,
      props.repo,
      props.prNumber,
      thread.id,
      commentId,
    );
    await reloadAfterMutation(contextKey);
  } catch (mutationError) {
    if (threadContextKey() === contextKey)
      setCommentError(operationKey, getErrorMessage(mutationError, "删除评论失败"));
  } finally {
    if (threadContextKey() === contextKey) {
      const after = new Set(deletingComments.value);
      after.delete(commentId);
      deletingComments.value = after;
    }
  }
}

function navigateUnresolvedThread(direction: -1 | 1): void {
  const candidates = threads.value.filter((thread) => thread.resolved === false);
  if (candidates.length === 0) return;
  threadFilter.value = "all";
  const currentIndex = candidates.findIndex((thread) => thread.id === activeThreadId.value);
  const nextIndex =
    currentIndex < 0
      ? direction > 0
        ? 0
        : candidates.length - 1
      : (currentIndex + direction + candidates.length) % candidates.length;
  const target = candidates[nextIndex];
  activeThreadId.value = target.id;
  void nextTick(() => {
    threadElements.get(target.id)?.scrollIntoView({ behavior: "smooth", block: "center" });
    threadElements.get(target.id)?.focus();
  });
}

async function loadReviews(): Promise<void> {
  const sequence = ++requestSequence;
  loading.value = true;
  error.value = "";
  try {
    const [reviews, comments] = await Promise.all([
      reviewList(props.platform, props.owner, props.repo, props.prNumber),
      reviewCommentsList(props.platform, props.owner, props.repo, props.prNumber),
    ]);
    if (sequence !== requestSequence) return;
    loadedComments.value = comments;
    generalItems.value = reviews
      .filter((review) => review.body.trim().length > 0)
      .map((review) => ({
        id: `review-${String(review.id)}`,
        author: review.author,
        body: review.body,
        time: review.submitted_at,
        kind: reviewKind(review),
        state: review.state,
      }))
      .sort((left, right) => new Date(right.time).getTime() - new Date(left.time).getTime());
    threads.value = buildThreads(comments);
    emitSummary();
  } catch (loadError) {
    if (sequence !== requestSequence) return;
    error.value = getErrorMessage(loadError, "加载评审意见失败");
    generalItems.value = [];
    threads.value = [];
    emitSummary();
  } finally {
    if (sequence === requestSequence) loading.value = false;
  }
}

async function setThreadResolved(thread: ReviewThread, resolved: boolean): Promise<void> {
  if (!props.canResolveThreads || !thread.resolvable || updatingThreads.value.has(thread.id))
    return;
  const contextKey = reviewContextKey();
  const operationKey = `${contextKey}\u0000${thread.id}`;
  const operationId = ++resolutionOperationSequence;
  activeResolutionOperations.set(operationKey, operationId);
  const request = {
    platform: props.platform,
    owner: props.owner,
    repo: props.repo,
    prNumber: props.prNumber,
  };
  const nextUpdating = new Set(updatingThreads.value);
  nextUpdating.add(thread.id);
  updatingThreads.value = nextUpdating;
  threadErrors.value = { ...threadErrors.value, [thread.id]: "" };
  try {
    await reviewThreadSetResolved(
      request.platform,
      request.owner,
      request.repo,
      request.prNumber,
      thread.id,
      resolved,
    );
    if (
      reviewContextKey() !== contextKey ||
      activeResolutionOperations.get(operationKey) !== operationId
    )
      return;
    threads.value = threads.value.map((candidate) =>
      candidate.id === thread.id
        ? {
            ...candidate,
            resolved,
            comments: candidate.comments.map((comment) => ({ ...comment, resolved })),
          }
        : candidate,
    );
    emitSummary();
  } catch (updateError) {
    if (
      reviewContextKey() !== contextKey ||
      activeResolutionOperations.get(operationKey) !== operationId
    )
      return;
    threadErrors.value = {
      ...threadErrors.value,
      [thread.id]: getErrorMessage(updateError, resolved ? "解决线程失败" : "重新打开线程失败"),
    };
  } finally {
    if (activeResolutionOperations.get(operationKey) === operationId) {
      activeResolutionOperations.delete(operationKey);
      if (reviewContextKey() === contextKey) {
        const after = new Set(updatingThreads.value);
        after.delete(thread.id);
        updatingThreads.value = after;
      }
    }
  }
}

function locateThread(thread: ReviewThread): void {
  if (!thread.canLocate) return;
  emit("locateComment", thread.displayPath, thread.line);
}

const PREVIEW_LENGTH = 180;
function needsExpand(body: string): boolean {
  return body.length > PREVIEW_LENGTH;
}

onMounted(loadReviews);
watch(
  () => [props.platform, props.owner, props.repo, props.prNumber] as const,
  () => {
    updatingThreads.value = new Set();
    threadErrors.value = {};
    replyBodies.value = {};
    editingComments.value = new Set();
    editingBodies.value = {};
    deletingComments.value = new Set();
    deleteConfirmations.value = new Set();
    commentErrors.value = {};
    activeThreadId.value = null;
    threadElements.clear();
    expandedBodies.value = new Set();
    codeExpanded.value = new Set();
    void loadReviews();
  },
);

watch(
  [() => props.diffFiles, () => props.diffPatches, () => props.headSha],
  () => {
    if (loadedComments.value.length === 0) return;
    threads.value = buildThreads(loadedComments.value);
    emitSummary();
  },
  { deep: true },
);

defineExpose({ refresh: loadReviews });
</script>

<template>
  <section class="review-list" aria-labelledby="review-list-title">
    <header class="review-list-heading">
      <div>
        <h4 id="review-list-title">评审进度</h4>
        <p>
          整体评审 {{ generalItems.length }} · 行级线程 {{ threads.length }} · 评论
          {{ threads.reduce((total, thread) => total + thread.comments.length, 0) }}
        </p>
      </div>
      <button class="btn btn-sm" type="button" :disabled="loading" @click="loadReviews">
        {{ loading ? "刷新中..." : "刷新" }}
      </button>
    </header>

    <p v-if="error" class="error-msg" role="alert">{{ error }}</p>
    <div v-if="loading && threads.length === 0 && generalItems.length === 0" class="loading-state">
      <div v-for="index in 3" :key="index" class="skeleton skeleton-review" />
    </div>

    <template v-else>
      <section
        v-if="generalItems.length > 0"
        class="review-section"
        aria-labelledby="general-review-title"
      >
        <h5 id="general-review-title">普通评论与整体评审</h5>
        <article v-for="item in generalItems" :key="item.id" class="general-review-item">
          <header class="comment-header">
            <img
              :src="item.author.avatar_url"
              :alt="`${item.author.login} 的头像`"
              class="avatar"
            />
            <strong>{{ item.author.login }}</strong>
            <span class="kind-badge">
              {{ item.kind === "overall_review" ? "整体评审" : "普通评论" }}
            </span>
            <span v-if="item.kind === 'overall_review' && item.state" class="review-state">{{
              item.state
            }}</span>
            <time :datetime="item.time">{{ new Date(item.time).toLocaleString() }}</time>
          </header>
          <p class="comment-body">{{ item.body }}</p>
        </article>
      </section>

      <section class="review-section" aria-labelledby="thread-list-title">
        <div class="thread-section-heading">
          <div>
            <h5 id="thread-list-title">行级评论线程</h5>
            <span
              v-if="threads.some((thread) => thread.resolved !== null)"
              class="resolution-summary"
            >
              未解决 {{ unresolvedCount }} · 已解决 {{ resolvedCount }}
            </span>
          </div>
          <div class="thread-filters" aria-label="线程状态筛选">
            <button
              type="button"
              :class="{ active: threadFilter === 'all' }"
              @click="threadFilter = 'all'"
            >
              全部 {{ threads.length }}
            </button>
            <button
              type="button"
              :class="{ active: threadFilter === 'unresolved' }"
              @click="threadFilter = 'unresolved'"
            >
              未解决 {{ unresolvedCount }}
            </button>
            <button
              type="button"
              :class="{ active: threadFilter === 'resolved' }"
              @click="threadFilter = 'resolved'"
            >
              已解决 {{ resolvedCount }}
            </button>
          </div>
          <div class="thread-navigation" aria-label="未解决线程导航">
            <button
              class="btn btn-sm"
              type="button"
              :disabled="unresolvedCount === 0"
              title="上一个未解决线程"
              @click="navigateUnresolvedThread(-1)"
            >
              上一个未解决
            </button>
            <button
              class="btn btn-sm"
              type="button"
              :disabled="unresolvedCount === 0"
              title="下一个未解决线程"
              @click="navigateUnresolvedThread(1)"
            >
              下一个未解决
            </button>
          </div>
        </div>

        <div v-if="threads.length === 0" class="empty-state">
          <p>暂无行级评论线程</p>
        </div>
        <div v-else-if="filteredThreads.length === 0" class="empty-state">
          <p>当前筛选条件下没有线程</p>
        </div>
        <div v-else class="threads">
          <article
            v-for="thread in filteredThreads"
            :key="thread.id"
            :ref="(element) => setThreadElement(thread.id, element)"
            tabindex="-1"
            class="review-thread"
            :class="{
              outdated: threadIsOutdated(thread),
              resolved: thread.resolved === true,
              active: activeThreadId === thread.id,
            }"
          >
            <header class="thread-header">
              <div class="thread-location">
                <span class="kind-badge">行级评论</span>
                <button
                  type="button"
                  class="path-button"
                  :disabled="!thread.canLocate"
                  :title="thread.canLocate ? '跳转到当前 Diff' : '当前 Diff 无法定位此评论'"
                  @click="locateThread(thread)"
                >
                  {{ thread.displayPath }}<template v-if="thread.line">:{{ thread.line }}</template>
                </button>
                <span v-if="threadIsOutdated(thread)" class="outdated-badge">代码已过期</span>
              </div>
              <div class="thread-status-actions">
                <span v-if="thread.resolved === true" class="resolution-badge resolved"
                  >已解决</span
                >
                <span v-else-if="thread.resolved === false" class="resolution-badge unresolved"
                  >未解决</span
                >
                <span v-else class="resolution-badge local-only">平台未提供解决状态</span>
                <button
                  v-if="canResolveThreads && thread.resolvable"
                  type="button"
                  class="btn btn-sm"
                  :disabled="updatingThreads.has(thread.id)"
                  @click="setThreadResolved(thread, thread.resolved !== true)"
                >
                  {{
                    updatingThreads.has(thread.id)
                      ? "处理中..."
                      : thread.resolved === true
                        ? "重新打开"
                        : "解决线程"
                  }}
                </button>
              </div>
            </header>

            <div
              v-if="thread.diffHunk"
              class="code-context"
              :class="{ collapsed: !codeExpanded.has(thread.id) }"
            >
              <button type="button" class="code-hint" @click="toggleCode(thread.id)">
                <span>{{ codeExpanded.has(thread.id) ? "▾" : "▸" }} 查看评论创建时的代码</span>
                <span v-if="threadIsOutdated(thread)" class="outdated-hint"
                  >代码位置可能已变化</span
                >
              </button>
              <MiniDiffView
                v-if="codeExpanded.has(thread.id)"
                :diff-hunk="thread.diffHunk"
                :outdated="threadIsOutdated(thread)"
                :comment-line="thread.contextLine ?? undefined"
                :comment-start-line="thread.startLine ?? undefined"
              />
            </div>
            <div v-else-if="!thread.canLocate" class="original-context-fallback">
              <strong>当前 Diff 无法定位此评论</strong>
              <span>
                原始位置：{{ thread.path || "未知文件"
                }}<template v-if="thread.contextLine"> :{{ thread.contextLine }}</template>
              </span>
              <span v-if="thread.comments[0]?.original_commit_id">
                原始提交：{{ thread.comments[0].original_commit_id }}
              </span>
              <span>评论正文仍保留在下方线程中。</span>
            </div>

            <ol class="thread-comments">
              <li
                v-for="comment in thread.comments"
                :key="itemId(thread.id, comment)"
                :class="{ reply: comment.reply_to_id !== null }"
              >
                <header class="comment-header">
                  <img
                    :src="comment.author.avatar_url"
                    :alt="`${comment.author.login} 的头像`"
                    class="avatar"
                  />
                  <strong>{{ comment.author.login }}</strong>
                  <span v-if="comment.reply_to_id !== null" class="reply-badge">回复</span>
                  <time :datetime="comment.created_at">{{
                    new Date(comment.created_at).toLocaleString()
                  }}</time>
                  <span v-if="comment.can_edit || comment.can_delete" class="comment-actions">
                    <button
                      v-if="comment.can_edit && !editingComments.has(String(comment.id))"
                      type="button"
                      class="text-button"
                      @click="beginEdit(comment)"
                    >
                      编辑
                    </button>
                    <button
                      v-if="comment.can_delete"
                      type="button"
                      class="text-button danger"
                      :disabled="deletingComments.has(String(comment.id))"
                      @click="deleteComment(thread, comment)"
                    >
                      {{
                        deleteConfirmations.has(commentKey(thread, comment)) ? "确认删除" : "删除"
                      }}
                    </button>
                  </span>
                </header>
                <template v-if="editingComments.has(String(comment.id))">
                  <textarea
                    v-model="editingBodies[String(comment.id)]"
                    class="comment-editor"
                    rows="4"
                  />
                  <div class="comment-edit-actions">
                    <button
                      type="button"
                      class="btn btn-sm btn-primary"
                      :disabled="updatingThreads.has(`${thread.id}:${String(comment.id)}:edit`)"
                      @click="saveEdit(thread, comment)"
                    >
                      保存
                    </button>
                    <button type="button" class="btn btn-sm" @click="cancelEdit(comment)">
                      取消
                    </button>
                  </div>
                </template>
                <button
                  v-else
                  type="button"
                  class="comment-body comment-body-button"
                  :aria-expanded="
                    !needsExpand(comment.body) || expandedBodies.has(itemId(thread.id, comment))
                  "
                  @click="toggleBody(itemId(thread.id, comment))"
                >
                  {{
                    needsExpand(comment.body) && !expandedBodies.has(itemId(thread.id, comment))
                      ? `${comment.body.slice(0, PREVIEW_LENGTH)}...`
                      : comment.body
                  }}
                </button>
                <p
                  v-if="
                    commentErrors[`${thread.id}:${String(comment.id)}:edit`] ||
                    commentErrors[`${thread.id}:${String(comment.id)}:delete`]
                  "
                  class="error-msg comment-error"
                  role="alert"
                >
                  {{
                    commentErrors[`${thread.id}:${String(comment.id)}:edit`] ||
                    commentErrors[`${thread.id}:${String(comment.id)}:delete`]
                  }}
                </p>
              </li>
            </ol>
            <form class="thread-reply-form" @submit.prevent="replyToThread(thread)">
              <textarea
                v-model="replyBodies[thread.id]"
                rows="3"
                placeholder="回复此线程..."
                :disabled="updatingThreads.has(`${thread.id}:reply`)"
              />
              <div class="thread-reply-actions">
                <button
                  type="submit"
                  class="btn btn-sm btn-primary"
                  :disabled="
                    updatingThreads.has(`${thread.id}:reply`) || !replyBodies[thread.id]?.trim()
                  "
                >
                  {{ updatingThreads.has(`${thread.id}:reply`) ? "回复中..." : "回复" }}
                </button>
              </div>
            </form>
            <p
              v-if="commentErrors[`${thread.id}:reply`]"
              class="error-msg thread-error"
              role="alert"
            >
              {{ commentErrors[`${thread.id}:reply`] }}
            </p>
            <p v-if="threadErrors[thread.id]" class="error-msg thread-error" role="alert">
              {{ threadErrors[thread.id] }}
            </p>
          </article>
        </div>
      </section>
    </template>
  </section>
</template>

<style scoped>
.review-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  padding: var(--space-4) 0;
}

.review-list-heading,
.thread-section-heading,
.thread-header,
.comment-header,
.thread-location,
.thread-status-actions {
  display: flex;
  align-items: center;
}

.review-list-heading,
.thread-section-heading,
.thread-header {
  justify-content: space-between;
  gap: var(--space-3);
}

.review-list-heading h4,
.review-section h5 {
  margin: 0;
}

.review-list-heading h4 {
  font-size: 16px;
}

.review-list-heading p,
.resolution-summary {
  margin: var(--space-1) 0 0;
  color: var(--color-text-secondary);
  font-size: 12px;
}

.loading-state,
.review-section,
.threads {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.skeleton-review {
  height: 88px;
  border-radius: var(--radius-lg);
}

.general-review-item,
.review-thread {
  padding: var(--space-4);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-surface);
  box-shadow: var(--shadow-sm);
}

.review-thread.resolved {
  background: color-mix(in srgb, var(--color-surface) 94%, var(--color-success));
}

.review-thread.outdated {
  border-color: var(--color-warning-border);
}

.review-thread.active {
  outline: 2px solid var(--color-primary-border);
  outline-offset: 2px;
}

.comment-header {
  gap: var(--space-2);
  min-width: 0;
  color: var(--color-text-secondary);
  font-size: 12px;
}

.comment-header strong {
  color: var(--color-text);
}

.comment-header time {
  margin-left: auto;
  color: var(--color-text-tertiary);
  white-space: nowrap;
}

.comment-actions {
  display: inline-flex;
  gap: var(--space-2);
  margin-left: var(--space-2);
}

.text-button {
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--color-primary);
  font-size: 11px;
}

.text-button.danger {
  color: var(--color-danger);
}

.avatar {
  width: 22px;
  height: 22px;
  border-radius: 50%;
}

.kind-badge,
.reply-badge,
.review-state,
.outdated-badge,
.resolution-badge {
  padding: 2px 7px;
  border-radius: var(--radius-full, 999px);
  font-size: 11px;
  white-space: nowrap;
}

.kind-badge,
.reply-badge {
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.review-state,
.resolution-badge.local-only {
  background: var(--color-surface-hover);
  color: var(--color-text-secondary);
}

.outdated-badge {
  background: var(--color-warning-light);
  color: var(--color-warning);
}

.resolution-badge.resolved {
  background: var(--color-success-light);
  color: var(--color-success);
}

.resolution-badge.unresolved {
  background: var(--color-danger-light);
  color: var(--color-danger);
}

.comment-body {
  margin: var(--space-3) 0 0;
  color: var(--color-text);
  font-size: 13px;
  line-height: 1.6;
  white-space: pre-wrap;
}

.comment-body-button {
  display: block;
  width: 100%;
  padding: 0;
  border: 0;
  background: transparent;
  text-align: left;
}

.thread-section-heading > div:first-child {
  min-width: 0;
}

.thread-filters {
  display: flex;
  gap: var(--space-1);
  padding: 3px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-surface-hover);
}

.thread-filters button {
  padding: 4px 9px;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  font-size: 12px;
}

.thread-filters button.active {
  background: var(--color-surface);
  color: var(--color-primary);
  box-shadow: var(--shadow-sm);
}

.thread-navigation {
  display: flex;
  gap: var(--space-1);
}

.thread-header {
  align-items: flex-start;
  padding-bottom: var(--space-3);
  border-bottom: 1px solid var(--color-border-light);
}

.thread-location,
.thread-status-actions {
  flex-wrap: wrap;
  gap: var(--space-2);
}

.path-button {
  max-width: 520px;
  padding: 0;
  overflow: hidden;
  border: 0;
  background: transparent;
  color: var(--color-primary);
  font-family: var(--font-mono);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.code-context {
  margin: var(--space-3) 0;
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
}

.code-hint {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: 0;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-surface-hover);
  color: var(--color-primary);
  font-size: 11px;
  text-align: left;
}

.code-context.collapsed .code-hint {
  border-bottom: 0;
}

.outdated-hint {
  color: var(--color-warning);
}

.thread-comments {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: var(--space-3) 0 0;
  padding: 0;
  list-style: none;
}

.thread-comments li.reply {
  margin-left: var(--space-6);
  padding-left: var(--space-3);
  border-left: 2px solid var(--color-primary-border);
}

.original-context-fallback {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2) var(--space-3);
  margin: var(--space-3) 0;
  padding: var(--space-3);
  border: 1px solid var(--color-warning-border);
  border-radius: var(--radius-md);
  background: var(--color-warning-light);
  color: var(--color-text-secondary);
  font-size: 12px;
}

.original-context-fallback strong {
  color: var(--color-warning);
}

.thread-reply-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-top: var(--space-4);
  padding-top: var(--space-3);
  border-top: 1px solid var(--color-border-light);
}

.thread-reply-form textarea,
.comment-editor {
  width: 100%;
  min-height: 64px;
  resize: vertical;
}

.thread-reply-actions,
.comment-edit-actions {
  display: flex;
  gap: var(--space-2);
}

.comment-edit-actions {
  margin-top: var(--space-2);
}

.comment-error {
  margin-top: var(--space-2);
}

.thread-error {
  margin-top: var(--space-3);
}

.empty-state {
  padding: var(--space-8);
  border: 1px dashed var(--color-border);
  border-radius: var(--radius-lg);
  color: var(--color-text-tertiary);
  text-align: center;
}

@media (max-width: 900px) {
  .review-list-heading,
  .thread-section-heading,
  .thread-header {
    align-items: stretch;
    flex-direction: column;
  }

  .thread-filters {
    align-self: flex-start;
    flex-wrap: wrap;
  }

  .thread-navigation {
    flex-wrap: wrap;
  }

  .thread-comments li.reply {
    margin-left: var(--space-3);
  }
}
</style>
