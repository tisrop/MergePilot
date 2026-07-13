import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getSupportInfo } from "@/api";
import { useAuthStore } from "@/stores/useAuthStore";
import SettingsPage from "../SettingsPage.vue";

const storage = new Map<string, string>();
const writeText = vi.fn();

vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});
vi.stubGlobal("navigator", { clipboard: { writeText } });

vi.mock("@/api", () => ({ getSupportInfo: vi.fn() }));

const supportInfo = {
  app_version: "0.3.0",
  operating_system: "macos",
  architecture: "aarch64",
  current_platform: "GitLab",
  platform_endpoint: "自托管（地址已隐藏）",
  credential_storage: "系统 Keyring",
  ai_configured: true,
  ai_endpoint: "自托管（地址已隐藏）",
  local_cache_available: true,
  formatted: "MergePilot 0.3.0\n当前平台：GitLab\n平台服务：自托管（地址已隐藏）",
};

function mountPage() {
  return mount(SettingsPage, {
    global: {
      stubs: {
        AppLayout: { template: "<main><slot name='header' /><slot /></main>" },
        AiSettings: true,
      },
    },
  });
}

describe("SettingsPage 诊断信息", () => {
  beforeEach(() => {
    storage.clear();
    setActivePinia(createPinia());
    writeText.mockReset();
    vi.mocked(getSupportInfo).mockReset();
  });

  it("使用当前平台获取后端脱敏文本并复制", async () => {
    vi.mocked(getSupportInfo).mockResolvedValue(supportInfo);
    writeText.mockResolvedValue(undefined);
    const wrapper = mountPage();
    useAuthStore().setActivePlatform("gitlab");

    await wrapper.get("button.copy-support-button").trigger("click");
    await flushPromises();

    expect(getSupportInfo).toHaveBeenCalledWith("gitlab");
    expect(writeText).toHaveBeenCalledWith(supportInfo.formatted);
    expect(wrapper.get(".support-status").text()).toContain("诊断信息已复制");
    expect(wrapper.get(".privacy-note").text()).toContain("不包含 Token");
  });

  it("剪贴板拒绝时显示可重试的中文错误且恢复按钮", async () => {
    vi.mocked(getSupportInfo).mockResolvedValue(supportInfo);
    writeText.mockRejectedValue(new Error("clipboard denied"));
    const wrapper = mountPage();

    await wrapper.get("button.copy-support-button").trigger("click");
    await flushPromises();

    expect(wrapper.get(".support-status.error").text()).toContain(
      "复制失败：Error: clipboard denied",
    );
    expect(wrapper.get<HTMLButtonElement>("button.copy-support-button").element.disabled).toBe(
      false,
    );
    expect(wrapper.get("button.copy-support-button").text()).toBe("复制诊断信息");
  });

  it("后端失败时不会向剪贴板写入内容", async () => {
    vi.mocked(getSupportInfo).mockRejectedValue("诊断信息暂不可用");
    const wrapper = mountPage();

    await wrapper.get("button.copy-support-button").trigger("click");
    await flushPromises();

    expect(writeText).not.toHaveBeenCalled();
    expect(wrapper.get(".support-status.error").text()).toContain("诊断信息暂不可用");
  });
});
