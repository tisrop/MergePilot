import { readFile, readdir } from "node:fs/promises";
import { extname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("../..", import.meta.url)));
const sourceRoot = resolve(projectRoot, "src");
const sourceExtensions = new Set([".css", ".ts", ".vue"]);
const styleExtensions = new Set([".css", ".vue"]);
const findings = [];

const normalizedPath = (path) => relative(projectRoot, path).replaceAll("\\", "/");

async function collectSourceFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectSourceFiles(path)));
    } else if (sourceExtensions.has(extname(entry.name))) {
      files.push(path);
    }
  }

  return files;
}

function report(file, source, index, rule, message) {
  const prefix = source.slice(0, index);
  const line = prefix.split("\n").length;
  const lastNewline = prefix.lastIndexOf("\n");
  const column = index - lastNewline;
  findings.push({ file: normalizedPath(file), line, column, rule, message });
}

function checkPattern(file, source, pattern, rule, message, isAllowed = () => false) {
  for (const match of source.matchAll(pattern)) {
    if (!isAllowed(match)) {
      report(file, source, match.index, rule, message);
    }
  }
}

function checkTauriBoundary(file, source) {
  if (normalizedPath(file) === "src/api/index.ts") return;

  checkPattern(
    file,
    source,
    /(?:from\s+["']@tauri-apps\/api\/core["']|\binvoke\s*\()/g,
    "tauri-ipc-boundary",
    "Tauri invoke 只能出现在 src/api/index.ts 中。",
  );
}

function checkVueHtml(file, source) {
  const path = normalizedPath(file);
  let diffRendererExceptionCount = 0;
  checkPattern(
    file,
    source,
    /\bv-html\s*=\s*["'][^"']*["']/g,
    "no-v-html",
    "远端内容禁止通过 v-html 渲染。",
    (match) => {
      const isDiffRenderer =
        path === "src/components/diff/DiffViewer.vue" && match[0] === 'v-html="diffHtml"';
      if (!isDiffRenderer) return false;
      diffRendererExceptionCount += 1;
      return diffRendererExceptionCount === 1;
    },
  );
}

function checkBannedUiImports(file, source) {
  const packages = [
    "tailwindcss",
    "@tailwindcss/",
    "element-plus",
    "ant-design-vue",
    "vuetify",
    "quasar",
    "naive-ui",
    "primevue",
    "styled-components",
    "@emotion/",
  ];
  const packagePattern = packages
    .map((name) => name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .join("|");
  const pattern = new RegExp(`(?:from\\s+|import\\s*)["'](?:${packagePattern})`, "g");

  checkPattern(
    file,
    source,
    pattern,
    "no-unapproved-ui-framework",
    "禁止未经讨论引入 UI 框架、Tailwind 或 CSS-in-JS。",
  );
}

function checkStyles(file, source) {
  checkPattern(
    file,
    source,
    /transition\s*:\s*all\b/gi,
    "explicit-transitions",
    "禁止 transition: all，请明确列出过渡属性。",
  );
  checkPattern(
    file,
    source,
    /outline\s*:\s*(?:none|0)(?:\s*;|\s*$)/gim,
    "preserve-focus-outline",
    "禁止移除焦点轮廓；请使用 :focus-visible 提供清晰焦点状态。",
  );
  checkPattern(
    file,
    source,
    /[^\n]*!important[^\n]*/g,
    "no-important",
    "禁止使用 !important；仅允许全局 reduced-motion 无障碍兜底。",
    (match) => {
      if (normalizedPath(file) !== "src/style.css") return false;
      return /(?:scroll-behavior:\s*auto|animation-duration:\s*0\.01ms|animation-iteration-count:\s*1|transition-duration:\s*0\.01ms)\s*!important/.test(
        match[0],
      );
    },
  );
}

async function checkConstraintLinks() {
  const checks = [
    ["AGENTS.md", "FRONTEND_STANDARDS.md"],
    ["CODE_STANDARDS.md", "FRONTEND_STANDARDS.md"],
  ];

  for (const [fileName, requiredText] of checks) {
    const path = resolve(projectRoot, fileName);
    const source = await readFile(path, "utf8");
    if (!source.includes(requiredText)) {
      findings.push({
        file: fileName,
        line: 1,
        column: 1,
        rule: "constraint-link",
        message: `${fileName} 必须引用 ${requiredText}。`,
      });
    }
  }
}

const files = await collectSourceFiles(sourceRoot);
for (const file of files) {
  const source = await readFile(file, "utf8");
  checkTauriBoundary(file, source);
  checkBannedUiImports(file, source);

  if (extname(file) === ".vue") {
    checkVueHtml(file, source);
  }
  if (styleExtensions.has(extname(file))) {
    checkStyles(file, source);
  }
}
await checkConstraintLinks();

if (findings.length > 0) {
  console.error(`前端规范检查失败，共 ${findings.length} 项：\n`);
  for (const finding of findings) {
    console.error(
      `${finding.file}:${finding.line}:${finding.column} [${finding.rule}] ${finding.message}`,
    );
  }
  process.exitCode = 1;
} else {
  process.stdout.write(`前端规范检查通过（已检查 ${files.length} 个源码文件）。\n`);
}
