import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { reviewInboxList } from "@/api";
import { useReviewInboxStore } from "@/stores/useReviewInboxStore";
import type {
  Paginated,
  Platform,
  ReviewInboxItem,
  ReviewInboxRelationship,
  ReviewInboxStatusSummary,
} from "@/types";

vi.mock("@/api", () => ({
  reviewInboxList: vi.fn(),
}));

function item(
  platform: Platform,
  repository: string,
  number: number,
  updatedAt: string,
  relationships: ReviewInboxRelationship[] = ["reviewer"],
  status: ReviewInboxStatusSummary = {
    status: "unknown",
    draft: null,
    has_conflicts: null,
    checks_status: "unknown",
    approvals_status: "unknown",
    blocking_reasons: [],
  },
): ReviewInboxItem {
  const parts = repository.split("/");
  return {
    platform,
    owner: parts.slice(0, -1).join("/"),
    repo: parts.at(-1) ?? "",
    repository_full_name: repository,
    categories: ["review_requested"],
    relationships,
    status,
    summary: {
      number,
      title: `${platform} #${number}`,
      author: { id: number, login: "author", name: "Author", avatar_url: "" },
      state: "open",
      created_at: updatedAt,
      updated_at: updatedAt,
      labels: [],
    },
  };
}

function page(items: ReviewInboxItem[], current = 1, total = 1): Paginated<ReviewInboxItem> {
  return { items, page: current, total_pages: total, total_count: items.length };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("useReviewInboxStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.mocked(reviewInboxList).mockReset();
  });

  it("聚合已登录平台并按更新时间排序、去重和过滤", async () => {
    vi.mocked(reviewInboxList).mockImplementation(async (platform) => {
      if (platform === "github") {
        return page([
          item("github", "team/a", 1, "2025-01-01T00:00:00Z"),
          item("github", "team/a", 1, "2025-01-01T00:00:00Z"),
        ]);
      }
      return page([item("gitlab", "group/subgroup/b", 2, "2025-01-03T00:00:00Z")]);
    });
    const store = useReviewInboxStore();

    await store.refresh(["github", "gitlab"]);

    expect(store.items.map((entry) => entry.summary.number)).toEqual([2, 1]);
    store.filters.repository = "TEAM/A";
    expect(store.items.map((entry) => entry.repository_full_name)).toEqual(["team/a"]);
    store.filters.repository = "";
    store.filters.platform = "github";
    expect(store.items.map((entry) => entry.platform)).toEqual(["github"]);
  });

  it("去重时合并角色和状态，并支持角色与合并状态筛选", async () => {
    vi.mocked(reviewInboxList).mockResolvedValue(
      page([
        item("github", "team/a", 1, "2025-01-01T00:00:00Z", ["reviewer"], {
          status: "pending",
          draft: false,
          has_conflicts: false,
          checks_status: "pending",
          approvals_status: "ready",
          blocking_reasons: [{ code: "checks_pending", message: "CI 检查仍在进行中" }],
        }),
        item("github", "team/a", 1, "2025-01-01T00:00:00Z", ["assignee"], {
          status: "blocked",
          draft: true,
          has_conflicts: null,
          checks_status: "unknown",
          approvals_status: "blocked",
          blocking_reasons: [{ code: "draft", message: "PR 仍处于 Draft 状态" }],
        }),
        item("github", "team/b", 2, "2025-01-02T00:00:00Z", ["tester"], {
          status: "ready",
          draft: false,
          has_conflicts: false,
          checks_status: "ready",
          approvals_status: "ready",
          blocking_reasons: [],
        }),
      ]),
    );
    const store = useReviewInboxStore();

    await store.refresh(["github"]);

    const merged = store.items.find((entry) => entry.summary.number === 1);
    expect(merged?.relationships).toEqual(["reviewer", "assignee"]);
    expect(merged?.status.status).toBe("blocked");
    expect(merged?.status.draft).toBe(true);
    expect(merged?.status.blocking_reasons).toHaveLength(2);

    store.filters.relationship = "assignee";
    expect(store.items.map((entry) => entry.summary.number)).toEqual([1]);
    store.filters.relationship = "all";
    store.filters.readiness = "ready";
    expect(store.items.map((entry) => entry.summary.number)).toEqual([2]);
  });

  it("平台筛选只请求选中的平台，全部平台模式才执行聚合请求", async () => {
    vi.mocked(reviewInboxList).mockImplementation(async (platform) =>
      page([item(platform, `team/${platform}`, 1, "2025-01-01T00:00:00Z")]),
    );
    const store = useReviewInboxStore();
    store.filters.platform = "gitlab";

    await store.refresh(["github", "gitlab", "gitee"]);

    expect(reviewInboxList).toHaveBeenCalledTimes(1);
    expect(reviewInboxList).toHaveBeenCalledWith("gitlab", "review_requested", 1, 20);
    expect(store.loggedInPlatforms).toEqual(["github", "gitlab", "gitee"]);
    expect(store.items.map((entry) => entry.platform)).toEqual(["gitlab"]);

    vi.mocked(reviewInboxList).mockClear();
    store.filters.platform = "all";
    await store.refresh(["github", "gitlab", "gitee"]);

    expect(reviewInboxList).toHaveBeenCalledTimes(3);
    expect(
      vi
        .mocked(reviewInboxList)
        .mock.calls.map(([platform]) => platform)
        .sort(),
    ).toEqual(["gitee", "github", "gitlab"]);
  });

  it("单个平台失败时保留其他平台结果并允许独立重试", async () => {
    vi.mocked(reviewInboxList).mockImplementation(async (platform) => {
      if (platform === "gitlab") throw new Error("GitLab unavailable");
      return page([item(platform, "team/repo", 1, "2025-01-01T00:00:00Z")]);
    });
    const store = useReviewInboxStore();

    await store.refresh(["github", "gitlab"]);

    expect(store.items).toHaveLength(1);
    expect(store.items[0].platform).toBe("github");
    expect(store.errors.gitlab).toContain("GitLab unavailable");

    vi.mocked(reviewInboxList).mockResolvedValueOnce(
      page([item("gitlab", "group/repo", 2, "2025-01-02T00:00:00Z")]),
    );
    await store.retry("gitlab");
    expect(store.items.map((entry) => entry.platform)).toEqual(["gitlab", "github"]);
    expect(store.errors.gitlab).toBeNull();
  });

  it("切换分类后忽略旧分类迟到响应", async () => {
    const oldRequest = deferred<Paginated<ReviewInboxItem>>();
    vi.mocked(reviewInboxList)
      .mockReturnValueOnce(oldRequest.promise)
      .mockResolvedValueOnce(page([item("github", "team/new", 2, "2025-01-02T00:00:00Z")]));
    const store = useReviewInboxStore();

    const pending = store.refresh(["github"]);
    store.filters.category = "authored";
    await store.refresh(["github"]);
    oldRequest.resolve(page([item("github", "team/old", 1, "2025-01-01T00:00:00Z")]));
    await pending;

    expect(store.items.map((entry) => entry.repository_full_name)).toEqual(["team/new"]);
    expect(reviewInboxList).toHaveBeenNthCalledWith(2, "github", "authored", 1, 20);
  });

  it("按平台独立追加分页且不覆盖已加载条目", async () => {
    vi.mocked(reviewInboxList)
      .mockResolvedValueOnce(page([item("github", "team/a", 1, "2025-01-01T00:00:00Z")], 1, 2))
      .mockResolvedValueOnce(page([item("github", "team/b", 2, "2025-01-02T00:00:00Z")], 2, 2));
    const store = useReviewInboxStore();

    await store.refresh(["github"]);
    await store.loadMore();

    expect(store.items.map((entry) => entry.summary.number)).toEqual([2, 1]);
    expect(store.pages.github).toBe(2);
    expect(store.hasMore).toBe(false);
  });
});
