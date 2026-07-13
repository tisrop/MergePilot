import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const projectRoot = process.cwd();

export function parseCargoVersion(source) {
  let inPackage = false;
  for (const line of source.split(/\r?\n/)) {
    const section = line.match(/^\s*\[([^\]]+)]\s*$/);
    if (section) {
      inPackage = section[1] === "package";
      continue;
    }
    if (inPackage) {
      const version = line.match(/^\s*version\s*=\s*"([^"]+)"\s*$/);
      if (version) {
        return version[1];
      }
    }
  }
  return null;
}

export function assertConsistentVersions(versions) {
  const missing = Object.entries(versions).filter(([, version]) => !version);
  if (missing.length > 0) {
    throw new Error(`无法读取版本：${missing.map(([file]) => file).join("、")}`);
  }

  const uniqueVersions = new Set(Object.values(versions));
  if (uniqueVersions.size !== 1) {
    const details = Object.entries(versions)
      .map(([file, version]) => `${file}=${version}`)
      .join("，");
    throw new Error(`应用版本不一致：${details}`);
  }

  return Object.values(versions)[0];
}

export function assertReleaseTag(tag, manifestVersion) {
  const match =
    /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.exec(
      tag,
    );
  if (!match) {
    throw new Error(`发布标签格式无效：${tag}，应为 vX.Y.Z`);
  }

  const tagVersion = tag.slice(1);
  if (tagVersion !== manifestVersion) {
    throw new Error(`发布标签与应用版本不一致：tag=${tag}，manifest=${manifestVersion}`);
  }

  return tagVersion;
}

export async function readProjectVersions(root = projectRoot) {
  const [packageSource, cargoSource, tauriSource] = await Promise.all([
    readFile(resolve(root, "package.json"), "utf8"),
    readFile(resolve(root, "src-tauri/Cargo.toml"), "utf8"),
    readFile(resolve(root, "src-tauri/tauri.conf.json"), "utf8"),
  ]);

  return {
    "package.json": JSON.parse(packageSource).version ?? null,
    "src-tauri/Cargo.toml": parseCargoVersion(cargoSource),
    "src-tauri/tauri.conf.json": JSON.parse(tauriSource).version ?? null,
  };
}

async function main() {
  const versions = await readProjectVersions();
  const version = assertConsistentVersions(versions);
  const tagIndex = process.argv.indexOf("--tag");
  if (tagIndex !== -1) {
    const tag = process.argv[tagIndex + 1];
    if (!tag) {
      throw new Error("--tag 缺少发布标签值");
    }
    assertReleaseTag(tag, version);
    process.stdout.write(`发布版本一致性检查通过：${tag}\n`);
    return;
  }
  process.stdout.write(`版本一致性检查通过：${version}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
