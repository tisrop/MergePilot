import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { assembleUpdaterMetadata, createUpdaterFragment } from "../updater-metadata.mjs";

const FIXTURE_VERSION = "0.3.5";
const FIXTURE_RELEASE_DOWNLOAD_URL = `https://github.com/tisrop/MergePilot/releases/download/v${FIXTURE_VERSION}`;

async function signatureFile(directory: string, name: string, signature: string) {
  const path = join(directory, name);
  await writeFile(path, signature);
  return path;
}

function fragment(platform: string, assetName: string, version = FIXTURE_VERSION) {
  const entry = {
    signature: `signature-${platform}`,
    asset_label: assetName,
    asset_name: assetName,
  };
  return {
    platform,
    platforms: { [platform]: entry },
    ...(platform === "windows-x86_64"
      ? {
          portable: {
            asset_name: `MergePilot_${version}_x64-portable.exe`,
            signature: "signature-windows-portable",
          },
        }
      : {}),
  };
}

function portableAsset(version = FIXTURE_VERSION, releaseName = `v${version}`) {
  const name = `MergePilot_${version}_x64-portable.exe`;
  return {
    name,
    label: "",
    browser_download_url: `https://github.com/tisrop/MergePilot/releases/download/${releaseName}/${name}`,
  };
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
      version: FIXTURE_VERSION,
    });

    expect(result.platforms["darwin-aarch64"]).toMatchObject({
      asset_label: `Merge Pilot_${FIXTURE_VERSION}_aarch64.app.tar.gz`,
      asset_name: `Merge.Pilot_${FIXTURE_VERSION}_aarch64.app.tar.gz`,
    });
    expect(result.platforms["darwin-aarch64-app"]).toEqual(result.platforms["darwin-aarch64"]);
  });

  it("Windows 主条目优先使用 MSI 并保留 NSIS 条目", async () => {
    const directory = await mkdtemp(join(tmpdir(), "mergepilot-updater-"));
    const msiSignature = await signatureFile(
      directory,
      `Merge Pilot_${FIXTURE_VERSION}_x64_en-US.msi.sig`,
      "msi-signature",
    );
    const nsisSignature = await signatureFile(
      directory,
      `Merge Pilot_${FIXTURE_VERSION}_x64-setup.exe.sig`,
      "nsis-signature",
    );
    const portableExecutable = await signatureFile(
      directory,
      `MergePilot_${FIXTURE_VERSION}_x64-portable.exe`,
      "portable executable",
    );
    await signatureFile(
      directory,
      `MergePilot_${FIXTURE_VERSION}_x64-portable.exe.sig`,
      "portable-signature",
    );

    const result = await createUpdaterFragment({
      artifactPaths: [nsisSignature, msiSignature],
      platform: "windows-x86_64",
      productName: "Merge Pilot",
      version: FIXTURE_VERSION,
      portableExecutablePath: portableExecutable,
    });

    expect(result.platforms["windows-x86_64"].signature).toBe("msi-signature");
    expect(result.platforms["windows-x86_64-msi"].signature).toBe("msi-signature");
    expect(result.platforms["windows-x86_64-nsis"].signature).toBe("nsis-signature");
    expect(result.portable).toEqual({
      asset_name: `MergePilot_${FIXTURE_VERSION}_x64-portable.exe`,
      signature: "portable-signature",
    });
  });

  it("将 Draft Release 临时资源地址转换为发布后的稳定地址", () => {
    const platforms = ["darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64"];
    const fragments = platforms.map((platform) => fragment(platform, `${platform}.updater`));
    const assets = [
      ...platforms.map((platform, index) => ({
        name: `${platform}.updater`,
        label: `${platform}.updater`,
        url: `https://api.github.com/repos/tisrop/MergePilot/releases/assets/${index + 1}`,
        browser_download_url: `https://github.com/tisrop/MergePilot/releases/download/untagged-a1b2c3/${platform}.updater`,
      })),
      portableAsset(FIXTURE_VERSION, "untagged-a1b2c3"),
    ];

    const metadata = assembleUpdaterMetadata({
      fragments,
      assets,
      version: FIXTURE_VERSION,
      notes: "发布说明",
      pubDate: "2026-07-13T12:00:00.000Z",
      assetDownloadUrlPrefix: `${FIXTURE_RELEASE_DOWNLOAD_URL}/`,
    });

    expect(Object.keys(metadata.platforms)).toEqual(platforms);
    expect(metadata.platforms["linux-x86_64"].url).toBe(
      `${FIXTURE_RELEASE_DOWNLOAD_URL}/linux-x86_64.updater`,
    );
    expect(metadata.portable["windows-x86_64"].url).toBe(
      `${FIXTURE_RELEASE_DOWNLOAD_URL}/MergePilot_${FIXTURE_VERSION}_x64-portable.exe`,
    );
    expect(metadata.portable["windows-x86_64"].url).not.toContain(".msi");
    expect(metadata.portable["windows-x86_64"].url).not.toContain(".zip");
    expect(metadata.portable["windows-x86_64"].signature).toBe("signature-windows-portable");
  });

  it("对稳定下载地址中的资源文件名进行 URL 编码", () => {
    const platforms = ["darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64"];
    const fragments = platforms.map((platform) => fragment(platform, `${platform}.updater`));
    const assets = [
      ...platforms.map((platform) => ({
        name: platform === "darwin-aarch64" ? "Merge Pilot.app.tar.gz" : `${platform}.updater`,
        label: `${platform}.updater`,
        browser_download_url: `https://github.com/tisrop/MergePilot/releases/download/untagged-a1b2c3/${platform}.updater`,
      })),
      portableAsset(FIXTURE_VERSION, "untagged-a1b2c3"),
    ];

    const metadata = assembleUpdaterMetadata({
      fragments,
      assets,
      version: FIXTURE_VERSION,
      notes: "发布说明",
      pubDate: "2026-07-13T12:00:00.000Z",
      assetDownloadUrlPrefix: `${FIXTURE_RELEASE_DOWNLOAD_URL}/`,
    });

    expect(metadata.platforms["darwin-aarch64"].url).toBe(
      `${FIXTURE_RELEASE_DOWNLOAD_URL}/Merge%20Pilot.app.tar.gz`,
    );
  });

  it("拒绝缺失平台、重复条目和非官方资源地址", () => {
    const validFragments = [
      fragment("darwin-aarch64", "darwin-aarch64.updater"),
      fragment("darwin-x86_64", "darwin-x86_64.updater"),
      fragment("linux-x86_64", "linux-x86_64.updater"),
      fragment("windows-x86_64", "windows-x86_64.updater"),
    ];
    const assets = [
      ...validFragments.map(({ platform }) => ({
        name: `${platform}.updater`,
        label: `${platform}.updater`,
        url: `https://api.github.com/repos/tisrop/MergePilot/releases/assets/${platform}`,
        browser_download_url: `${FIXTURE_RELEASE_DOWNLOAD_URL}/${platform}.updater`,
      })),
      portableAsset(),
    ];
    const input = {
      fragments: validFragments,
      assets,
      version: FIXTURE_VERSION,
      notes: "发布说明",
      pubDate: "2026-07-13T12:00:00.000Z",
      assetDownloadUrlPrefix: `${FIXTURE_RELEASE_DOWNLOAD_URL}/`,
    };

    expect(() => assembleUpdaterMetadata({ ...input, fragments: validFragments.slice(1) })).toThrow(
      "latest.json 缺少平台条目：darwin-aarch64",
    );
    expect(() =>
      assembleUpdaterMetadata({ ...input, fragments: [...validFragments, validFragments[0]] }),
    ).toThrow("updater 平台条目重复：darwin-aarch64");
    expect(() => assembleUpdaterMetadata({ ...input, assets: assets.slice(0, -1) })).toThrow(
      "Windows 便携版可执行文件无法唯一匹配 Release 资源",
    );
    expect(() =>
      assembleUpdaterMetadata({
        ...input,
        fragments: validFragments.map((item) =>
          item.platform === "windows-x86_64" ? { ...item, portable: undefined } : item,
        ),
      }),
    ).toThrow("Windows 便携版可执行文件签名无效");
    expect(() =>
      assembleUpdaterMetadata({
        ...input,
        fragments: validFragments.map((item) =>
          item.platform === "windows-x86_64"
            ? { ...item, portable: { ...item.portable, signature: "   " } }
            : item,
        ),
      }),
    ).toThrow("Windows 便携版可执行文件签名无效");
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
    expect(workflow).toContain("RELEASE_ID: ${{ needs.prepare-release.outputs.release-id }}");
    expect(workflow).toContain(
      "RELEASE_UPLOAD_URL: ${{ needs.prepare-release.outputs.release-upload-url }}",
    );
    expect(workflow).toContain('$portableName = "MergePilot_${version}_x64-portable.exe"');
    expect(workflow).toContain('Copy-Item $exe "$tmpDir/$portableName"');
    expect(workflow).toContain('npm run tauri -- signer sign "$tmpDir/$portableName"');
    expect(workflow).toContain('Test-Path "$tmpDir/$portableName.sig"');
    expect(workflow).toContain("PORTABLE_EXE=$tmpDir/$portableName");
    expect(workflow).not.toContain("Compress-Archive");
    expect(workflow).not.toContain("PORTABLE_ZIP");
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
