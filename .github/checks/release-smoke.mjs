import { readFile, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const EXPECTED_PLATFORMS = [
  "darwin-aarch64",
  "darwin-x86_64",
  "linux-x86_64",
  "windows-x86_64",
  "windows-x86_64-msi",
  "windows-x86_64-nsis",
];

function assertNonEmptyString(value, label) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${label}不能为空`);
  }
}

function expectedAssetUrl(repository, tag, assetName) {
  return `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(assetName)}`;
}

function assetNameFromUrl(url, repository, tag, label) {
  let parsed;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error(`${label}的资源地址无效`);
  }
  const webPrefix = `/${repository}/releases/download/${tag}/`;
  if (
    parsed.protocol !== "https:" ||
    parsed.hostname !== "github.com" ||
    !parsed.pathname.startsWith(webPrefix)
  ) {
    throw new Error(`${label}必须指向当前仓库和 Tag`);
  }
  const encodedName = parsed.pathname.slice(webPrefix.length);
  if (!encodedName || encodedName.includes("/")) {
    throw new Error(`${label}的资源文件名无效`);
  }
  let name;
  try {
    name = decodeURIComponent(encodedName);
  } catch {
    throw new Error(`${label}的资源文件名编码无效`);
  }
  if (basename(name) !== name || name.includes("\\")) {
    throw new Error(`${label}的资源文件名不安全`);
  }
  if (url !== expectedAssetUrl(repository, tag, name)) {
    throw new Error(`${label}的资源地址不是规范的正式 Tag 地址`);
  }
  return name;
}

export function validateReleaseSmoke({ metadata, assets, version, tag, repository }) {
  assertNonEmptyString(version, "版本");
  assertNonEmptyString(tag, "Tag");
  assertNonEmptyString(repository, "仓库");
  if (tag !== `v${version}`) {
    throw new Error(`Tag 与版本不一致：${tag} != v${version}`);
  }
  if (!metadata || typeof metadata !== "object" || metadata.version !== version) {
    throw new Error(`latest.json 版本必须为 ${version}`);
  }
  if (!Array.isArray(assets)) {
    throw new Error("Release 资源列表无效");
  }
  if (Number.isNaN(Date.parse(metadata.pub_date))) {
    throw new Error("latest.json 的 pub_date 无效");
  }

  const assetCounts = new Map();
  for (const asset of assets) {
    assertNonEmptyString(asset?.name, "Release 资源名称");
    assetCounts.set(asset.name, (assetCounts.get(asset.name) ?? 0) + 1);
  }

  const updaterAssets = [];
  for (const platform of EXPECTED_PLATFORMS) {
    const entry = metadata.platforms?.[platform];
    if (!entry || typeof entry !== "object") {
      throw new Error(`latest.json 缺少平台条目：${platform}`);
    }
    assertNonEmptyString(entry.signature, `${platform} updater 签名`);
    const assetName = assetNameFromUrl(entry.url, repository, tag, platform);
    if (assetCounts.get(assetName) !== 1) {
      throw new Error(`${platform} 无法唯一匹配 Release 资源：${assetName}`);
    }
    updaterAssets.push({ platform, asset_name: assetName, signature: entry.signature });
  }

  const portableName = `MergeBeacon_${version}_x64-portable.zip`;
  const portable = metadata.portable?.["windows-x86_64"];
  if (!portable || typeof portable !== "object") {
    throw new Error("latest.json 缺少 Windows 便携版 ZIP");
  }
  const actualPortableName = assetNameFromUrl(portable.url, repository, tag, "Windows 便携版 ZIP");
  if (actualPortableName !== portableName || assetCounts.get(portableName) !== 1) {
    throw new Error("Windows 便携版 ZIP 无法唯一匹配 Release 资源");
  }

  return { updater_assets: updaterAssets, portable_asset: portableName };
}

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith("--") || value === undefined) {
      throw new Error(`命令参数无效：${name ?? "<empty>"}`);
    }
    options[name.slice(2)] = value;
  }
  return options;
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const [metadata, assets] = await Promise.all([
    readFile(options.metadata, "utf8").then(JSON.parse),
    readFile(options.assets, "utf8").then(JSON.parse),
  ]);
  const manifest = validateReleaseSmoke({
    metadata,
    assets,
    version: options.version,
    tag: options.tag,
    repository: options.repository,
  });
  await writeFile(options.output, `${JSON.stringify(manifest, null, 2)}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
