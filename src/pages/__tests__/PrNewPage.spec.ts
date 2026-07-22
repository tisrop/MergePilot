import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PrNewPage from "@/pages/PrNewPage.vue";
import {
  getPlatformCapabilities,
  prBranches,
  prCreate,
  prCreatePreview,
  prLabels,
  prParticipantSuggestions,
  repoList,
} from "@/api";
import { useAuthStore } from "@/stores/useAuthStore";
import { useRepoStore } from "@/stores/useRepoStore";
import { PR_CREATE_WARNING_QUERY, readPrCreateWarnings } from "@/utils/prCreateWarnings";
import type {
  Platform,
  PlatformCapabilities,
  PrCreatePreview,
  PrLabel,
  RepoSummary,
  User,
} from "@/types";

vi.mock("@/api", () => ({
  getPlatformCapabilities: vi.fn(),
  prBranches: vi.fn(),
  prCreate: vi.fn(),
  prCreatePreview: vi.fn(),
  prLabels: vi.fn(),
  prParticipantSuggestions: vi.fn(),
  repoList: vi.fn(),
}));

const diffViewerStub = {
  props: {
    diff: { type: Object, required: true },
    readOnly: { type: Boolean, default: false },
  },
  template: '<div data-testid="diff-preview">{{ diff.files.length }} files</div>',
};

function createPreview(
  title: string,
  sha = "1234567890abcdef",
  filename = "src/a.ts",
  incomplete = false,
): PrCreatePreview {
  return {
    incomplete,
    incomplete_reasons: incomplete ? ["platform_limit"] : [],
    commits: [
      {
        sha,
        title,
        author_name: "Alice",
        authored_at: "2026-07-19T10:00:00Z",
      },
    ],
    diff: {
      diff: "diff --git a/src/a.ts b/src/a.ts",
      files: [
        {
          filename,
          status: "modified",
          patch: "@@ -1 +1 @@\n-old\n+new",
          additions: 1,
          deletions: 1,
        },
      ],
      patch_schema_version: 1,
      patches: [],
    },
  };
}

function platformCapabilities(platform: Platform): PlatformCapabilities {
  return {
    platform,
    review_events: platform === "github" ? ["comment", "approve", "request_changes"] : ["comment"],
    merge_strategies: platform === "gitlab" ? ["merge", "squash"] : ["merge", "squash", "rebase"],
    supports_fork_context: true,
    supports_issue_auto_close: true,
    supports_compare_diff: true,
    supports_review_thread_resolution: platform !== "gitee",
    supports_remote_file_viewed_state: platform === "github",
    supports_pr_title_body_edit: true,
    supports_pr_draft_toggle: platform !== "gitee",
    supports_pr_reviewer_management: true,
    supports_pr_assignee_management: true,
    supports_pr_label_management: true,
    supports_pr_milestone_management: true,
    supports_pr_creation: true,
    merge_queue_kind:
      platform === "github" ? "merge_queue" : platform === "gitlab" ? "merge_train" : null,
  };
}

function repository(
  fullName: string,
  fork = false,
  parentFullName: string | null = null,
): RepoSummary {
  const [owner, ...repo] = fullName.split("/");
  return {
    id: fullName.length,
    name: repo.at(-1) ?? "",
    full_name: fullName,
    owner,
    owner_type: "user",
    owner_display_name: owner,
    description: "",
    private: false,
    fork,
    parent_full_name: parentFullName,
    parent_owner: parentFullName?.split("/")[0] ?? null,
  };
}

async function mountPage(
  platform: Platform = "github",
  cachedRepositories = [repository("team/repo")],
  activePlatform: Platform = platform,
  globalCreation = false,
) {
  const pinia = createPinia();
  setActivePinia(pinia);
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/pr/new/:platform", name: "pr-new", component: PrNewPage },
      { path: "/pr/:platform/:owner/:repo/:number", name: "pr-detail", component: {} },
    ],
  });
  const initialTarget = cachedRepositories[0]?.full_name ?? "team/repo";
  await router.push({
    name: "pr-new",
    params: { platform },
    query: globalCreation ? undefined : { target: initialTarget },
  });
  await router.isReady();
  const auth = useAuthStore();
  auth.setActivePlatform(activePlatform);
  auth.platforms[platform].isLoggedIn = true;
  const repos = useRepoStore();
  const activeRepository = cachedRepositories[0]?.full_name ?? "team/repo";
  const [owner, ...repoParts] = activeRepository.split("/");
  repos.activeRepos[platform] = { owner, repo: repoParts.join("/") };
  repos.reposCache[platform] = cachedRepositories;
  if (cachedRepositories.length > 0) {
    repos.pages[platform] = 1;
    repos.totalPagesByPlatform[platform] = 1;
  }
  const wrapper = mount(PrNewPage, {
    global: {
      plugins: [pinia, router],
      stubs: {
        AppLayout: {
          props: { compactSidebar: Boolean },
          template:
            '<div data-testid="app-layout" :data-compact-sidebar="compactSidebar"><slot name="header"/><slot/></div>',
        },
        DiffViewer: diffViewerStub,
      },
    },
  });
  await flushPromises();
  return { wrapper, router, repos };
}

describe("PrNewPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.sessionStorage.clear();
    vi.mocked(getPlatformCapabilities).mockImplementation(async (platform) =>
      platformCapabilities(platform),
    );
    vi.mocked(prBranches).mockResolvedValue({
      branches: ["main", "feature"],
      default_branch: "main",
    });
    vi.mocked(prCreate).mockResolvedValue({
      number: 51,
      detail: null,
      updated_fields: [],
      failures: [],
    });
    vi.mocked(prCreatePreview).mockResolvedValue(createPreview("Add feature"));
    vi.mocked(prLabels).mockResolvedValue([
      { name: "bug", color: "#d73a4a", description: "需要修复的问题" },
      { name: "feature", color: "b60205", description: "新功能" },
      { name: "frontend", color: null, description: null },
    ]);
    vi.mocked(prParticipantSuggestions).mockResolvedValue([
      { id: 1, login: "Alice", name: "Alice Zhang", avatar_url: "https://example.com/alice.png" },
      { id: 2, login: "Bob", name: "Bob", avatar_url: "" },
    ]);
    vi.mocked(repoList).mockResolvedValue({
      items: [repository("team/repo")],
      page: 1,
      total_pages: 1,
      total_count: 1,
    });
  });

  it("创建页使用紧凑侧栏布局", async () => {
    const { wrapper } = await mountPage();

    expect(wrapper.get('[data-testid="app-layout"]').attributes("data-compact-sidebar")).toBe(
      "true",
    );
  });

  it("创建同仓库 PR 后跳转现有详情页", async () => {
    const { wrapper, router } = await mountPage();
    await wrapper.get("input[placeholder='简要说明这次变更']").setValue("Add feature");
    await wrapper.get('[aria-label="Reviewers"]').trigger("click");
    await wrapper.get(".multi-select-option[data-value='Alice']").trigger("click");
    await wrapper.get(".multi-select-option[data-value='Bob']").trigger("click");
    await wrapper.get('input[placeholder="搜索Reviewers"]').trigger("keydown", { key: "Escape" });
    await wrapper.get('[aria-label="Assignees"]').trigger("click");
    await wrapper.get(".multi-select-option[data-value='Alice']").trigger("click");
    await wrapper.get('input[placeholder="搜索Assignees"]').trigger("keydown", { key: "Escape" });
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(prBranches).toHaveBeenCalledTimes(1);
    expect(prCreate).toHaveBeenCalledWith(
      "github",
      "team",
      "repo",
      expect.objectContaining({
        source_owner: "team",
        source_repo: "repo",
        source_branch: "feature",
        target_branch: "main",
        title: "Add feature",
        reviewers: ["Alice", "Bob"],
        assignees: ["Alice"],
      }),
    );
    expect(router.currentRoute.value.name).toBe("pr-detail");
    expect(router.currentRoute.value.params.number).toBe("51");
  });

  it("创建成功切换目标仓库时清理旧 Fork 上下文", async () => {
    const { wrapper, repos } = await mountPage(
      "github",
      [repository("old/fork", true, "old/upstream"), repository("target/repo")],
      "github",
      true,
    );
    repos.setForkContext(
      {
        upstreamFullName: "old/upstream",
        upstreamOwner: "old",
        forkOwner: "old",
        forkRepo: "fork",
      },
      "github",
    );

    await wrapper.get('[aria-label="目标仓库"]').trigger("click");
    await wrapper.get(".dropdown-option[data-value='target/repo']").trigger("click");
    await flushPromises();
    await wrapper.get("input[placeholder='简要说明这次变更']").setValue("Target change");
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(repos.activeRepos.github).toEqual({ owner: "target", repo: "repo" });
    expect(repos.forkContexts.github).toBeNull();
  });

  it("创建部分成功时通过 query 和会话暂存向详情页传递警告", async () => {
    vi.mocked(prCreate).mockResolvedValue({
      number: 52,
      detail: null,
      updated_fields: [],
      failures: [{ field: "reviewers", message: "部分评审者不存在" }],
    });
    const { wrapper, router } = await mountPage();
    await wrapper.get("input[placeholder='简要说明这次变更']").setValue("Partial success");
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(router.currentRoute.value.name).toBe("pr-detail");
    expect(router.currentRoute.value.query[PR_CREATE_WARNING_QUERY]).toBe("1");
    expect(readPrCreateWarnings("github", "team", "repo", 52)).toEqual(["部分评审者不存在"]);
  });

  it("GitHub 分支下拉可以展开并切换源分支和目标分支", async () => {
    const { wrapper } = await mountPage();
    const sourceSelect = wrapper.get('[aria-label="源分支"]');

    await sourceSelect.trigger("click");
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual([
      "main",
      "feature",
    ]);
    await wrapper.get(".dropdown-option[data-value='main']").trigger("click");
    expect(sourceSelect.text()).toContain("main");

    const targetSelect = wrapper.get('[aria-label="目标分支"]');
    await targetSelect.trigger("click");
    await wrapper.get(".dropdown-option[data-value='feature']").trigger("click");
    expect(targetSelect.text()).toContain("feature");
  });

  it("从目标仓库加载标签并将多选结果提交给创建接口", async () => {
    const { wrapper } = await mountPage();

    expect(prLabels).toHaveBeenCalledWith("github", "team", "repo");
    const labelsSelect = wrapper.get('[aria-label="Labels"]');
    await labelsSelect.trigger("click");
    await wrapper.get('input[placeholder="搜索标签"]').setValue("feature");
    await wrapper.get(".multi-select-option[data-value='feature']").trigger("click");
    await wrapper.get('input[placeholder="搜索标签"]').setValue("front");
    await wrapper.get(".multi-select-option[data-value='frontend']").trigger("click");
    await wrapper.get('input[placeholder="搜索标签"]').trigger("keydown", { key: "Escape" });

    await wrapper.get("input[placeholder='简要说明这次变更']").setValue("Labeled change");
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(prCreate).toHaveBeenCalledWith(
      "github",
      "team",
      "repo",
      expect.objectContaining({ labels: ["feature", "frontend"] }),
    );
  });

  it("切换目标仓库后清空选择并忽略迟到的旧标签请求", async () => {
    let resolveOld!: (value: PrLabel[]) => void;
    vi.mocked(prLabels).mockImplementation((_platform, owner) => {
      if (owner === "team") {
        return new Promise((resolve) => {
          resolveOld = resolve;
        });
      }
      return Promise.resolve([
        { name: "other-only", color: "0e8a16", description: "其他仓库标签" },
      ]);
    });
    const { wrapper } = await mountPage(
      "github",
      [repository("team/repo"), repository("other/repo")],
      "github",
      true,
    );

    const targetSelect = wrapper.get('[aria-label="目标仓库"]');
    await targetSelect.trigger("click");
    await wrapper.get(".dropdown-option[data-value='other/repo']").trigger("click");
    await flushPromises();

    expect(prLabels).toHaveBeenLastCalledWith("github", "other", "repo");
    await wrapper.get('[aria-label="Labels"]').trigger("click");
    expect(
      wrapper.findAll(".multi-select-option-copy > span").map((option) => option.text()),
    ).toEqual(["other-only"]);
    await wrapper.get(".multi-select-option[data-value='other-only']").trigger("click");
    await wrapper.get('input[placeholder="搜索标签"]').trigger("keydown", { key: "Escape" });

    resolveOld([{ name: "stale-label", color: null, description: null }]);
    await flushPromises();
    await wrapper.get('[aria-label="Labels"]').trigger("click");
    expect(wrapper.find(".multi-select-option[data-value='stale-label']").exists()).toBe(false);
    expect(wrapper.get('[aria-label="Labels"]').text()).toContain("other-only");
  });

  it("切换目标仓库后重新加载成员并忽略迟到的旧 Suggestions", async () => {
    let resolveOld!: (value: User[]) => void;
    vi.mocked(prParticipantSuggestions).mockImplementation((_platform, owner) => {
      if (owner === "team") {
        return new Promise((resolve) => {
          resolveOld = resolve;
        });
      }
      return Promise.resolve([
        { id: 3, login: "carol", name: "Carol", avatar_url: "https://example.com/carol.png" },
      ]);
    });
    const { wrapper } = await mountPage(
      "github",
      [repository("team/repo"), repository("other/repo")],
      "github",
      true,
    );

    await wrapper.get('[aria-label="目标仓库"]').trigger("click");
    await wrapper.get(".dropdown-option[data-value='other/repo']").trigger("click");
    await flushPromises();

    expect(prParticipantSuggestions).toHaveBeenLastCalledWith("github", "other", "repo");
    await wrapper.get('[aria-label="Reviewers"]').trigger("click");
    await wrapper.get(".multi-select-option[data-value='carol']").trigger("click");
    await wrapper.get('input[placeholder="搜索Reviewers"]').trigger("keydown", { key: "Escape" });

    resolveOld([{ id: 1, login: "stale-user", name: "Stale", avatar_url: "" }]);
    await flushPromises();
    await wrapper.get('[aria-label="Reviewers"]').trigger("click");
    expect(wrapper.find(".multi-select-option[data-value='stale-user']").exists()).toBe(false);
    expect(wrapper.get('[aria-label="Reviewers"]').text()).toContain("carol");
  });

  it("标签读取失败时显示错误但不阻止创建", async () => {
    vi.mocked(prLabels).mockRejectedValue(new Error("labels unavailable"));
    const { wrapper } = await mountPage();

    expect(wrapper.text()).toContain("labels unavailable");
    await wrapper.get("input[placeholder='简要说明这次变更']").setValue("No labels");
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(prCreate).toHaveBeenCalledWith(
      "github",
      "team",
      "repo",
      expect.objectContaining({ labels: [] }),
    );
  });

  it("仓库和分支下拉支持搜索", async () => {
    vi.mocked(prBranches).mockResolvedValue({
      branches: ["main", "feature", "release"],
      default_branch: "main",
    });
    const { wrapper } = await mountPage("github", [
      repository("team/repo"),
      repository("other/repo", true, "team/repo"),
    ]);

    const repositorySelect = wrapper.get('[aria-label="源仓库"]');
    await repositorySelect.trigger("click");
    await wrapper.get('input[placeholder="搜索仓库"]').setValue("other");
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual([
      "other/repo",
    ]);
    await wrapper.get(".dropdown-option[data-value='other/repo']").trigger("click");
    await flushPromises();

    const sourceBranchSelect = wrapper.get('[aria-label="源分支"]');
    await sourceBranchSelect.trigger("click");
    await wrapper.get('input[placeholder="搜索源分支"]').setValue("rel");
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual(["release"]);
    await wrapper.get(".dropdown-option[data-value='release']").trigger("click");

    const targetBranchSelect = wrapper.get('[aria-label="目标分支"]');
    await targetBranchSelect.trigger("click");
    await wrapper.get('input[placeholder="搜索目标分支"]').setValue("mai");
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual(["main"]);
  });

  it("切换源仓库时重新请求并替换目标分支", async () => {
    let targetRequestCount = 0;
    vi.mocked(prBranches).mockImplementation(async (_platform, owner) => {
      if (owner === "team") {
        targetRequestCount += 1;
        return targetRequestCount === 1
          ? { branches: ["main", "feature"], default_branch: "main" }
          : { branches: ["develop"], default_branch: "develop" };
      }
      return { branches: ["topic"], default_branch: "topic" };
    });
    const { wrapper } = await mountPage("github", [
      repository("team/repo"),
      repository("other/repo", true, "team/repo"),
    ]);

    expect(wrapper.get('[aria-label="目标分支"]').text()).toContain("main");
    await wrapper.get('[aria-label="源仓库"]').trigger("click");
    await wrapper.get(".dropdown-option[data-value='other/repo']").trigger("click");
    await flushPromises();

    expect(targetRequestCount).toBe(2);
    expect(prBranches).toHaveBeenCalledWith("github", "other", "repo");
    expect(wrapper.get('[aria-label="目标分支"]').text()).toContain("develop");
  });

  it("切换源仓库时保留仍然有效的已选目标分支", async () => {
    vi.mocked(prBranches).mockImplementation(async (_platform, owner) =>
      owner === "team"
        ? { branches: ["main", "feature", "release/1.2"], default_branch: "main" }
        : { branches: ["fork-topic"], default_branch: "fork-topic" },
    );
    const { wrapper } = await mountPage("github", [
      repository("team/repo"),
      repository("other/repo", true, "team/repo"),
    ]);

    const targetBranchSelect = wrapper.get('[aria-label="目标分支"]');
    await targetBranchSelect.trigger("click");
    await wrapper.get(".dropdown-option[data-value='release/1.2']").trigger("click");
    await flushPromises();

    await wrapper.get('[aria-label="源仓库"]').trigger("click");
    await wrapper.get(".dropdown-option[data-value='other/repo']").trigger("click");
    await flushPromises();

    expect(wrapper.get('[aria-label="目标分支"]').text()).toContain("release/1.2");
    expect(prCreatePreview).toHaveBeenLastCalledWith("github", "team", "repo", {
      source_owner: "other",
      source_repo: "repo",
      source_branch: "fork-topic",
      target_branch: "release/1.2",
    });
  });

  it("分支选择完成后展示提交列表和只读 Diff", async () => {
    const { wrapper } = await mountPage();

    expect(prCreatePreview).toHaveBeenCalledWith("github", "team", "repo", {
      source_owner: "team",
      source_repo: "repo",
      source_branch: "feature",
      target_branch: "main",
    });
    expect(wrapper.text()).toContain("Add feature");
    expect(wrapper.text()).toContain("12345678");

    await wrapper.get('[role="tab"][aria-selected="false"]').trigger("click");
    expect(wrapper.get('[data-testid="diff-preview"]').text()).toBe("1 files");
    expect(wrapper.getComponent(diffViewerStub).props("readOnly")).toBe(true);
  });

  it("预览被平台截断时显著警告但仍允许创建", async () => {
    const incompletePreview = createPreview(
      "Partial preview",
      "partial-sha",
      "src/partial.ts",
      true,
    );
    incompletePreview.commits = [];
    incompletePreview.diff.files = [];
    vi.mocked(prCreatePreview).mockResolvedValue(incompletePreview);
    const { wrapper } = await mountPage();

    expect(wrapper.get(".preview-warning").text()).toContain("预览不完整");
    expect(wrapper.get(".preview-warning").text()).toContain("不影响创建 PR");
    await wrapper.get("input[placeholder='简要说明这次变更']").setValue("Large change");
    expect(wrapper.get<HTMLButtonElement>("button[type='submit']").element.disabled).toBe(false);

    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(prCreate).toHaveBeenCalledOnce();
  });

  it("Compare 补页失败时展示可排障的降级提示且仍允许创建", async () => {
    const incompletePreview = createPreview(
      "Partial preview",
      "partial-sha",
      "src/partial.ts",
      true,
    );
    incompletePreview.incomplete_reasons = ["pagination_failed"];
    vi.mocked(prCreatePreview).mockResolvedValue(incompletePreview);
    const { wrapper } = await mountPage();

    expect(wrapper.get(".preview-warning").text()).toContain("后续分页加载失败");
    expect(wrapper.get(".preview-warning").text()).toContain("不影响创建 PR");
    await wrapper.get("input[placeholder='简要说明这次变更']").setValue("Large change");
    expect(wrapper.get<HTMLButtonElement>("button[type='submit']").element.disabled).toBe(false);
  });

  it("创建 PR 描述支持 Markdown 编辑和预览且提交原始文本", async () => {
    const { wrapper } = await mountPage();
    const markdown = "# 变更说明\n\n- 第一项\n- 第二项\n\n`code`";
    await wrapper.get('textarea[aria-label="Markdown 描述"]').setValue(markdown);

    const modeTabs = wrapper.get('[aria-label="Markdown 描述模式"]');
    await modeTabs.findAll("button")[1].trigger("click");

    expect(wrapper.get(".description-preview h1").text()).toBe("变更说明");
    expect(wrapper.findAll(".description-preview li")).toHaveLength(2);
    expect(wrapper.get(".description-preview code").text()).toBe("code");

    await modeTabs.findAll("button")[0].trigger("click");
    expect(
      wrapper.get<HTMLTextAreaElement>('textarea[aria-label="Markdown 描述"]').element.value,
    ).toBe(markdown);
    await wrapper.get("input[placeholder='简要说明这次变更']").setValue("Markdown change");
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(prCreate).toHaveBeenCalledWith(
      "github",
      "team",
      "repo",
      expect.objectContaining({ body: markdown }),
    );
  });

  it("Diff 可以按提交切换，并支持恢复全部提交视图", async () => {
    vi.mocked(prCreatePreview).mockImplementation(async (_platform, _owner, _repo, request) =>
      request.commit_sha
        ? createPreview("Commit-only", request.commit_sha, "src/commit.ts", true)
        : createPreview("All commits", "branch-sha", "src/all.ts"),
    );
    const { wrapper } = await mountPage();

    await wrapper.get('[role="tab"][aria-selected="false"]').trigger("click");
    const scopeSelect = wrapper.get('[aria-label="Diff 范围"]');
    await scopeSelect.trigger("click");
    await wrapper.get(".dropdown-option[data-value='branch-sha']").trigger("click");
    await flushPromises();

    expect(prCreatePreview).toHaveBeenLastCalledWith("github", "team", "repo", {
      source_owner: "team",
      source_repo: "repo",
      source_branch: "feature",
      target_branch: "main",
      commit_sha: "branch-sha",
    });
    expect(wrapper.getComponent(diffViewerStub).props("diff").files[0].filename).toBe(
      "src/commit.ts",
    );
    expect(wrapper.get(".preview-warning").text()).toContain("预览不完整");

    await scopeSelect.trigger("click");
    await wrapper.get(".dropdown-option[data-value='']").trigger("click");
    await flushPromises();
    expect(wrapper.getComponent(diffViewerStub).props("diff").files[0].filename).toBe("src/all.ts");
  });

  it("不完整提示只跟随当前选择的 Diff 范围", async () => {
    vi.mocked(prCreatePreview).mockImplementation(async (_platform, _owner, _repo, request) =>
      request.commit_sha
        ? createPreview("Complete commit", request.commit_sha, "src/commit.ts")
        : createPreview("Partial branch", "branch-sha", "src/all.ts", true),
    );
    const { wrapper } = await mountPage();

    expect(wrapper.find(".preview-warning").exists()).toBe(true);
    await wrapper.get('[role="tab"][aria-selected="false"]').trigger("click");
    const scopeSelect = wrapper.get('[aria-label="Diff 范围"]');
    await scopeSelect.trigger("click");
    await wrapper.get(".dropdown-option[data-value='branch-sha']").trigger("click");
    await flushPromises();

    expect(wrapper.find(".preview-warning").exists()).toBe(false);
  });

  it("切换分支后忽略迟到的旧预览请求", async () => {
    let resolveOld!: (value: PrCreatePreview) => void;
    vi.mocked(prBranches).mockResolvedValue({
      branches: ["main", "feature", "next"],
      default_branch: "main",
    });
    vi.mocked(prCreatePreview).mockImplementation((_platform, _owner, _repo, request) => {
      if (request.source_branch === "feature") {
        return new Promise((resolve) => {
          resolveOld = resolve;
        });
      }
      return Promise.resolve(createPreview("Newest preview", "next1234"));
    });
    const { wrapper } = await mountPage();

    const sourceSelect = wrapper.get('[aria-label="源分支"]');
    await sourceSelect.trigger("click");
    await wrapper.get(".dropdown-option[data-value='next']").trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("Newest preview");

    resolveOld(createPreview("Stale preview", "stale123"));
    await flushPromises();
    expect(wrapper.text()).toContain("Newest preview");
    expect(wrapper.text()).not.toContain("Stale preview");
  });

  it("查看上游仓库时默认使用 Fork 作为源仓库", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/pr/new/:platform", name: "pr-new", component: PrNewPage },
        { path: "/pr/:platform/:owner/:repo/:number", name: "pr-detail", component: {} },
      ],
    });
    await router.push({ name: "pr-new", params: { platform: "github" } });
    const auth = useAuthStore();
    auth.platforms.github.isLoggedIn = true;
    const repos = useRepoStore();
    repos.setActiveRepo("upstream", "repo");
    repos.reposCache.github = [repository("contributor/repo", true, "upstream/repo")];
    repos.setForkContext({
      upstreamFullName: "upstream/repo",
      upstreamOwner: "upstream",
      forkOwner: "contributor",
      forkRepo: "repo",
    });
    vi.mocked(prBranches).mockImplementation(async (_platform, owner) =>
      owner === "upstream"
        ? { branches: ["release"], default_branch: "develop" }
        : { branches: ["feature"], default_branch: "feature" },
    );

    const wrapper = mount(PrNewPage, {
      global: {
        plugins: [pinia, router],
        stubs: {
          AppLayout: {
            props: { compactSidebar: Boolean },
            template: '<div><slot name="header"/><slot/></div>',
          },
          DiffViewer: diffViewerStub,
        },
      },
    });
    await flushPromises();

    const targetSelect = wrapper.get('[aria-label="目标分支"]');
    await targetSelect.trigger("click");
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual([
      "develop",
      "release",
    ]);
    await targetSelect.trigger("click");

    const sourceSelect = wrapper.get('[aria-label="源分支"]');
    await sourceSelect.trigger("click");
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual(["feature"]);
    await sourceSelect.trigger("click");

    expect(prCreatePreview).toHaveBeenCalledWith("github", "upstream", "repo", {
      source_owner: "contributor",
      source_repo: "repo",
      source_branch: "feature",
      target_branch: "develop",
    });

    await wrapper.get("input[placeholder='简要说明这次变更']").setValue("Fork change");
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(prCreate).toHaveBeenCalledWith(
      "github",
      "upstream",
      "repo",
      expect.objectContaining({
        source_owner: "contributor",
        source_repo: "repo",
        source_branch: "feature",
        target_branch: "develop",
      }),
    );
  });

  it("Gitee 使用评审者和测试者文案且不显示 Draft 选项", async () => {
    const { wrapper } = await mountPage("gitee");

    expect(wrapper.get("h2").text()).toBe("创建 PR");
    expect(wrapper.text()).toContain("评审者");
    expect(wrapper.text()).toContain("测试者");
    expect(wrapper.text()).not.toContain("创建为 Draft");
  });

  it("GitLab 使用创建 MR 文案", async () => {
    const { wrapper } = await mountPage("gitlab");

    expect(wrapper.get("h2").text()).toBe("创建 MR");
    expect(wrapper.get("button[type='submit']").text()).toBe("创建 MR");
  });

  it("创建页按路由平台请求，避免使用过期的全局活动平台", async () => {
    const { wrapper } = await mountPage("gitee", [repository("t8y2/dbx")], "github");

    expect(wrapper.text()).toContain("目标仓库：t8y2/dbx");
    expect(wrapper.find('[aria-label="目标仓库"]').exists()).toBe(false);
    expect(getPlatformCapabilities).toHaveBeenCalledWith("gitee");
    expect(prBranches).toHaveBeenCalledWith("gitee", "t8y2", "dbx");
    expect(prCreatePreview).toHaveBeenCalledWith("gitee", "t8y2", "dbx", expect.any(Object));
    expect(
      vi.mocked(prBranches).mock.calls.every(([requestPlatform]) => requestPlatform === "gitee"),
    ).toBe(true);
  });

  it("源仓库搜索只展示目标仓库及其同平台 Fork", async () => {
    const { wrapper, repos } = await mountPage(
      "gitee",
      [
        repository("t8y2/dbx"),
        repository("gitee-only/dbx", true, "t8y2/dbx"),
        repository("unrelated/tools"),
      ],
      "github",
    );
    repos.reposCache.github = [repository("github-only/project")];
    await flushPromises();

    const repositorySelect = wrapper.get('[aria-label="源仓库"]');
    await repositorySelect.trigger("click");

    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual([
      "t8y2/dbx",
      "gitee-only/dbx",
    ]);
    await wrapper.get('input[placeholder="搜索仓库"]').setValue("unrelated");
    expect(wrapper.findAll(".dropdown-option")).toHaveLength(0);
    await wrapper.get('input[placeholder="搜索仓库"]').setValue("github-only");
    expect(wrapper.findAll(".dropdown-option")).toHaveLength(0);
  });

  it("全局创建页可以搜索并切换目标仓库", async () => {
    const { wrapper } = await mountPage(
      "github",
      [repository("team/repo"), repository("other/repo", true, "team/repo")],
      "github",
      true,
    );

    const targetSelect = wrapper.get('[aria-label="目标仓库"]');
    await targetSelect.trigger("click");
    await wrapper.get('input[placeholder="搜索目标仓库"]').setValue("other");
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual([
      "other/repo",
    ]);
    await wrapper.get(".dropdown-option[data-value='other/repo']").trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("目标仓库：other/repo");
    expect(prBranches).toHaveBeenCalledWith("github", "other", "repo");
    expect(prCreatePreview).toHaveBeenLastCalledWith("github", "other", "repo", {
      source_owner: "other",
      source_repo: "repo",
      source_branch: "feature",
      target_branch: "main",
    });
  });

  it("全局创建页只加载首屏并按需加载更多仓库", async () => {
    vi.mocked(repoList).mockImplementation(async (_platform, page = 1) => ({
      items: [repository(page === 1 ? "first/repo" : "second/repo")],
      page,
      total_pages: 3,
      total_count: 3,
    }));

    const { wrapper } = await mountPage("github", [], "github", true);

    expect(repoList).toHaveBeenCalledTimes(1);
    expect(repoList).toHaveBeenLastCalledWith("github", 1);
    const targetSelect = wrapper.get('[aria-label="目标仓库"]');
    await targetSelect.trigger("click");
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toContain(
      "first/repo",
    );

    await wrapper.get(".dropdown-load-more").trigger("click");
    await flushPromises();

    expect(repoList).toHaveBeenCalledTimes(2);
    expect(repoList).toHaveBeenLastCalledWith("github", 2);
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toContain(
      "second/repo",
    );
  });

  it("全局创建页将 Fork 的上游仓库加入目标仓库候选", async () => {
    const { wrapper } = await mountPage(
      "github",
      [repository("contributor/project", true, "upstream/project")],
      "github",
      true,
    );

    const targetSelect = wrapper.get('[aria-label="目标仓库"]');
    await targetSelect.trigger("click");
    await wrapper.get('input[placeholder="搜索目标仓库"]').setValue("upstream");
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual([
      "upstream/project",
    ]);
    await wrapper.get(".dropdown-option[data-value='upstream/project']").trigger("click");
    await flushPromises();

    const sourceSelect = wrapper.get('[aria-label="源仓库"]');
    await sourceSelect.trigger("click");
    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual([
      "upstream/project",
      "contributor/project",
    ]);
  });

  it("平台不支持创建 PR / MR 时显示明确提示", async () => {
    vi.mocked(getPlatformCapabilities).mockResolvedValue({
      ...platformCapabilities("github"),
      supports_pr_creation: false,
    });
    const { wrapper } = await mountPage();

    expect(wrapper.get('[role="status"]').text()).toBe("当前平台不支持创建 PR。");
    expect(wrapper.get<HTMLButtonElement>("button[type='submit']").element.disabled).toBe(true);
  });
});
