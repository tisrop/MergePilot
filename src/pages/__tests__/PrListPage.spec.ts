import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import PrListPage from "@/pages/PrListPage.vue";
import type { Platform, PrSummary } from "@/types";

const item: PrSummary = {
  number: 1,
  title: "历史变更",
  author: { id: 1, login: "dev", name: "Dev", avatar_url: "" },
  state: "closed",
  created_at: "",
  updated_at: "",
  labels: [],
};

const mocks = vi.hoisted(() => ({
  router: { push: vi.fn() },
  route: { query: {} as Record<string, string> },
  authStore: {
    activePlatform: "github" as Platform,
    isLoggedIn: false,
  },
  repoStore: {
    activeRepo: { owner: "team", repo: "repo" },
    activeFullName: "team/repo",
    forkContext: null,
    hasUpstreamInfo: false,
    viewingUpstream: false,
    switchForkView: vi.fn(),
  },
  prStore: {
    list: [] as PrSummary[],
    listTruncated: true,
    listTotalCount: 1234,
    loading: false,
    error: null,
    totalPages: 1,
    perPage: 20,
    pageSizes: [10, 20, 50, 100],
    filters: { state: "closed", page: 1 },
    fetchStateCounts: vi.fn(),
    fetchPrList: vi.fn(),
    clearContext: vi.fn(),
    prevPage: vi.fn(),
    nextPage: vi.fn(),
    setPerPage: vi.fn(),
  },
}));

vi.mock("vue-router", () => ({
  useRouter: () => mocks.router,
  useRoute: () => mocks.route,
}));
vi.mock("@/stores/useAuthStore", () => ({ useAuthStore: () => mocks.authStore }));
vi.mock("@/stores/useRepoStore", () => ({ useRepoStore: () => mocks.repoStore }));
vi.mock("@/stores/usePrStore", () => ({ usePrStore: () => mocks.prStore }));

function mountPage(platform: Platform) {
  mocks.authStore.activePlatform = platform;
  mocks.prStore.list = [item];
  return mount(PrListPage, {
    global: {
      stubs: {
        AppLayout: { template: "<main><slot name='header' /><slot /></main>" },
        PrFilterBar: true,
        PrCard: { template: "<article />" },
        AppSelect: true,
        RouterLink: { template: "<a><slot /></a>" },
      },
    },
  });
}

describe("PrListPage 截断提示", () => {
  afterEach(() => {
    mocks.authStore.activePlatform = "github";
    mocks.prStore.list = [];
    mocks.prStore.listTruncated = true;
    mocks.prStore.listTotalCount = 1234;
  });

  it("GitHub 提示真实总数和可浏览上限", () => {
    const wrapper = mountPage("github");

    expect(wrapper.get(".search-limit-notice").text()).toBe(
      "共 1,234 条已关闭或已合并 Pull Request，仅可浏览前 1,000 条。",
    );
  });

  it.each([
    ["gitlab", "GitLab 当前仅返回部分 Merge Request，更多历史记录暂不可分页查看。"],
    ["gitee", "Gitee 当前仅返回部分 Pull Request，更多历史记录暂不可分页查看。"],
  ] as const)("%s 使用平台对应的中性截断提示", (platform, expected) => {
    const wrapper = mountPage(platform);

    expect(wrapper.get(".search-limit-notice").text()).toBe(expected);
  });
});
