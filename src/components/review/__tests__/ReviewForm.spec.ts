import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ReviewForm from "../ReviewForm.vue";
import { getPlatformCapabilities, reviewSubmit } from "@/api";
import type { Platform, PlatformCapabilities } from "@/types";

vi.mock("@/api", () => ({ reviewSubmit: vi.fn(), getPlatformCapabilities: vi.fn() }));

const props = { owner: "team", repo: "repo", prNumber: 1 };
function capabilities(platform: Platform): PlatformCapabilities {
  return {
    platform,
    review_events: platform === "github" ? ["comment", "approve", "request_changes"] : ["comment"],
    merge_strategies: platform === "gitlab" ? ["merge", "squash"] : ["merge", "squash", "rebase"],
    supports_fork_context: true,
    supports_issue_auto_close: true,
    supports_compare_diff: true,
    supports_review_thread_resolution: false,
    supports_remote_file_viewed_state: false,
    supports_pr_title_body_edit: true,
    supports_pr_draft_toggle: true,
    supports_pr_reviewer_management: true,
    supports_pr_assignee_management: true,
    supports_pr_label_management: true,
    supports_pr_milestone_management: true,
  };
}

async function mountForm(platform: Platform) {
  vi.mocked(getPlatformCapabilities).mockImplementation(async (candidate) =>
    capabilities(candidate),
  );
  const wrapper = mount(ReviewForm, { props: { ...props, platform } });
  await flushPromises();
  return wrapper;
}

describe("ReviewForm", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it.each(["gitlab", "gitee"] as const)("%s 禁用不支持的批准和请求修改操作", async (platform) => {
    const wrapper = await mountForm(platform);
    const buttons = wrapper.findAll(".event-select button");
    expect(buttons.map((button) => button.text())).toEqual(["评论", "批准", "请求修改"]);
    expect(buttons.map((button) => button.attributes("disabled") !== undefined)).toEqual([
      false,
      true,
      true,
    ]);
    expect(buttons[1].attributes("title")).toBe("当前平台不支持此评审操作");
  });

  it("GitHub 启用全部评审操作", async () => {
    const wrapper = await mountForm("github");
    expect(
      wrapper
        .findAll(".event-select button")
        .every((button) => button.attributes("disabled") === undefined),
    ).toBe(true);
  });

  it("从 GitHub 切换到 GitLab 时重置不支持的评审事件", async () => {
    vi.mocked(reviewSubmit).mockResolvedValue({
      id: 1,
      body: "切换平台后的评论",
      state: "commented",
      author: { id: 1, login: "user", name: "User", avatar_url: "" },
      submitted_at: "",
    });
    const wrapper = await mountForm("github");
    await wrapper.findAll(".event-select button")[1].trigger("click");
    await wrapper.setProps({ platform: "gitlab" });
    await flushPromises();
    await wrapper.get("textarea").setValue("切换平台后的评论");
    await wrapper.get(".btn-primary").trigger("click");
    await flushPromises();

    expect(reviewSubmit).toHaveBeenCalledWith(
      "gitlab",
      "team",
      "repo",
      1,
      "切换平台后的评论",
      "comment",
      [],
    );
  });

  it("提交前提示未查看文件和未解决线程", async () => {
    vi.mocked(reviewSubmit).mockResolvedValue({
      id: 1,
      body: "评审意见",
      state: "commented",
      author: { id: 1, login: "user", name: "User", avatar_url: "" },
      submitted_at: "",
    });
    const wrapper = mount(ReviewForm, {
      props: {
        ...props,
        platform: "github",
        unviewedFileCount: 2,
        unresolvedThreadCount: 1,
      },
    });
    await flushPromises();
    await wrapper.get("textarea").setValue("评审意见");
    await wrapper.get(".btn-primary").trigger("click");

    expect(reviewSubmit).not.toHaveBeenCalled();
    expect(wrapper.get('[role="alert"]').text()).toContain("2 个文件未查看");
    expect(wrapper.get('[role="alert"]').text()).toContain("1 个未解决线程");

    await wrapper.get(".btn-primary").trigger("click");
    await flushPromises();
    expect(reviewSubmit).toHaveBeenCalledTimes(1);
  });
});
