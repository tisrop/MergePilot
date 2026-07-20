import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReviewDraftStore, type UnifiedReviewDraft } from "@/stores/useReviewDraftStore";

const storage = new Map<string, string>();
const context = { platform: "github" as const, owner: "team", repo: "repo", prNumber: 9 };

function draft(source: UnifiedReviewDraft["source"], id: string): UnifiedReviewDraft {
  return {
    id,
    source,
    body: `${source} draft`,
    event: "comment",
    headSha: source === "ai" ? "head-1" : "",
    path: "",
    startLine: null,
    endLine: null,
    suggestionIndex: null,
    historyId: null,
    touchedAt: 1,
  };
}

describe("useReviewDraftStore", () => {
  beforeEach(() => {
    storage.clear();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    setActivePinia(createPinia());
  });

  it("在同一 PR 上统一保存人工和 AI 草稿", () => {
    const store = useReviewDraftStore();

    store.upsert(context, draft("manual", "manual"));
    store.upsert(context, draft("ai", "ai"));

    expect(store.count(context)).toBe(2);
    expect(
      store
        .list(context)
        .map((item) => item.source)
        .sort(),
    ).toEqual(["ai", "manual"]);
  });

  it("替换 AI 草稿时保留人工草稿", () => {
    const store = useReviewDraftStore();
    store.upsert(context, draft("manual", "manual"));
    store.upsert(context, draft("ai", "old-ai"));

    store.replaceSource(context, "ai", [draft("ai", "new-ai")]);

    expect(
      store
        .list(context)
        .map((item) => item.id)
        .sort(),
    ).toEqual(["manual", "new-ai"]);
  });
});
