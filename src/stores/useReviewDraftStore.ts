import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import type { Platform, ReviewEvent } from "@/types";

const STORAGE_KEY = "mergebeacon:review-drafts:v1";
const MAX_DRAFTS = 500;
const PERSIST_DEBOUNCE_MS = 300;

export interface ReviewDraftContext {
  platform: Platform;
  owner: string;
  repo: string;
  prNumber: number;
}

export interface UnifiedReviewDraft {
  id: string;
  source: "manual" | "ai";
  body: string;
  event: ReviewEvent;
  headSha: string;
  path: string;
  startLine: number | null;
  endLine: number | null;
  suggestionIndex: number | null;
  historyId: string | null;
  touchedAt: number;
}

function contextKey(context: ReviewDraftContext): string {
  return `${context.platform}\u0000${context.owner}\u0000${context.repo}\u0000${context.prNumber}`;
}

function loadDrafts(): Record<string, UnifiedReviewDraft[]> {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    const parsed = stored ? (JSON.parse(stored) as unknown) : null;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    const sanitized: Record<string, UnifiedReviewDraft[]> = {};
    let count = 0;
    for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
      if (!Array.isArray(value) || count >= MAX_DRAFTS) continue;
      const drafts = value.filter(isUnifiedDraft).slice(0, MAX_DRAFTS - count);
      if (drafts.length > 0) sanitized[key] = drafts;
      count += drafts.length;
    }
    return sanitized;
  } catch {
    return {};
  }
}

function isUnifiedDraft(value: unknown): value is UnifiedReviewDraft {
  if (!value || typeof value !== "object") return false;
  const draft = value as Partial<UnifiedReviewDraft>;
  return (
    typeof draft.id === "string" &&
    (draft.source === "manual" || draft.source === "ai") &&
    typeof draft.body === "string" &&
    ["comment", "approve", "request_changes"].includes(String(draft.event)) &&
    typeof draft.headSha === "string" &&
    typeof draft.path === "string" &&
    (draft.startLine === null || typeof draft.startLine === "number") &&
    (draft.endLine === null || typeof draft.endLine === "number") &&
    (draft.suggestionIndex === null || typeof draft.suggestionIndex === "number") &&
    (draft.historyId === null || typeof draft.historyId === "string") &&
    typeof draft.touchedAt === "number" &&
    Number.isFinite(draft.touchedAt)
  );
}

function persistDrafts(value: Record<string, UnifiedReviewDraft[]>): void {
  try {
    const flattened = Object.entries(value)
      .flatMap(([key, drafts]) => drafts.map((draft) => ({ key, draft })))
      .filter(({ draft }) => draft.body.trim())
      .sort((left, right) => right.draft.touchedAt - left.draft.touchedAt)
      .slice(0, MAX_DRAFTS);
    const limited: Record<string, UnifiedReviewDraft[]> = {};
    for (const { key, draft } of flattened) (limited[key] ??= []).push(draft);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(limited));
  } catch {
    // Draft persistence is best effort; the active Pinia state remains available.
  }
}

export const useReviewDraftStore = defineStore("review-drafts", () => {
  const draftsByContext = ref<Record<string, UnifiedReviewDraft[]>>(loadDrafts());
  let persistenceTimer: ReturnType<typeof setTimeout> | null = null;

  const totalCount = computed(() =>
    Object.values(draftsByContext.value).reduce((count, drafts) => count + drafts.length, 0),
  );

  function list(context: ReviewDraftContext): UnifiedReviewDraft[] {
    return draftsByContext.value[contextKey(context)] ?? [];
  }

  function count(context: ReviewDraftContext): number {
    return list(context).length;
  }

  function replaceSource(
    context: ReviewDraftContext,
    source: UnifiedReviewDraft["source"],
    drafts: UnifiedReviewDraft[],
  ): void {
    const key = contextKey(context);
    const retained = (draftsByContext.value[key] ?? []).filter((draft) => draft.source !== source);
    const next = [...retained, ...drafts].filter((draft) => draft.body.trim());
    if (next.length > 0) draftsByContext.value[key] = next;
    else delete draftsByContext.value[key];
  }

  function upsert(context: ReviewDraftContext, draft: UnifiedReviewDraft): void {
    const key = contextKey(context);
    const drafts = draftsByContext.value[key] ?? [];
    const index = drafts.findIndex((candidate) => candidate.id === draft.id);
    if (index >= 0) drafts[index] = draft;
    else draftsByContext.value[key] = [...drafts, draft];
  }

  function remove(context: ReviewDraftContext, draftId: string): void {
    const key = contextKey(context);
    const next = (draftsByContext.value[key] ?? []).filter((draft) => draft.id !== draftId);
    if (next.length > 0) draftsByContext.value[key] = next;
    else delete draftsByContext.value[key];
  }

  function flushPersistence(): void {
    if (persistenceTimer) clearTimeout(persistenceTimer);
    persistenceTimer = null;
    persistDrafts(draftsByContext.value);
  }

  watch(
    draftsByContext,
    () => {
      if (persistenceTimer) clearTimeout(persistenceTimer);
      persistenceTimer = setTimeout(flushPersistence, PERSIST_DEBOUNCE_MS);
    },
    { deep: true },
  );

  return {
    draftsByContext,
    totalCount,
    list,
    count,
    replaceSource,
    upsert,
    remove,
    flushPersistence,
  };
});
