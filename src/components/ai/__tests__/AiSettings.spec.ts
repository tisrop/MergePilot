import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiSettings from "../AiSettings.vue";
import { aiListModels } from "@/api";

vi.mock("@/api", () => ({
  aiGetConfig: vi.fn().mockRejectedValue(new Error("no config")),
  aiSaveConfig: vi.fn(),
  aiSaveApiKey: vi.fn(),
  aiTestConnection: vi.fn(),
  aiListModels: vi.fn(),
}));

describe("AiSettings", () => {
  beforeEach(() => {
    vi.mocked(aiListModels).mockResolvedValue([`gpt<img src=x onerror="alert(1)">model`]);
  });

  it("将恶意模型 ID 作为纯文本渲染", async () => {
    const wrapper = mount(AiSettings);
    const fetchButton = wrapper.findAll("button").find((button) => button.text() === "获取模型");
    expect(fetchButton).toBeDefined();
    await fetchButton!.trigger("click");
    await flushPromises();
    const input = wrapper.get(".model-input-wrap input");
    await input.trigger("focus");
    await input.setValue("gpt");

    expect(wrapper.get(".model-item").text()).toContain('<img src=x onerror="alert(1)">');
    expect(wrapper.find(".model-item img").exists()).toBe(false);
    expect(wrapper.get(".model-item").element.querySelector("[onerror]")).toBeNull();
  });
});
