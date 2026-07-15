import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  checkForUpdates,
  downloadAndInstallUpdate,
  openExternalUrl,
  listenToUpdateProgress,
  restartAfterUpdate,
} from "@/api";
import type { UpdateCheckResult } from "@/types";
import { useUpdateStore } from "../useUpdateStore";

const storage = new Map<string, string>();

vi.stubGlobal("localStorage", {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

vi.mock("@/api", () => ({
  checkForUpdates: vi.fn(),
  downloadAndInstallUpdate: vi.fn(),
  openExternalUrl: vi.fn(),
  listenToUpdateProgress: vi.fn(),
  restartAfterUpdate: vi.fn(),
}));

const noUpdate: UpdateCheckResult = {
  current_version: "0.3.5",
  available: false,
  version: null,
  notes: null,
  published_at: null,
  update_mode: "installer",
};

describe("useUpdateStore", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    storage.clear();
    setActivePinia(createPinia());
    vi.mocked(checkForUpdates).mockReset();
    vi.mocked(downloadAndInstallUpdate).mockReset();
    vi.mocked(openExternalUrl).mockReset();
    vi.mocked(listenToUpdateProgress).mockReset();
    vi.mocked(listenToUpdateProgress).mockResolvedValue(() => undefined);
    vi.mocked(restartAfterUpdate).mockReset();
  });

  it("迁移旧品牌的自动更新设置", () => {
    localStorage.setItem("mergepilot:auto-update-check", "false");

    const store = useUpdateStore();

    expect(store.isAutoUpdateCheckEnabled).toBe(false);
    expect(localStorage.getItem("mergebeacon:auto-update-check")).toBe("false");
    expect(localStorage.getItem("mergepilot:auto-update-check")).toBeNull();
  });

  it("启动后台检查后记录时间并在一天内避免重复请求", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue(noUpdate);
    const store = useUpdateStore();

    await store.maybeCheckForUpdatesInBackground();
    await store.maybeCheckForUpdatesInBackground();

    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(storage.get("mergebeacon:last-update-check")).toBeTruthy();
    expect(store.updateResult).toEqual(noUpdate);
  });

  it("用户关闭自动检查后启动不发起请求", async () => {
    storage.set("mergebeacon:auto-update-check", "false");
    const store = useUpdateStore();

    await store.maybeCheckForUpdatesInBackground();

    expect(checkForUpdates).not.toHaveBeenCalled();
  });

  it("后台检查失败不产生阻断错误并恢复检查状态", async () => {
    vi.mocked(checkForUpdates).mockRejectedValue("network unavailable");
    const store = useUpdateStore();

    await store.maybeCheckForUpdatesInBackground();

    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(store.updateError).toBe("");
    expect(store.isCheckingUpdate).toBe(false);
  });

  it("本地存储不可用时仍可执行后台检查", async () => {
    vi.spyOn(localStorage, "getItem").mockImplementation(() => {
      throw new DOMException("storage denied", "SecurityError");
    });
    vi.spyOn(localStorage, "setItem").mockImplementation(() => {
      throw new DOMException("storage denied", "SecurityError");
    });
    vi.mocked(checkForUpdates).mockResolvedValue(noUpdate);
    const store = useUpdateStore();

    await store.maybeCheckForUpdatesInBackground();

    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(store.updateResult).toEqual(noUpdate);
  });

  it("下载安装状态由 Store 持有并阻止同一版本重复安装", async () => {
    vi.stubGlobal("crypto", { randomUUID: vi.fn(() => "update-store") });
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.5",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
      update_mode: "installer",
    });
    let resolveDownload!: () => void;
    const pendingDownload = new Promise<void>((resolve) => {
      resolveDownload = resolve;
    });
    vi.mocked(downloadAndInstallUpdate).mockReturnValue(pendingDownload);
    const unlisten = vi.fn();
    vi.mocked(listenToUpdateProgress).mockResolvedValue(unlisten);
    const store = useUpdateStore();
    await store.checkUpdate();

    const installing = store.installUpdate();
    await Promise.resolve();
    await store.installUpdate();

    expect(store.isInstallingUpdate).toBe(true);
    expect(downloadAndInstallUpdate).toHaveBeenCalledOnce();
    expect(downloadAndInstallUpdate).toHaveBeenCalledWith("update-store", "0.4.0");

    resolveDownload();
    await installing;
    await store.installUpdate();

    expect(store.isInstallingUpdate).toBe(false);
    expect(store.isUpdateInstalled).toBe(true);
    expect(downloadAndInstallUpdate).toHaveBeenCalledOnce();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("Windows 便携版在浏览器打开 ZIP，且不注册下载进度监听", async () => {
    const url =
      "https://github.com/tisrop/MergeBeacon/releases/download/v0.4.0/MergeBeacon_0.4.0_x64-portable.zip";
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.5",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
      update_mode: "portable",
      portable_download_url: url,
    });
    vi.mocked(openExternalUrl).mockResolvedValue(undefined);
    const store = useUpdateStore();
    await store.checkUpdate();
    await store.installUpdate();

    expect(openExternalUrl).toHaveBeenCalledWith(url);
    expect(downloadAndInstallUpdate).not.toHaveBeenCalled();
    expect(listenToUpdateProgress).not.toHaveBeenCalled();
    expect(store.isRestartingUpdate).toBe(false);
    expect(store.isUpdateInstalled).toBe(false);
  });

  it("打开便携版 ZIP 失败后允许重试", async () => {
    const url =
      "https://github.com/tisrop/MergeBeacon/releases/download/v0.4.0/MergeBeacon_0.4.0_x64-portable.zip";
    vi.mocked(checkForUpdates).mockResolvedValue({
      current_version: "0.3.5",
      available: true,
      version: "0.4.0",
      notes: null,
      published_at: null,
      update_mode: "portable",
      portable_download_url: url,
    });
    vi.mocked(openExternalUrl)
      .mockRejectedValueOnce(new Error("browser denied"))
      .mockResolvedValueOnce(undefined);
    const store = useUpdateStore();
    await store.checkUpdate();

    await store.installUpdate();
    expect(store.updateError).toContain("browser denied");
    await store.installUpdate();
    expect(openExternalUrl).toHaveBeenCalledTimes(2);
    expect(store.updateError).toBe("");
  });
});
