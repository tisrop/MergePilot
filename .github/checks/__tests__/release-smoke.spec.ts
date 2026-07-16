import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";
import { validateReleaseSmoke } from "../release-smoke.mjs";

const version = "0.4.0";
const tag = `v${version}`;
const repository = "tisrop/MergeBeacon";
const platforms = [
  "darwin-aarch64",
  "darwin-x86_64",
  "linux-x86_64",
  "windows-x86_64",
  "windows-x86_64-msi",
  "windows-x86_64-nsis",
];

function releaseUrl(name: string) {
  return `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(name)}`;
}

function fixture() {
  const platformEntries = Object.fromEntries(
    platforms.map((platform) => {
      const name = `${platform}.updater`;
      return [platform, { url: releaseUrl(name), signature: `signature-${platform}` }];
    }),
  );
  const portableName = `MergeBeacon_${version}_x64-portable.zip`;
  return {
    metadata: {
      version,
      notes: "release notes",
      pub_date: "2026-07-15T00:00:00.000Z",
      platforms: platformEntries,
      portable: { "windows-x86_64": { url: releaseUrl(portableName) } },
    },
    assets: [
      ...platforms.map((platform, index) => ({ id: index + 1, name: `${platform}.updater` })),
      { id: 100, name: portableName },
      { id: 101, name: "latest.json" },
    ],
  };
}

describe("Release smoke 检查", () => {
  it("接受当前 Tag 下资源完整且签名非空的元数据", () => {
    const input = fixture();
    const result = validateReleaseSmoke({ ...input, version, tag, repository });

    expect(result.updater_assets).toHaveLength(platforms.length);
    expect(result.portable_asset).toBe(`MergeBeacon_${version}_x64-portable.zip`);
  });

  it("拒绝版本、仓库或 Tag 不一致的资源地址", () => {
    const wrongVersion = fixture();
    wrongVersion.metadata.version = "0.4.1";
    expect(() => validateReleaseSmoke({ ...wrongVersion, version, tag, repository })).toThrow(
      "latest.json 版本必须为 0.4.0",
    );

    const wrongUrl = fixture();
    wrongUrl.metadata.platforms["linux-x86_64"].url =
      "https://github.com/attacker/project/releases/download/v0.4.0/linux.updater";
    expect(() => validateReleaseSmoke({ ...wrongUrl, version, tag, repository })).toThrow(
      "linux-x86_64必须指向当前仓库和 Tag",
    );
  });

  it("拒绝缺少平台、签名或唯一资源匹配的元数据", () => {
    const missingPlatform = fixture();
    delete missingPlatform.metadata.platforms["darwin-aarch64"];
    expect(() => validateReleaseSmoke({ ...missingPlatform, version, tag, repository })).toThrow(
      "latest.json 缺少平台条目：darwin-aarch64",
    );

    const missingSignature = fixture();
    missingSignature.metadata.platforms["windows-x86_64"].signature = "";
    expect(() => validateReleaseSmoke({ ...missingSignature, version, tag, repository })).toThrow(
      "windows-x86_64 updater 签名不能为空",
    );

    const duplicateAsset = fixture();
    duplicateAsset.assets.push({ id: 200, name: "linux-x86_64.updater" });
    expect(() => validateReleaseSmoke({ ...duplicateAsset, version, tag, repository })).toThrow(
      "linux-x86_64 无法唯一匹配 Release 资源",
    );
  });

  it("拒绝非规范 portable ZIP 地址和资源", () => {
    const input = fixture();
    input.metadata.portable["windows-x86_64"].url = releaseUrl("MergeBeacon.exe");
    expect(() => validateReleaseSmoke({ ...input, version, tag, repository })).toThrow(
      "Windows 便携版 ZIP 无法唯一匹配 Release 资源",
    );
  });

  it("发布必须在 smoke-check 通过后清理签名并发布", async () => {
    const workflow = await readFile(".github/workflows/release.yml", "utf8");
    expect(workflow).toContain("release-smoke:");
    expect(workflow).toContain("release-smoke-windows-portable:");
    expect(workflow).toContain("needs: [release-smoke, release-smoke-windows-portable]");
    expect(workflow).toContain("needs: [cleanup-release-signatures, prepare-release]");
  });

  it("读取 Draft Release 资源的 smoke job 必须拥有 contents write 权限", async () => {
    const workflow = await readFile(".github/workflows/release.yml", "utf8");
    const releaseSmoke = workflow.slice(
      workflow.indexOf("\n  release-smoke:"),
      workflow.indexOf("\n  release-smoke-windows-portable:"),
    );
    const portableSmoke = workflow.slice(
      workflow.indexOf("\n  release-smoke-windows-portable:"),
      workflow.indexOf("\n  cleanup-release-signatures:"),
    );

    expect(releaseSmoke).toMatch(/permissions:\n\s+contents: write/);
    expect(portableSmoke).toMatch(/permissions:\n\s+contents: write/);
    expect(releaseSmoke).toContain("libwebkit2gtk-4.1-dev");
    expect(releaseSmoke).toContain("libgtk-3-dev");
  });
});
