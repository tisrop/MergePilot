import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { assembleUpdaterMetadata, createUpdaterFragment } from "../updater-metadata.mjs";

async function signatureFile(directory: string, name: string, signature: string) {
  const path = join(directory, name);
  await writeFile(path, signature);
  return path;
}

function fragment(platform: string, assetName: string) {
  const entry = {
    signature: `signature-${platform}`,
    asset_label: assetName,
    asset_name: assetName,
  };
  return { platform, platforms: { [platform]: entry } };
}

describe("updater 元数据汇总", () => {
  it("从 macOS Tauri 产物生成架构隔离的分片", async () => {
    const directory = await mkdtemp(join(tmpdir(), "mergepilot-updater-"));
    const signature = await signatureFile(
      directory,
      "Merge Pilot.app.tar.gz.sig",
      "trusted updater signature",
    );

    const result = await createUpdaterFragment({
      artifactPaths: [signature],
      platform: "darwin-aarch64",
      productName: "Merge Pilot",
      version: "0.3.5",
    });

    expect(result.platforms["darwin-aarch64"]).toMatchObject({
      asset_label: "Merge Pilot_0.3.5_aarch64.app.tar.gz",
      asset_name: "Merge.Pilot_0.3.5_aarch64.app.tar.gz",
    });
    expect(result.platforms["darwin-aarch64-app"]).toEqual(result.platforms["darwin-aarch64"]);
  });

  it("Windows 主条目优先使用 MSI 并保留 NSIS 条目", async () => {
    const directory = await mkdtemp(join(tmpdir(), "mergepilot-updater-"));
    const msiSignature = await signatureFile(
      directory,
      "Merge Pilot_0.3.5_x64_en-US.msi.sig",
      "msi-signature",
    );
    const nsisSignature = await signatureFile(
      directory,
      "Merge Pilot_0.3.5_x64-setup.exe.sig",
      "nsis-signature",
    );

    const result = await createUpdaterFragment({
      artifactPaths: [nsisSignature, msiSignature],
      platform: "windows-x86_64",
      productName: "Merge Pilot",
      version: "0.3.5",
    });

    expect(result.platforms["windows-x86_64"].signature).toBe("msi-signature");
    expect(result.platforms["windows-x86_64-msi"].signature).toBe("msi-signature");
    expect(result.platforms["windows-x86_64-nsis"].signature).toBe("nsis-signature");
  });

  it("将 Draft Release 临时资源地址转换为发布后的稳定地址", () => {
    const platforms = ["darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64"];
    const fragments = platforms.map((platform) => fragment(platform, `${platform}.updater`));
    const assets = platforms.map((platform, index) => ({
      name: `${platform}.updater`,
      label: `${platform}.updater`,
      url: `https://api.github.com/repos/tisrop/MergePilot/releases/assets/${index + 1}`,
      browser_download_url: `https://github.com/tisrop/MergePilot/releases/download/untagged-a1b2c3/${platform}.updater`,
    }));

    const metadata = assembleUpdaterMetadata({
      fragments,
      assets,
      version: "0.3.5",
      notes: "发布说明",
      pubDate: "2026-07-13T12:00:00.000Z",
      assetDownloadUrlPrefix: "https://github.com/tisrop/MergePilot/releases/download/v0.3.5/",
    });

    expect(Object.keys(metadata.platforms)).toEqual(platforms);
    expect(metadata.platforms["linux-x86_64"].url).toBe(
      "https://github.com/tisrop/MergePilot/releases/download/v0.3.5/linux-x86_64.updater",
    );
  });

  it("对稳定下载地址中的资源文件名进行 URL 编码", () => {
    const platforms = ["darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64"];
    const fragments = platforms.map((platform) => fragment(platform, `${platform}.updater`));
    const assets = platforms.map((platform) => ({
      name: platform === "darwin-aarch64" ? "Merge Pilot.app.tar.gz" : `${platform}.updater`,
      label: `${platform}.updater`,
      browser_download_url: `https://github.com/tisrop/MergePilot/releases/download/untagged-a1b2c3/${platform}.updater`,
    }));

    const metadata = assembleUpdaterMetadata({
      fragments,
      assets,
      version: "0.3.5",
      notes: "发布说明",
      pubDate: "2026-07-13T12:00:00.000Z",
      assetDownloadUrlPrefix: "https://github.com/tisrop/MergePilot/releases/download/v0.3.5/",
    });

    expect(metadata.platforms["darwin-aarch64"].url).toBe(
      "https://github.com/tisrop/MergePilot/releases/download/v0.3.5/Merge%20Pilot.app.tar.gz",
    );
  });

  it("拒绝缺失平台、重复条目和非官方资源地址", () => {
    const validFragments = [
      fragment("darwin-aarch64", "darwin-aarch64.updater"),
      fragment("darwin-x86_64", "darwin-x86_64.updater"),
      fragment("linux-x86_64", "linux-x86_64.updater"),
      fragment("windows-x86_64", "windows-x86_64.updater"),
    ];
    const assets = validFragments.map(({ platform }) => ({
      name: `${platform}.updater`,
      label: `${platform}.updater`,
      url: `https://api.github.com/repos/tisrop/MergePilot/releases/assets/${platform}`,
      browser_download_url: `https://github.com/tisrop/MergePilot/releases/download/v0.3.5/${platform}.updater`,
    }));
    const input = {
      fragments: validFragments,
      assets,
      version: "0.3.5",
      notes: "发布说明",
      pubDate: "2026-07-13T12:00:00.000Z",
      assetDownloadUrlPrefix: "https://github.com/tisrop/MergePilot/releases/download/v0.3.5/",
    };

    expect(() => assembleUpdaterMetadata({ ...input, fragments: validFragments.slice(1) })).toThrow(
      "latest.json 缺少平台条目：darwin-aarch64",
    );
    expect(() =>
      assembleUpdaterMetadata({ ...input, fragments: [...validFragments, validFragments[0]] }),
    ).toThrow("updater 平台条目重复：darwin-aarch64");
    expect(() =>
      assembleUpdaterMetadata({
        ...input,
        assets: [
          { ...assets[0], browser_download_url: "https://example.com/update" },
          ...assets.slice(1),
        ],
      }),
    ).toThrow("darwin-aarch64 的 Release updater 资源地址无效");

    expect(() =>
      assembleUpdaterMetadata({
        ...input,
        assets: [
          {
            ...assets[0],
            browser_download_url:
              "https://github.com/tisrop/MergePilot/releases/download/v0.3.4/darwin-aarch64.updater",
          },
          ...assets.slice(1),
        ],
      }),
    ).toThrow("darwin-aarch64 的 Release updater 资源地址无效");
  });

  it("Release 工作流按 Tag 串行、平台并行且只有汇总任务写 latest.json", async () => {
    const workflow = await readFile(resolve(".github/workflows/release.yml"), "utf8");

    expect(workflow).toContain("group: release-${{ github.ref }}");
    expect(workflow).toContain("cancel-in-progress: false");
    expect(workflow).not.toContain("max-parallel: 1");
    expect(workflow).toContain("prepare-release:");
    expect(workflow).toContain("releaseId: ${{ needs.prepare-release.outputs.release-id }}");
    expect(workflow).toContain("uploadUpdaterJson: false");
    expect(workflow).toContain("name: updater-fragment-${{ matrix.updater-platform }}");
    expect(workflow).toContain("assemble-updater-metadata:");
    expect(workflow).toContain("needs: [prepare-release, build]");
    expect(workflow).toContain("--asset-download-url-prefix");
    expect(workflow).toContain(
      "${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/releases/download/${GITHUB_REF_NAME}/",
    );
    expect(workflow.match(/gh release upload[^\n]*latest\.json/g)).toHaveLength(1);
  });
});
