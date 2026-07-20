import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  appendAiReviewHistory,
  loadAiReviewHistory,
  loadRepositoryRules,
  saveRepositoryRules,
  updateAiReviewHistoryResult,
} from "@/services/aiReviewPersistence";
import type { AiReviewHistoryEntry } from "@/types";

const storage = new Map<string, string>();
const reference = { platform: "github" as const, owner: "team", repo: "repo", prNumber: 7 };

function entry(index: number): AiReviewHistoryEntry {
  return {
    id: `entry-${index}`,
    created_at: index,
    head_sha: `head-${index}`,
    base_sha: null,
    focus: "all",
    mode: "full",
    model: "gpt-test",
    truncated: false,
    result: { summary: `result-${index}`, suggestions: [] },
  };
}

describe("aiReviewPersistence", () => {
  beforeEach(() => {
    storage.clear();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
      removeItem: (key: string) => storage.delete(key),
    });
  });

  it("按仓库隔离评审规则并清理首尾空白", () => {
    expect(saveRepositoryRules(reference, "  必须检查并发安全  ")).toBe("必须检查并发安全");
    expect(loadRepositoryRules(reference)).toBe("必须检查并发安全");
    expect(loadRepositoryRules({ ...reference, repo: "other" })).toBe("");
  });

  it("每个 PR 只保留最近 20 条有效历史", () => {
    for (let index = 0; index < 25; index += 1) appendAiReviewHistory(reference, entry(index));

    const history = loadAiReviewHistory(reference);
    expect(history).toHaveLength(20);
    expect(history[0].id).toBe("entry-24");
    expect(history.at(-1)?.id).toBe("entry-5");
  });

  it("持久化历史建议的采纳状态", () => {
    appendAiReviewHistory(reference, entry(1));

    const history = updateAiReviewHistoryResult(reference, "entry-1", {
      summary: "result-1",
      suggestions: [
        {
          file: "src/main.ts",
          line_start: 2,
          line_end: 2,
          severity: "major",
          category: "logic",
          description: "竞态",
          suggestion: null,
          action: "accept",
        },
      ],
    });

    expect(history[0].result.suggestions[0].action).toBe("accept");
    expect(loadAiReviewHistory(reference)[0].result.suggestions[0].action).toBe("accept");
  });
});
