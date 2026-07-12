import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import ReviewForm from "../ReviewForm.vue";

vi.mock("@/api", () => ({ reviewSubmit: vi.fn() }));

const props = { owner: "team", repo: "repo", prNumber: 1 };

describe("ReviewForm", () => {
  it.each(["gitlab", "gitee"] as const)("%s 只展示评论操作", (platform) => {
    const wrapper = mount(ReviewForm, { props: { ...props, platform } });
    const labels = wrapper.findAll(".event-select button").map((button) => button.text());
    expect(labels).toEqual(["评论"]);
  });

  it("GitHub 展示全部评审操作", () => {
    const wrapper = mount(ReviewForm, { props: { ...props, platform: "github" } });
    expect(wrapper.findAll(".event-select button").map((button) => button.text())).toEqual([
      "评论",
      "批准",
      "请求修改",
    ]);
  });
});
