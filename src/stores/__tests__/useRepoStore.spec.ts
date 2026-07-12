import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { repoList } from "@/api";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import type { RepoSummary } from "@/types";

vi.mock("@/api", () => ({
  repoList: vi.fn(),
  authLogin: vi.fn(),
  authLogout: vi.fn(),
  authCheck: vi.fn(),
}));

const storage = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

function repo(id: number, fullName: string): RepoSummary {
  const [owner, name] = fullName.split("/");
  return {
    id,
    name,
    full_name: fullName,
    owner,
    owner_type: "user",
    owner_display_name: owner,
    description: "",
    private: false,
    fork: false,
    parent_full_name: null,
    parent_owner: null,
  };
}

describe("useRepoStore", () => {
  beforeEach(() => {
    localStorage.clear();
    setActivePinia(createPinia());
  });

  it("增量追加并去重，且分页按平台隔离", async () => {
    vi.mocked(repoList)
      .mockResolvedValueOnce({ items: [repo(1, "a/one")], page: 1, total_pages: 2, total_count: 2 })
      .mockResolvedValueOnce({
        items: [repo(1, "a/one"), repo(2, "a/two")],
        page: 2,
        total_pages: 2,
        total_count: 2,
      })
      .mockResolvedValueOnce({
        items: [repo(3, "b/three")],
        page: 1,
        total_pages: 3,
        total_count: 3,
      });
    const auth = useAuthStore();
    const store = useRepoStore();

    await store.fetchRepos("github");
    await store.loadMore("github");
    expect(store.reposCache.github.map((item) => item.full_name)).toEqual(["a/one", "a/two"]);

    auth.setActivePlatform("gitlab");
    await store.fetchRepos("gitlab");
    expect(store.repos.map((item) => item.full_name)).toEqual(["b/three"]);
    expect(store.pages.github).toBe(2);
    expect(store.pages.gitlab).toBe(1);
  });

  it("加载更多失败时保留数据并可重试同一页", async () => {
    vi.mocked(repoList)
      .mockResolvedValueOnce({ items: [repo(1, "a/one")], page: 1, total_pages: 2, total_count: 2 })
      .mockRejectedValueOnce(new Error("network"))
      .mockResolvedValueOnce({
        items: [repo(2, "a/two")],
        page: 2,
        total_pages: 2,
        total_count: 2,
      });
    const store = useRepoStore();
    await store.fetchRepos("github");
    await store.loadMore("github");
    expect(store.reposCache.github).toHaveLength(1);
    expect(store.errors.github).toContain("network");
    await store.retry("github");
    expect(store.reposCache.github).toHaveLength(2);
  });
});
