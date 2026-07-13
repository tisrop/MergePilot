import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  checkForUpdates,
  copySupportInfo,
  downloadAndInstallUpdate,
  getAppVersion,
  listenToUpdateProgress,
  restartAfterUpdate,
} from "@/api";
import { useAuthStore } from "@/stores/useAuthStore";
import SettingsPage from "../SettingsPage.vue";

const storage = new Map<string, string>();

vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

vi.mock("@/api", () => ({
  copySupportInfo: vi.fn(),
  getAppVersion: vi.fn(),
  checkForUpdates: vi.fn(),
  downloadAndInstallUpdate: vi.fn(),
  listenToUpdateProgress: vi.fn(),
  restartAfterUpdate: vi.fn(),
}));

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
    vi.restoreAllMocks();
    storage.clear();
    storage.set("mergepilot:auto-update-check", "false");
    setActivePinia(createPinia());
    vi.mocked(copySupportInfo).mockReset();
    vi.mocked(getAppVersion).mockResolvedValue("0.3.0");
    vi.mocked(checkForUpdates).mockReset();
    vi.mocked(downloadAndInstallUpdate).mockReset();
    vi.mocked(listenToUpdateProgress).mockReset();
    vi.mocked(listenToUpdateProgress).mockResolvedValue(() => undefined);
    vi.mocked(restartAfterUpdate).mockReset();
  });

  it("使用当前平台获取后端脱敏文本并复制", async () => {
    vi.mocked(copySupportInfo).mockResolvedValue(undefined);
    const wrapper = mountPage();
    useAuthStore().setActivePlatform("gitlab");

    await wrapper.get("button.copy-support-button").trigger("click");
    await flushPromises();

    expect(copySupportInfo).toHaveBeenCalledWith("gitlab");
    expect(wrapper.get(".support-status").text()).toContain("诊断信息已复制");
    expect(
      wrapper.findAll(".privacy-note").some((node) => node.text().includes("不包含 Token")),
    ).toBe(true);
  });

  it("剪贴板拒绝时显示可重试的中文错误且恢复按钮", async () => {
    vi.mocked(copySupportInfo).mockRejectedValue(new Error("clipboard denied"));
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

  it("后端失败时显示错误并恢复按钮", async () => {
    vi.mocked(copySupportInfo).mockRejectedValue("诊断信息暂不可用");
    const wrapper = mountPage();

    await wrapper.get("button.copy-support-button").trigger("click");
    await flushPromises();

    expect(copySupportInfo).toHaveBeenCalledOnce();
    expect(wrapper.get(".support-status.error").text()).toContain("诊断信息暂不可用");
  });
  it("显示当前版本并提示已是最新版本", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: false,
      version: null,
      notes: null,
      published_at: null,
    });
    const wrapper = mountPage();
    await flushPromises();

    expect(getAppVersion).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("当前版本：v0.3.0");

    await wrapper.get("button.check-update-button").trigger("click");
    await flushPromises();

    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("当前已是最新版本");
  });

  it("将远端版本说明按不可信文本渲染", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: true,
      version: "0.4.0",
      notes: "<script>危险说明</script>",
      published_at: "2026-07-13",
    });
    const wrapper = mountPage();

    await wrapper.get("button.check-update-button").trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("发现新版本 v0.4.0");
    expect(wrapper.get(".update-notes").text()).toBe("<script>危险说明</script>");
    expect(wrapper.find(".update-notes script").exists()).toBe(false);
  });

  it("检查更新失败后恢复按钮并允许重试", async () => {
    vi.mocked(checkForUpdates).mockRejectedValueOnce("签名验证失败").mockResolvedValueOnce({
      current_version: "0.3.0",
      available: false,
      version: null,
      notes: null,
      published_at: null,
    });
    const wrapper = mountPage();
    const button = wrapper.get<HTMLButtonElement>("button.check-update-button");

    await button.trigger("click");
    await flushPromises();

    expect(wrapper.get(".support-status.error").text()).toContain("签名验证失败");
    expect(button.element.disabled).toBe(false);
    expect(button.text()).toBe("检查更新");

    await button.trigger("click");
    await flushPromises();

    expect(checkForUpdates).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).toContain("当前已是最新版本");
  });

  it("要求二次确认后才下载安装更新", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
    });
    vi.mocked(downloadAndInstallUpdate).mockResolvedValue(undefined);
    const wrapper = mountPage();

    await wrapper.get("button.check-update-button").trigger("click");
    await flushPromises();
    await wrapper.get("button.install-update-button").trigger("click");

    expect(downloadAndInstallUpdate).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("安装前请保存工作");

    await wrapper.get("button.install-update-button").trigger("click");
    await flushPromises();

    expect(downloadAndInstallUpdate).toHaveBeenCalledWith(expect.any(String), "0.4.0");
    expect(wrapper.text()).toContain("更新已安装，重启应用后生效");
  });

  it("下载失败后清理进度监听并允许重试", async () => {
    vi.stubGlobal("crypto", { randomUUID: vi.fn(() => "update-current") });
    let progressCallback:
      | ((event: {
          request_id: string;
          downloaded: number;
          total: number | null;
          phase: "downloading" | "installing";
        }) => void)
      | null = null;
    vi.mocked(listenToUpdateProgress).mockImplementation(async (callback) => {
      progressCallback = callback;
      return () => undefined;
    });
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
    });
    vi.mocked(downloadAndInstallUpdate).mockImplementation(async () => {
      progressCallback?.({ request_id: "old", downloaded: 90, total: 100, phase: "downloading" });
      progressCallback?.({
        request_id: "update-current",
        downloaded: 25,
        total: 100,
        phase: "downloading",
      });
      await Promise.resolve();
      throw new Error("download failed");
    });
    const wrapper = mountPage();

    await wrapper.get("button.check-update-button").trigger("click");
    await flushPromises();
    await wrapper.get("button.install-update-button").trigger("click");
    await wrapper.get("button.install-update-button").trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("Error: download failed");
    expect(wrapper.get<HTMLButtonElement>("button.install-update-button").element.disabled).toBe(
      false,
    );
  });

  it("更新安装完成后由用户主动确认重启", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
    });
    vi.mocked(downloadAndInstallUpdate).mockResolvedValue(undefined);
    vi.mocked(restartAfterUpdate).mockResolvedValue(undefined);
    const wrapper = mountPage();

    await wrapper.get("button.check-update-button").trigger("click");
    await flushPromises();
    await wrapper.get("button.install-update-button").trigger("click");
    await wrapper.get("button.install-update-button").trigger("click");
    await flushPromises();
    await wrapper.get("button.install-update-button").trigger("click");

    expect(restartAfterUpdate).toHaveBeenCalledOnce();
  });

  it("启用后启动时最多每天后台检查一次", async () => {
    storage.set("mergepilot:auto-update-check", "true");
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: false,
      version: null,
      notes: null,
      published_at: null,
    });

    const first = mountPage();
    await flushPromises();
    first.unmount();
    const second = mountPage();
    await flushPromises();

    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(storage.get("mergepilot:last-update-check")).toBeTruthy();
    second.unmount();
  });

  it("后台检查失败不显示阻断错误", async () => {
    storage.set("mergepilot:auto-update-check", "true");
    vi.mocked(checkForUpdates).mockRejectedValue("network unavailable");
    const wrapper = mountPage();
    await flushPromises();

    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(wrapper.find(".support-status.error").exists()).toBe(false);
  });

  it("本地存储不可用时仍可挂载并后台检查更新", async () => {
    vi.spyOn(localStorage, "getItem").mockImplementation((key: string) => {
      if (["mergepilot:auto-update-check", "mergepilot:last-update-check"].includes(key)) {
        throw new DOMException("storage denied", "SecurityError");
      }
      return storage.get(key) ?? null;
    });
    vi.spyOn(localStorage, "setItem").mockImplementation((key: string, value: string) => {
      if (["mergepilot:auto-update-check", "mergepilot:last-update-check"].includes(key)) {
        throw new DOMException("storage denied", "SecurityError");
      }
      storage.set(key, value);
    });
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: false,
      version: null,
      notes: null,
      published_at: null,
    });

    const wrapper = mountPage();
    await flushPromises();

    expect(wrapper.text()).toContain("应用更新");
    expect(checkForUpdates).toHaveBeenCalledOnce();
  });

  it("记录检查时间失败时不阻断手动检查", async () => {
    vi.spyOn(localStorage, "setItem").mockImplementation(() => {
      throw new DOMException("storage denied", "SecurityError");
    });
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: false,
      version: null,
      notes: null,
      published_at: null,
    });
    const wrapper = mountPage();

    await wrapper.get("button.check-update-button").trigger("click");
    await flushPromises();

    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("当前已是最新版本");
  });

  it("允许关闭自动检查且不记录敏感网络数据", async () => {
    storage.set("mergepilot:auto-update-check", "true");
    storage.set("mergepilot:last-update-check", String(Date.now()));
    const wrapper = mountPage();
    const toggle = wrapper.get<HTMLInputElement>('input[aria-label="每日自动检查更新"]');

    await toggle.setValue(false);

    expect(storage.get("mergepilot:auto-update-check")).toBe("false");
    expect([...storage.keys()].sort()).toEqual([
      "mergepilot:auto-update-check",
      "mergepilot:last-update-check",
    ]);
  });
});
