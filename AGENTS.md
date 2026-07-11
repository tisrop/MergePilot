# MergePilot — AGENTS.md

Tauri 2 + Vue 3 + Rust 跨平台 Code Merge 桌面客户端（GitHub/GitLab/Gitee + AI 评审）。

## 开发者命令

```bash
npm run dev              # 仅前端 Vite dev server (port 1420)
npm run tauri -- dev     # 完整 Tauri 桌面应用（dev server + 原生窗口）
npm run build            # vue-tsc --noEmit 类型检查 → vite build
npm run tauri -- build   # 生产构建，产物在 src-tauri/target/release/bundle/
npm run lint             # ESLint 检查
npm run lint:fix         # ESLint 自动修复
npm run format           # Prettier 格式检查
npm run format:fix       # Prettier 格式化
cd src-tauri && cargo test              # 全部 Rust 测试
cd src-tauri && cargo test github_adapter -- --nocapture  # 单个测试目标
```

- `TAURI_DEV_HOST` 环境变量启用网络 HMR（默认仅 localhost:1420）。
- 前端 IPC 封装在 `src/api/index.ts`（`invoke("command_name", {...})`）。**不要直接调用 Tauri API**。
- Vite 配置了 `@/` → `./src/*` 别名。

## 项目结构

```
src/                 # Vue 3 前端 (Composition API, <script setup lang="ts">)
  api/index.ts       # 唯一 IPC 入口 — 所有后端调用走这里
  types/index.ts     # 前端 TS 类型
  stores/            # Pinia (useAuthStore, usePrStore, useRepoStore)
  router/index.ts    # 7 条路由
  pages/             # 页面组件
  components/        # ai/ diff/ issue/ layout/ pr/ review/
  main.ts            # 入口：挂载 Pinia + Router，阻止 /settings 在 HMR 恢复
src-tauri/           # Rust 后端
  src/lib.rs         # Tauri Builder：注册 20 个 IPC 命令，设置菜单
  src/state.rs       # AppState (HttpClient + TokenVault + AiConfigManager)
  src/platform/      # GitPlatform trait + GitHub/GitLab/Gitee adapter
  src/ai/            # OpenAI 兼容客户端（同步 + 流式 SSE）
  src/commands/      # auth pr review issue ai
  tests/             # WireMock 集成测试
```

## Rust 后端要点

- **error.rs**: `AppError` 枚举，通过 `impl From<AppError> for String` 兼容 Tauri 命令返回值。新增错误变体需添加此转换。
- **crypto.rs**: AES-256-GCM；密钥 = SHA256(应用固定值 + OS 用户名)。
- **platform/mod.rs**: `#[async_trait] trait GitPlatform` — 添加新平台只需实现此 trait。
- GitLab/Gitee `create_pr_comment` 和 `list_pr_comments` **未实现**（函数体为空）。
- AppError → String 转换丢失类型信息；无结构化日志（仅 `eprintln!`）。
- 测试用 WireMock mock HTTP server，不依赖真实 API。

## 前端要点

- **无 UI 框架**：手写 CSS + CSS 变量。不要引入 Tailwind 或组件库除非沟通。
- Pinia store 直接调用 `src/api/index.ts`，不直接 `invoke`。
- ESLint 使用平面配置（`eslint.config.js`），集成 `eslint-plugin-vue`、`@typescript-eslint`、`eslint-config-prettier`。无内联 `eslint-disable` 注释。
- Prettier 配置在 `.prettierrc`，分号、双引号、尾逗号、100 列宽。`.prettierignore` 忽略 `dist/`、`src-tauri/`。
- `src/main.ts:8` 有 HMR 保护：`/settings` 路径在启动时重定向到 `/pr`。
- 菜单 "设置..."（Cmd+,）通过 `window.__goToSettings()` 在运行时切换路由。
- 语言：界面和 AI Prompt 均为中文。

## 测试

- **仅 Rust 后端有测试**（WireMock），前端无测试。
- 两个命名测试目标：`github_adapter`、`gitee_adapter`（定义在 `Cargo.toml` `[[test]]`）。
- 运行单个测试：`cargo test github_adapter -- --nocapture`（从 `src-tauri/`）。

## 构建与部署

- macOS bundle name: "Merge Pilot.app", identifier: `com.mergepilot`。
- 构建前自动执行 `npm run build`（`tauri.conf.json` `beforeBuildCommand`）。
- 前端构建产物在 `dist/`；Rust 构建产物在 `src-tauri/target/`。
- macOS entitlement 在 `src-tauri/mergepilot.entitlements`。

## 注意

- `.github/` **无 CI 工作流**。
- 此仓库使用 OpenCode 的 openspec 工作流（`openspec/` 目录）。
- `.gitignore` 排除了 `.opencode/`、`.reasonix/`、`reasonix.toml`。
