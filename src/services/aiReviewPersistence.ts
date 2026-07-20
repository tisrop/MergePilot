import type { AiReviewHistoryEntry, AiReviewResult, AiSuggestion, Platform } from "@/types";

const HISTORY_PREFIX = "mergebeacon:ai-review-history:v1";
const RULES_PREFIX = "mergebeacon:ai-repository-rules:v1";
const MAX_HISTORY_ENTRIES = 20;
const MAX_REPOSITORY_RULES_LENGTH = 12_000;

export interface AiRepositoryRef {
  platform: Platform;
  owner: string;
  repo: string;
}

export interface AiReviewRef extends AiRepositoryRef {
  prNumber: number;
}

function repositoryKey(prefix: string, reference: AiRepositoryRef): string {
  return `${prefix}:${reference.platform}:${encodeURIComponent(reference.owner)}:${encodeURIComponent(reference.repo)}`;
}

function historyKey(reference: AiReviewRef): string {
  return `${repositoryKey(HISTORY_PREFIX, reference)}:${reference.prNumber}`;
}

function readStorage(key: string): unknown {
  try {
    const value = localStorage.getItem(key);
    return value ? JSON.parse(value) : null;
  } catch {
    return null;
  }
}

function writeStorage(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // AI history and repository rules are best-effort local enhancements.
  }
}

function isSuggestion(value: unknown): value is AiSuggestion {
  if (!value || typeof value !== "object") return false;
  const suggestion = value as Partial<AiSuggestion>;
  return (
    typeof suggestion.file === "string" &&
    typeof suggestion.description === "string" &&
    typeof suggestion.category === "string" &&
    ["critical", "major", "minor", "info"].includes(String(suggestion.severity))
  );
}

function isHistoryEntry(value: unknown): value is AiReviewHistoryEntry {
  if (!value || typeof value !== "object") return false;
  const entry = value as Partial<AiReviewHistoryEntry>;
  return (
    typeof entry.id === "string" &&
    Number.isFinite(entry.created_at) &&
    typeof entry.head_sha === "string" &&
    typeof entry.model === "string" &&
    typeof entry.truncated === "boolean" &&
    (entry.mode === "full" || entry.mode === "incremental") &&
    ["all", "security", "performance", "logic", "code_style"].includes(String(entry.focus)) &&
    !!entry.result &&
    typeof entry.result.summary === "string" &&
    Array.isArray(entry.result.suggestions) &&
    entry.result.suggestions.every(isSuggestion)
  );
}

export function loadRepositoryRules(reference: AiRepositoryRef): string {
  try {
    return (localStorage.getItem(repositoryKey(RULES_PREFIX, reference)) ?? "").slice(
      0,
      MAX_REPOSITORY_RULES_LENGTH,
    );
  } catch {
    return "";
  }
}

export function saveRepositoryRules(reference: AiRepositoryRef, rules: string): string {
  const normalized = rules.trim().slice(0, MAX_REPOSITORY_RULES_LENGTH);
  try {
    const key = repositoryKey(RULES_PREFIX, reference);
    if (normalized) localStorage.setItem(key, normalized);
    else localStorage.removeItem(key);
  } catch {
    // Keep the normalized in-memory value available to the caller.
  }
  return normalized;
}

export function loadAiReviewHistory(reference: AiReviewRef): AiReviewHistoryEntry[] {
  const stored = readStorage(historyKey(reference));
  if (!Array.isArray(stored)) return [];
  return stored
    .filter(isHistoryEntry)
    .sort((left, right) => right.created_at - left.created_at)
    .slice(0, MAX_HISTORY_ENTRIES);
}

export function appendAiReviewHistory(
  reference: AiReviewRef,
  entry: AiReviewHistoryEntry,
): AiReviewHistoryEntry[] {
  const history = [entry, ...loadAiReviewHistory(reference).filter((item) => item.id !== entry.id)]
    .sort((left, right) => right.created_at - left.created_at)
    .slice(0, MAX_HISTORY_ENTRIES);
  writeStorage(historyKey(reference), history);
  return history;
}

export function updateAiReviewHistoryResult(
  reference: AiReviewRef,
  entryId: string,
  result: AiReviewResult,
): AiReviewHistoryEntry[] {
  const history = loadAiReviewHistory(reference).map((entry) =>
    entry.id === entryId ? { ...entry, result } : entry,
  );
  writeStorage(historyKey(reference), history);
  return history;
}
