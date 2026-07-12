import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { prDetail, prList, prMerge } from "@/api";
import { usePrStore } from "@/stores/usePrStore";
import type { Paginated, PrDetail, PrSummary } from "@/types";

vi.mock("@/api", () => ({
  prList: vi.fn(),
  prDetail: vi.fn(),
  prDiff: vi.fn(),
  prMerge: vi.fn(),
  prClose: vi.fn(),
  prReopen: vi.fn(),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("usePrStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("清空平台上下文后忽略迟到的列表响应", async () => {
    const oldRequest = deferred<Paginated<PrSummary>>();
    vi.mocked(prList)
      .mockReturnValueOnce(oldRequest.promise)
      .mockResolvedValueOnce({ items: [], page: 1, total_pages: 1, total_count: 0 });
    const store = usePrStore();

    const pending = store.fetchPrList("github", "old", "repo");
    store.clearContext();
    await store.fetchPrList("gitlab", "new", "repo");
    oldRequest.resolve({
      items: [
        {
          number: 1,
          title: "旧平台 PR",
          author: { id: 1, login: "old", name: "Old", avatar_url: "" },
          state: "open",
          created_at: "",
          updated_at: "",
          labels: [],
        },
      ],
      page: 1,
      total_pages: 1,
      total_count: 1,
    });
    await pending;

    expect(store.list).toEqual([]);
  });

  it("合并部分成功时仍刷新详情并返回失败 Issue", async () => {
    const outcome = {
      merge: { merged: true, message: "merged", sha: "abc" },
      closed_issues: [1],
      issue_close_failures: [{ number: 2, error: "forbidden" }],
    };
    const detail: PrDetail = {
      summary: {
        number: 42,
        title: "已合并",
        author: { id: 1, login: "user", name: "User", avatar_url: "" },
        state: "merged",
        created_at: "",
        updated_at: "",
        labels: [],
      },
      body: "",
      source_branch: "feature",
      target_branch: "main",
      mergeable: true,
      head_sha: "abc",
    };
    vi.mocked(prMerge).mockResolvedValue(outcome);
    vi.mocked(prDetail).mockResolvedValue(detail);
    const store = usePrStore();

    const result = await store.mergePr("github", "o", "r", 42, "merge", undefined, undefined, true);

    expect(result).toEqual(outcome);
    expect(prDetail).toHaveBeenCalledWith("github", "o", "r", 42);
    expect(store.currentPr?.summary.title).toBe("已合并");
  });
});
