import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  checkForUpdates,
  copyRecentErrorLogs,
  copySupportInfo,
  downloadAndInstallUpdate,
  openExternalUrl,
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
  copyRecentErrorLogs: vi.fn(),
  copySupportInfo: vi.fn(),
  getAppVersion: vi.fn(),
  checkForUpdates: vi.fn(),
  downloadAndInstallUpdate: vi.fn(),
  openExternalUrl: vi.fn(),
  listenToUpdateProgress: vi.fn(),
  restartAfterUpdate: vi.fn(),
}));

function mountPage() {
  return mount(SettingsPage, {
    global: {
      stubs: {
        AppLayout: { template: "<main><slot name='header' /><slot /></main>" },
        AiSettings: true,
        NotificationSettings: true,
      },
    },
  });
}

describe("SettingsPage 诊断信息", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    storage.clear();
    storage.set("mergebeacon:auto-update-check", "false");
    setActivePinia(createPinia());
    vi.mocked(copySupportInfo).mockReset();
    vi.mocked(copyRecentErrorLogs).mockReset();
    vi.mocked(getAppVersion).mockResolvedValue("0.3.0");
    vi.mocked(checkForUpdates).mockReset();
    vi.mocked(downloadAndInstallUpdate).mockReset();
    vi.mocked(openExternalUrl).mockReset();
    vi.mocked(listenToUpdateProgress).mockReset();
    vi.mocked(listenToUpdateProgress).mockResolvedValue(() => undefined);
    vi.mocked(restartAfterUpdate).mockReset();
  });

  it("持久化 Diff 同步滚动设置并在重新加载后恢复", async () => {
    const firstPage = mountPage();
    const firstToggle = firstPage.get<HTMLInputElement>('input[aria-label="同步 Diff 横向滚动"]');

    expect(firstToggle.element.checked).toBe(true);
    await firstToggle.setValue(false);
    expect(storage.get("mergebeacon:diff-sync-scroll")).toBe("false");
    firstPage.unmount();

    setActivePinia(createPinia());
    const secondPage = mountPage();
    expect(
      secondPage.get<HTMLInputElement>('input[aria-label="同步 Diff 横向滚动"]').element.checked,
    ).toBe(false);
  });

  it("分别持久化依赖关系与合并队列显示设置", async () => {
    const firstPage = mountPage();
    const dependencyToggle = firstPage.get<HTMLInputElement>('input[aria-label="显示依赖关系"]');
    const queueToggle = firstPage.get<HTMLInputElement>(
      'input[aria-label="显示 Merge Queue / Merge Train"]',
    );

    expect(dependencyToggle.element.checked).toBe(true);
    expect(queueToggle.element.checked).toBe(true);
    await queueToggle.setValue(false);
    await dependencyToggle.setValue(false);
    expect(storage.get("mergebeacon:pr-dependencies-visible")).toBe("false");
    expect(storage.get("mergebeacon:merge-queue-visible")).toBe("false");
    firstPage.unmount();

    setActivePinia(createPinia());
    const secondPage = mountPage();
    expect(
      secondPage.get<HTMLInputElement>('input[aria-label="显示依赖关系"]').element.checked,
    ).toBe(false);
    expect(
      secondPage.get<HTMLInputElement>('input[aria-label="显示 Merge Queue / Merge Train"]').element
        .checked,
    ).toBe(false);
  });

  it("关闭依赖关系后禁用合并队列开关并保留原偏好", async () => {
    const wrapper = mountPage();
    const dependencyToggle = wrapper.get<HTMLInputElement>('input[aria-label="显示依赖关系"]');
    const queueToggle = wrapper.get<HTMLInputElement>(
      'input[aria-label="显示 Merge Queue / Merge Train"]',
    );

    await dependencyToggle.setValue(false);

    expect(queueToggle.element.disabled).toBe(true);
    expect(queueToggle.element.checked).toBe(false);
    expect(wrapper.text()).toContain("需先开启依赖关系");
    expect(storage.get("mergebeacon:merge-queue-visible")).toBeUndefined();

    await dependencyToggle.setValue(true);
    expect(queueToggle.element.disabled).toBe(false);
    expect(queueToggle.element.checked).toBe(true);
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
      wrapper
        .findAll(".privacy-note")
        .some(
          (node) =>
            node.text().includes("诊断信息包含版本、系统") && node.text().includes("不包含 Token"),
        ),
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

  it("复制脱敏的近期错误日志并显示记录数量", async () => {
    vi.mocked(copyRecentErrorLogs).mockResolvedValue(7);
    const wrapper = mountPage();
    const button = wrapper
      .findAll("button.copy-support-button")
      .find((node) => node.text() === "复制近期错误日志");

    expect(button).toBeDefined();
    await button!.trigger("click");
    await flushPromises();

    expect(copyRecentErrorLogs).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("近期错误日志已复制（7 条）");
    expect(
      wrapper
        .findAll(".privacy-note")
        .some(
          (node) =>
            node.text().includes("错误日志仅包含时间、命令") &&
            node.text().includes("不包含远端正文"),
        ),
    ).toBe(true);
  });

  it("近期错误日志为空时显示明确的空状态", async () => {
    vi.mocked(copyRecentErrorLogs).mockResolvedValue(0);
    const wrapper = mountPage();
    const button = wrapper
      .findAll("button.copy-support-button")
      .find((node) => node.text() === "复制近期错误日志");

    await button!.trigger("click");
    await flushPromises();

    expect(copyRecentErrorLogs).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("近期没有已记录的错误");
    expect(wrapper.text()).not.toContain("近期错误日志已复制（0 条）");
  });

  it("近期错误日志复制失败时恢复按钮并显示错误", async () => {
    vi.mocked(copyRecentErrorLogs).mockRejectedValue(new Error("clipboard denied"));
    const wrapper = mountPage();
    const button = wrapper
      .findAll<HTMLButtonElement>("button.copy-support-button")
      .find((node) => node.text() === "复制近期错误日志");

    await button!.trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("复制失败：Error: clipboard denied");
    expect(button!.element.disabled).toBe(false);
    expect(button!.text()).toBe("复制近期错误日志");
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

  it("Windows 便携版通过浏览器下载 ZIP 并提示手动覆盖", async () => {
    const url =
      "https://github.com/tisrop/MergeBeacon/releases/download/v0.4.0/MergeBeacon_0.4.0_x64-portable.zip";
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.0",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
      update_mode: "portable",
      portable_download_url: url,
    });
    vi.mocked(openExternalUrl).mockResolvedValue(undefined);
    const wrapper = mountPage();

    await wrapper.get("button.check-update-button").trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("浏览器中下载 ZIP");
    expect(wrapper.text()).toContain("MergeBeacon.exe 覆盖旧文件");
    expect(wrapper.get("button.install-update-button").text()).toBe("下载便携版 ZIP");

    await wrapper.get("button.install-update-button").trigger("click");
    await flushPromises();
    expect(openExternalUrl).toHaveBeenCalledWith(url);
    expect(downloadAndInstallUpdate).not.toHaveBeenCalled();
    expect(restartAfterUpdate).not.toHaveBeenCalled();
  });

  it("下载中断后清理进度监听并允许重新确认后重试", async () => {
    const requestIds = ["update-first", "update-retry"];
    vi.stubGlobal("crypto", {
      randomUUID: vi.fn(() => requestIds.shift() ?? "unexpected"),
    });
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
    vi.mocked(downloadAndInstallUpdate)
      .mockImplementationOnce(async () => {
        progressCallback?.({
          request_id: "update-first",
          downloaded: 25,
          total: 100,
          phase: "downloading",
        });
        await Promise.resolve();
        throw new Error("download interrupted");
      })
      .mockImplementationOnce(async () => {
        progressCallback?.({
          request_id: "update-retry",
          downloaded: 100,
          total: 100,
          phase: "downloading",
        });
      });
    const wrapper = mountPage();

    await wrapper.get("button.check-update-button").trigger("click");
    await flushPromises();
    await wrapper.get("button.install-update-button").trigger("click");
    await wrapper.get("button.install-update-button").trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("Error: download interrupted");
    expect(wrapper.get<HTMLButtonElement>("button.install-update-button").element.disabled).toBe(
      false,
    );

    await wrapper.get("button.install-update-button").trigger("click");
    expect(wrapper.text()).toContain("安装前请保存工作");
    await wrapper.get("button.install-update-button").trigger("click");
    await flushPromises();

    expect(downloadAndInstallUpdate).toHaveBeenNthCalledWith(1, "update-first", "0.4.0");
    expect(downloadAndInstallUpdate).toHaveBeenNthCalledWith(2, "update-retry", "0.4.0");
    expect(wrapper.text()).not.toContain("download interrupted");
    expect(wrapper.text()).toContain("更新已安装，重启应用后生效");
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
    storage.set("mergebeacon:auto-update-check", "true");
    storage.set("mergebeacon:last-update-check", String(Date.now()));
    const wrapper = mountPage();
    const toggle = wrapper.get<HTMLInputElement>('input[aria-label="每日自动检查更新"]');

    await toggle.setValue(false);

    expect(storage.get("mergebeacon:auto-update-check")).toBe("false");
    expect([...storage.keys()].sort()).toEqual([
      "mergebeacon:auto-update-check",
      "mergebeacon:last-update-check",
    ]);
  });
});
