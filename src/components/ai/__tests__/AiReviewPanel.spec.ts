import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiReviewPanel from "../AiReviewPanel.vue";
import { aiReviewCancel, aiReviewStream } from "@/api";

type EventCallback = (event: { payload: unknown }) => void;
const listeners = new Map<string, EventCallback>();
const unlisteners = new Map<string, ReturnType<typeof vi.fn>>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, callback: EventCallback) => {
    listeners.set(event, callback);
    const unlisten = vi.fn();
    unlisteners.set(event, unlisten);
    return unlisten;
  }),
}));

vi.mock("@/api", () => ({
  aiReview: vi.fn(),
  aiReviewStream: vi.fn().mockResolvedValue(undefined),
  aiReviewCancel: vi.fn().mockResolvedValue(undefined),
}));

function mountPanel() {
  return mount(AiReviewPanel, {
    props: {
      platform: "github",
      owner: "octocat",
      repo: "hello-world",
      prNumber: 42,
      diff: "+changed",
      context: null,
    },
    global: {
      stubs: {
        AppSelect: true,
        AiSuggestionCard: true,
      },
    },
  });
}

describe("AiReviewPanel", () => {
  beforeEach(() => {
    listeners.clear();
    unlisteners.clear();
    vi.stubGlobal("crypto", {
      randomUUID: vi.fn().mockReturnValueOnce("request-1").mockReturnValueOnce("request-2"),
    });
  });

  it("只消费当前请求事件，并在重新评审和卸载时取消请求", async () => {
    const wrapper = mountPanel();
    const button = wrapper.get("button.btn-primary");

    await button.trigger("click");
    await flushPromises();
    expect(aiReviewStream).toHaveBeenCalledWith(
      "request-1",
      expect.objectContaining({ diff: "+changed" }),
    );

    listeners.get("ai-review-chunk")?.({
      payload: { request_id: "another-request", payload: "不应显示" },
    });
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).not.toContain("不应显示");

    listeners.get("ai-review-chunk")?.({
      payload: { request_id: "request-1", payload: "当前响应" },
    });
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).toContain("当前响应");

    await button.trigger("click");
    await flushPromises();
    expect(aiReviewCancel).toHaveBeenCalledWith("request-1");
    expect(aiReviewStream).toHaveBeenLastCalledWith(
      "request-2",
      expect.objectContaining({ diff: "+changed" }),
    );

    wrapper.unmount();
    await flushPromises();
    expect(aiReviewCancel).toHaveBeenCalledWith("request-2");
  });
});
