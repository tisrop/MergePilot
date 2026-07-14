import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  checkForUpdates,
  copySupportInfo,
  downloadAndInstallUpdate,
  downloadAndReplacePortableUpdate,
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
  downloadAndReplacePortableUpdate: vi.fn(),
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
    vi.mocked(downloadAndReplacePortableUpdate).mockReset();
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
      update_mode: "installer",
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
      update_mode: "installer",
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
      update_mode: "installer",
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
      update_mode: "installer",
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

  it("Windows 便携版验签后自动替换 EXE 并失败回滚", async () => {
    vi.stubGlobal("crypto", { randomUUID: vi.fn(() => "portable-page") });
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
      update_mode: "portable",
    });
    vi.mocked(downloadAndReplacePortableUpdate).mockResolvedValue(undefined);
    const wrapper = mountPage();

    await wrapper.get("button.check-update-button").trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("下载并验签后");
    expect(wrapper.text()).toContain("自动退出、替换当前 EXE 并重新启动");
    expect(wrapper.get("button.install-update-button").text()).toBe("下载并自动更新");

    await wrapper.get("button.install-update-button").trigger("click");
    expect(wrapper.text()).toContain("替换失败时会自动恢复并启动旧版本");
    expect(downloadAndReplacePortableUpdate).not.toHaveBeenCalled();

    await wrapper.get("button.install-update-button").trigger("click");
    await flushPromises();

    expect(downloadAndReplacePortableUpdate).toHaveBeenCalledWith("portable-page", "0.4.0");
    expect(downloadAndInstallUpdate).not.toHaveBeenCalled();
    expect(restartAfterUpdate).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("新版 EXE 已通过签名验证");
    expect(wrapper.text()).toContain("正在退出并启动新版");
    expect(wrapper.find("button.install-update-button").exists()).toBe(false);
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
      update_mode: "installer",
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

  it("离开设置页后继续安装并在返回时保留完成状态", async () => {
    vi.stubGlobal("crypto", { randomUUID: vi.fn(() => "update-across-pages") });
    let resolveListener!: (unlisten: () => void) => void;
    const delayedListener = new Promise<() => void>((resolve) => {
      resolveListener = resolve;
    });
    const unlisten = vi.fn();
    vi.mocked(listenToUpdateProgress).mockReturnValue(delayedListener);

    let resolveDownload!: () => void;
    const pendingDownload = new Promise<void>((resolve) => {
      resolveDownload = resolve;
    });
    vi.mocked(downloadAndInstallUpdate).mockReturnValue(pendingDownload);
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
      update_mode: "installer",
    });
    const firstPage = mountPage();

    await firstPage.get("button.check-update-button").trigger("click");
    await flushPromises();
    await firstPage.get("button.install-update-button").trigger("click");
    await firstPage.get("button.install-update-button").trigger("click");
    expect(listenToUpdateProgress).toHaveBeenCalledOnce();

    firstPage.unmount();
    resolveListener(unlisten);
    await flushPromises();

    expect(downloadAndInstallUpdate).toHaveBeenCalledWith("update-across-pages", "0.4.0");
    expect(unlisten).not.toHaveBeenCalled();

    resolveDownload();
    await flushPromises();
    expect(unlisten).toHaveBeenCalledOnce();

    const secondPage = mountPage();
    await flushPromises();
    expect(secondPage.text()).toContain("更新已安装，重启应用后生效");
    expect(secondPage.get("button.install-update-button").text()).toBe("重启完成更新");
    expect(downloadAndInstallUpdate).toHaveBeenCalledOnce();
  });

  it("更新安装完成后由用户主动确认重启", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
      update_mode: "installer",
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

  it("重启期间阻止重复触发并在失败后允许重试", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
      update_mode: "installer",
    });
    vi.mocked(downloadAndInstallUpdate).mockResolvedValue(undefined);
    let rejectRestart!: (reason?: unknown) => void;
    const pendingRestart = new Promise<void>((_resolve, reject) => {
      rejectRestart = reject;
    });
    vi.mocked(restartAfterUpdate)
      .mockReturnValueOnce(pendingRestart)
      .mockResolvedValueOnce(undefined);
    const wrapper = mountPage();

    await wrapper.get("button.check-update-button").trigger("click");
    await flushPromises();
    await wrapper.get("button.install-update-button").trigger("click");
    await wrapper.get("button.install-update-button").trigger("click");
    await flushPromises();

    const restartButton = wrapper.get<HTMLButtonElement>("button.install-update-button");
    void restartButton.trigger("click");
    void restartButton.trigger("click");
    await wrapper.vm.$nextTick();

    expect(restartAfterUpdate).toHaveBeenCalledOnce();
    expect(restartButton.element.disabled).toBe(true);
    expect(restartButton.attributes("aria-busy")).toBe("true");
    expect(restartButton.text()).toBe("正在重启...");

    rejectRestart("应用重启暂不可用");
    await flushPromises();

    expect(wrapper.get(".support-status.error").text()).toContain("应用重启暂不可用");
    expect(restartButton.element.disabled).toBe(false);
    expect(restartButton.attributes("aria-busy")).toBe("false");
    expect(restartButton.text()).toBe("重启完成更新");

    await restartButton.trigger("click");
    await flushPromises();
    expect(restartAfterUpdate).toHaveBeenCalledTimes(2);
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
      update_mode: "installer",
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
