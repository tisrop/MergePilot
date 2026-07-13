import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiReviewPanel from "../AiReviewPanel.vue";
import { aiReviewCancel, aiReviewStream } from "@/api";

type EventCallback = (event: { payload: unknown }) => void;
const listeners = new Map<string, EventCallback[]>();
const unlisteners: ReturnType<typeof vi.fn>[] = [];

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, callback: EventCallback) => {
    listeners.set(event, [...(listeners.get(event) ?? []), callback]);
    const unlisten = vi.fn();
    unlisteners.push(unlisten);
    return unlisten;
  }),
}));

vi.mock("@/api", () => ({
  aiReview: vi.fn(),
  aiReviewStream: vi.fn().mockResolvedValue(undefined),
  aiReviewCancel: vi.fn().mockResolvedValue(undefined),
}));

function latestListener(event: string) {
  return listeners.get(event)?.at(-1);
}

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
    vi.clearAllMocks();
    listeners.clear();
    unlisteners.length = 0;
    vi.mocked(aiReviewStream).mockResolvedValue(undefined);
    vi.mocked(aiReviewCancel).mockResolvedValue(undefined);
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

    latestListener("ai-review-chunk")?.({
      payload: { request_id: "another-request", payload: "不应显示" },
    });
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).not.toContain("不应显示");

    latestListener("ai-review-chunk")?.({
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

  it("重新评审后忽略旧监听器交错到达的 chunk、done 和 error", async () => {
    const wrapper = mountPanel();
    const button = wrapper.get("button.btn-primary");

    await button.trigger("click");
    await flushPromises();
    const oldChunk = latestListener("ai-review-chunk");
    const oldDone = latestListener("ai-review-done");
    const oldError = latestListener("ai-review-error");

    await button.trigger("click");
    await flushPromises();
    oldChunk?.({ payload: { request_id: "request-1", payload: "旧响应" } });
    oldDone?.({
      payload: {
        request_id: "request-1",
        payload: { summary: "旧结果", suggestions: [] },
      },
    });
    oldError?.({ payload: { request_id: "request-1", payload: "旧错误" } });
    await wrapper.vm.$nextTick();

    expect(wrapper.text()).not.toContain("旧响应");
    expect(wrapper.text()).not.toContain("旧结果");
    expect(wrapper.text()).not.toContain("旧错误");
    expect(wrapper.text()).toContain("重新评审");
  });

  it("卸载时取消当前请求并解除全部事件监听", async () => {
    const wrapper = mountPanel();
    await wrapper.get("button.btn-primary").trigger("click");
    await flushPromises();
    expect(unlisteners).toHaveLength(3);

    wrapper.unmount();
    await flushPromises();

    expect(aiReviewCancel).toHaveBeenCalledWith("request-1");
    expect(unlisteners.every((unlisten) => unlisten.mock.calls.length === 1)).toBe(true);
  });

  it("取消请求失败时不向用户显示错误", async () => {
    vi.mocked(aiReviewCancel).mockRejectedValueOnce(new Error("cancel failed"));
    const wrapper = mountPanel();
    const button = wrapper.get("button.btn-primary");

    await button.trigger("click");
    await flushPromises();
    await button.trigger("click");
    await flushPromises();

    expect(aiReviewStream).toHaveBeenCalledTimes(2);
    expect(wrapper.find(".error-box").exists()).toBe(false);
  });
});
