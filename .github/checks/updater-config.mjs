import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const projectRoot = process.cwd();
const OFFICIAL_UPDATER_ENDPOINT =
  "https://github.com/tisrop/MergePilot/releases/latest/download/latest.json";

function decodeCanonicalBase64(value, field) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value !== value.trim() ||
    value.length % 4 !== 0 ||
    !/^[A-Za-z0-9+/]+={0,2}$/.test(value)
  ) {
    throw new Error(`${field}不是规范的 Base64`);
  }

  const decoded = Buffer.from(value, "base64");
  if (decoded.toString("base64") !== value) {
    throw new Error(`${field}不是规范的 Base64`);
  }
  return decoded;
}

export function assertUpdaterConfig(config) {
  if (config?.bundle?.createUpdaterArtifacts !== true) {
    throw new Error("必须启用 bundle.createUpdaterArtifacts");
  }

  const updater = config?.plugins?.updater;
  if (!updater || typeof updater !== "object") {
    throw new Error("缺少 plugins.updater 配置");
  }

  if (
    !Array.isArray(updater.endpoints) ||
    updater.endpoints.length !== 1 ||
    updater.endpoints[0] !== OFFICIAL_UPDATER_ENDPOINT
  ) {
    throw new Error(`updater endpoint 必须精确配置为 ${OFFICIAL_UPDATER_ENDPOINT}`);
  }

  const publicKeyFile = decodeCanonicalBase64(updater.pubkey, "updater 公钥");
  const publicKeyText = publicKeyFile.toString("utf8");
  if (!publicKeyFile.equals(Buffer.from(publicKeyText, "utf8"))) {
    throw new Error("updater 公钥不是有效的 UTF-8 文本");
  }

  const lines = publicKeyText.replace(/\r?\n$/, "").split(/\r?\n/);
  const commentMatch = /^untrusted comment: minisign public key: ([0-9A-F]{16})$/.exec(
    lines[0] ?? "",
  );
  if (lines.length !== 2 || !commentMatch) {
    throw new Error("updater 公钥必须是 Tauri 生成的 minisign 公钥文件");
  }

  const keyBytes = decodeCanonicalBase64(lines[1], "minisign 公钥数据");
  if (keyBytes.length !== 42 || keyBytes[0] !== 0x45 || keyBytes[1] !== 0x64) {
    throw new Error("minisign 公钥数据格式无效");
  }

  const encodedKeyId = keyBytes.subarray(2, 10).reverse().toString("hex").toUpperCase();
  if (encodedKeyId !== commentMatch[1]) {
    throw new Error("minisign 公钥注释中的 Key ID 与公钥数据不一致");
  }
}

export async function readUpdaterConfig(root = projectRoot) {
  const source = await readFile(resolve(root, "src-tauri/tauri.conf.json"), "utf8");
  return JSON.parse(source);
}

async function main() {
  assertUpdaterConfig(await readUpdaterConfig());
  process.stdout.write("updater 配置安全检查通过\n");
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
