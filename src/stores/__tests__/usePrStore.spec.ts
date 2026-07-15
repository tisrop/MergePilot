import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { prDetail, prDiff, prList, prMerge, prMergeReadiness } from "@/api";
import { usePrStore } from "@/stores/usePrStore";
import type { DiffResult, Paginated, PrDetail, PrSummary, PrMergeReadiness } from "@/types";

vi.mock("@/api", () => ({
  prList: vi.fn(),
  prDetail: vi.fn(),
  prDiff: vi.fn(),
  prMerge: vi.fn(),
  prMergeReadiness: vi.fn().mockResolvedValue(null),
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

  it("忽略迟到的旧 PR 合并就绪响应", async () => {
    const oldReadiness = deferred<PrMergeReadiness>();
    const currentReadiness: PrMergeReadiness = {
      status: "ready",
      head_sha: "current-sha",
      mergeable: true,
      draft: false,
      has_conflicts: false,
      checks_status: "ready",
      approvals_status: "ready",
      approvals_required: null,
      approvals_received: null,
      has_merge_permission: true,
      branch_behind: false,
      blocking_reasons: [],
    };
    vi.mocked(prMergeReadiness)
      .mockReturnValueOnce(oldReadiness.promise)
      .mockResolvedValueOnce(currentReadiness);
    const store = usePrStore();

    const oldRequest = store.fetchMergeReadiness("github", "old", "repo", 1);
    await store.fetchMergeReadiness("gitlab", "new", "repo", 2);
    oldReadiness.resolve({ ...currentReadiness, head_sha: "old-sha" });
    await oldRequest;

    expect(store.mergeReadiness).toEqual(currentReadiness);
  });

  it("忽略同类请求中较早返回的详情和 diff", async () => {
    const oldDetail = deferred<PrDetail>();
    const oldDiff = deferred<DiffResult>();
    const currentDetail: PrDetail = {
      summary: {
        number: 2,
        title: "当前仓库 PR",
        author: { id: 2, login: "new", name: "New", avatar_url: "" },
        state: "open",
        created_at: "",
        updated_at: "",
        labels: [],
      },
      body: "",
      source_branch: "current",
      target_branch: "main",
      mergeable: true,
      head_sha: "new-sha",
    };
    const currentDiff: DiffResult = { diff: "current diff", files: [] };
    vi.mocked(prDetail).mockReturnValueOnce(oldDetail.promise).mockResolvedValueOnce(currentDetail);
    vi.mocked(prDiff).mockReturnValueOnce(oldDiff.promise).mockResolvedValueOnce(currentDiff);
    const store = usePrStore();

    const oldDetailRequest = store.fetchPrDetail("github", "old", "repo", 1);
    const oldDiffRequest = store.fetchPrDiff("github", "old", "repo", 1);
    await store.fetchPrDetail("gitlab", "new", "repo", 2);
    await store.fetchPrDiff("gitlab", "new", "repo", 2);
    oldDetail.resolve({
      ...currentDetail,
      summary: { ...currentDetail.summary, title: "迟到 PR" },
    });
    oldDiff.resolve({ diff: "late diff", files: [] });
    await Promise.all([oldDetailRequest, oldDiffRequest]);

    expect(store.currentPr?.summary.title).toBe("当前仓库 PR");
    expect(store.diff).toEqual(currentDiff);
  });
});
