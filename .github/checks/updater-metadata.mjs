import { readFile, readdir, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const EXPECTED_PRIMARY_PLATFORMS = [
  "darwin-aarch64",
  "darwin-x86_64",
  "linux-x86_64",
  "windows-x86_64",
];

const PLATFORM_RULES = {
  "darwin-aarch64": [{ suffix: ".app.tar.gz.sig", bundle: "app" }],
  "darwin-x86_64": [{ suffix: ".app.tar.gz.sig", bundle: "app" }],
  "linux-x86_64": [{ suffix: ".AppImage.sig", bundle: "appimage" }],
  "windows-x86_64": [
    { suffix: ".msi.sig", bundle: "msi" },
    { suffix: ".exe.sig", bundle: "nsis" },
  ],
};

function assertNonEmptyString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${label}不能为空`);
  }
}

function githubAssetName(label) {
  return label
    .trim()
    .replaceAll(/[^a-zA-Z0-9_-]/g, ".")
    .replaceAll(/\.\./g, ".");
}

function assetLabelFor(platform, assetPath, productName, version) {
  if (!platform.startsWith("darwin-")) {
    return basename(assetPath);
  }

  const arch = platform === "darwin-aarch64" ? "aarch64" : "x64";
  return `${productName}_${version}_${arch}.app.tar.gz`;
}

export async function createUpdaterFragment({ artifactPaths, platform, productName, version }) {
  const rules = PLATFORM_RULES[platform];
  if (!rules) {
    throw new Error(`不支持的 updater 平台：${platform}`);
  }
  if (!Array.isArray(artifactPaths) || artifactPaths.some((path) => typeof path !== "string")) {
    throw new Error("Tauri artifactPaths 必须是字符串数组");
  }
  assertNonEmptyString(productName, "productName");
  assertNonEmptyString(version, "version");

  const platformEntries = {};
  let primaryEntry = null;

  for (const rule of rules) {
    const candidates = artifactPaths.filter((path) => path.endsWith(rule.suffix));
    if (candidates.length === 0) {
      throw new Error(`${platform} 缺少 ${rule.suffix} updater 签名`);
    }
    if (candidates.length > 1) {
      throw new Error(`${platform} 存在多个 ${rule.suffix} updater 签名，无法安全选择`);
    }

    const signaturePath = candidates[0];
    const signature = await readFile(signaturePath, "utf8");
    if (signature.length === 0 || signature.length > 16 * 1024) {
      throw new Error(`${platform} 的 ${rule.bundle} updater 签名长度无效`);
    }

    const assetPath = signaturePath.slice(0, -".sig".length);
    const assetLabel = assetLabelFor(platform, assetPath, productName, version);
    const entry = {
      signature,
      asset_label: assetLabel,
      asset_name: githubAssetName(assetLabel),
    };
    platformEntries[`${platform}-${rule.bundle}`] = entry;
    primaryEntry ??= entry;
  }

  platformEntries[platform] = primaryEntry;
  return { platform, platforms: platformEntries };
}

export function assembleUpdaterMetadata({
  fragments,
  assets,
  version,
  notes,
  pubDate,
  assetDownloadUrlPrefix,
}) {
  if (!Array.isArray(fragments) || fragments.length === 0) {
    throw new Error("缺少 updater 元数据分片");
  }
  if (!Array.isArray(assets)) {
    throw new Error("Release assets 必须是数组");
  }
  assertNonEmptyString(version, "version");
  if (typeof notes !== "string") {
    throw new Error("notes 必须是字符串");
  }
  assertNonEmptyString(pubDate, "pub_date");
  if (Number.isNaN(Date.parse(pubDate))) {
    throw new Error("pub_date 不是有效时间");
  }
  assertNonEmptyString(assetDownloadUrlPrefix, "Release asset 下载 URL 前缀");

  const references = {};
  for (const fragment of fragments) {
    if (!fragment || !PLATFORM_RULES[fragment.platform] || typeof fragment.platforms !== "object") {
      throw new Error("updater 元数据分片格式无效");
    }
    for (const [key, reference] of Object.entries(fragment.platforms)) {
      if (references[key]) {
        throw new Error(`updater 平台条目重复：${key}`);
      }
      references[key] = reference;
    }
  }

  for (const platform of EXPECTED_PRIMARY_PLATFORMS) {
    if (!references[platform]) {
      throw new Error(`latest.json 缺少平台条目：${platform}`);
    }
  }

  const platforms = {};
  for (const [key, reference] of Object.entries(references)) {
    if (
      !reference ||
      typeof reference !== "object" ||
      typeof reference.signature !== "string" ||
      reference.signature.length === 0 ||
      reference.signature.length > 16 * 1024
    ) {
      throw new Error(`${key} 的 updater 签名无效`);
    }

    const matchingAssets = assets.filter(
      (asset) => asset?.label === reference.asset_label || asset?.name === reference.asset_name,
    );
    if (matchingAssets.length !== 1) {
      throw new Error(`${key} 无法唯一匹配 Release updater 资源`);
    }

    const url = matchingAssets[0].browser_download_url;
    if (typeof url !== "string" || !url.startsWith(assetDownloadUrlPrefix)) {
      throw new Error(`${key} 的 Release updater 资源地址无效`);
    }
    platforms[key] = { signature: reference.signature, url };
  }

  return { version, notes, pub_date: pubDate, platforms };
}

function parseArguments(argv) {
  const [command, ...rest] = argv;
  const options = {};
  for (let index = 0; index < rest.length; index += 2) {
    const name = rest[index];
    const value = rest[index + 1];
    if (!name?.startsWith("--") || value === undefined) {
      throw new Error(`命令参数无效：${name ?? "<empty>"}`);
    }
    options[name.slice(2)] = value;
  }
  return { command, options };
}

async function readFragments(directory) {
  const names = (await readdir(directory)).filter((name) => name.endsWith(".json")).sort();
  return Promise.all(
    names.map(async (name) => JSON.parse(await readFile(resolve(directory, name), "utf8"))),
  );
}

async function main() {
  const { command, options } = parseArguments(process.argv.slice(2));

  if (command === "fragment") {
    const artifactPaths = JSON.parse(process.env.TAURI_ARTIFACT_PATHS ?? "null");
    const config = JSON.parse(await readFile(options.config, "utf8"));
    const fragment = await createUpdaterFragment({
      artifactPaths,
      platform: options.platform,
      productName: config.productName,
      version: config.version,
    });
    await writeFile(options.output, `${JSON.stringify(fragment, null, 2)}\n`);
    return;
  }

  if (command === "assemble") {
    const [fragments, assets, notes] = await Promise.all([
      readFragments(options.fragments),
      readFile(options.assets, "utf8").then(JSON.parse),
      readFile(options.notes, "utf8"),
    ]);
    const metadata = assembleUpdaterMetadata({
      fragments,
      assets,
      version: options.version,
      notes,
      pubDate: options["pub-date"] ?? new Date().toISOString(),
      assetDownloadUrlPrefix: options["asset-download-url-prefix"],
    });
    await writeFile(options.output, `${JSON.stringify(metadata, null, 2)}\n`);
    return;
  }

  throw new Error(`未知命令：${command ?? "<empty>"}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
