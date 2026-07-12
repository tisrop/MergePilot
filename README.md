# MergePilot

[![Rust](https://img.shields.io/badge/Rust-1.96.1%20verified-orange.svg)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-blue.svg)](https://v2.tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.x-42b883.svg)](https://vuejs.org)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

基于 **Tauri 2 + Vue 3 + Rust** 的跨平台 Code Merge 桌面客户端。
使用统一界面连接 GitHub、GitLab 和 Gitee，并可通过 OpenAI 兼容接口对 PR / MR
的代码变更进行 AI 辅助评审。

> 当前版本：`0.2.0`

## 功能概览

- **多平台仓库管理**
  - 使用 Personal Access Token 登录 GitHub、GitLab、Gitee
  - 各平台独立保存登录状态、仓库选择、Fork 上下文和分页进度
  - 仓库侧栏支持增量“加载更多”、去重、失败重试和独立刷新
  - GitLab 与 Gitee 支持填写私有化部署地址，并统一规范化 API 版本路径
- **Pull Request / Merge Request**
  - 按 Open、Closed、Merged、All 状态筛选
  - 支持分页以及 10 / 20 / 50 / 100 条每页
  - 查看标题、作者、分支、标签和合并状态
- **Diff 与人工评审**
  - 使用 diff2html 渲染 side-by-side Diff，并显示文件列表
  - 选中代码后右键添加快速评论
  - 评论支持逻辑、安全、性能、代码风格、日志等分类
  - `⌘/Ctrl + Enter` 提交快速评论，`Esc` 取消
  - 查看整体评审与行级评论，提交评论、批准或请求修改
  - 评论附带代码上下文（被评代码块、过期检测、MiniDiffView）
- **PR 合并 / 关闭 / 重新打开**
  - 支持 Merge、Squash、Rebase 三种合并策略
  - 可自定义合并提交标题和描述
  - 关闭已打开的 PR，重新打开已关闭的 PR
  - 覆盖 GitHub、GitLab、Gitee 三个平台
- **Fork 感知**
  - 自动读取 Fork 的上游仓库信息
  - 默认查看上游 PR，并可在 Fork 与上游仓库之间切换
- **Issue**
  - 查看仓库 Issue
  - 创建带正文和标签的 Issue
- **AI 辅助评审**
  - 支持 OpenAI 兼容的 Chat Completions 与 Models API
  - 内置 OpenAI、DeepSeek、通义千问、Moonshot、Ollama 预设
  - 支持连接测试、模型列表获取与搜索、Temperature、Max Tokens 配置
  - 支持普通响应和标准 SSE 流式响应
  - 流式请求使用独立 request ID，可在重新评审或离开页面时取消
  - 可聚焦全部、安全、性能、逻辑或代码风格
  - 以 Critical / Major / Minor / Info 输出结构化建议
- **界面与交互**
  - 原生菜单栏（macOS ⌘, 打开设置），并提供撤销、剪切、复制、粘贴、全选
  - 平台可见性切换：可独立显示/隐藏 GitHub、GitLab、Gitee 选项卡
  - 设置页面新增界面设置区域

## 平台能力

| 能力 | GitHub | GitLab | Gitee |
|---|:---:|:---:|:---:|
| PAT 登录与仓库列表 | ✅ | ✅ | ✅ |
| 私有化实例地址 | — | ✅ | ✅ |
| PR / MR 列表、详情与 Diff | ✅ | ✅ | ✅ |
| 整体评审 | 评论 / 批准 / 请求修改 | 评论（MR Note） | 评论（PR Comment） |
| 查看整体评审 | ✅ | ✅ | ✅ |
| Diff 行级评论 | ✅ | 🚧 | ✅ |
| Issue 列表与创建 | ✅ | ✅ | ✅ |
| Fork 上游识别 | ✅ | ✅ | ✅ |

> GitLab 的行级评论接口尚未实现；当前快速行评功能在 GitHub 和 Gitee
> 上会真正提交到远端。

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3、Composition API、Pinia、Vue Router |
| 前端构建 | TypeScript、Vite 6 |
| 代码规范 | ESLint + oxfmt（平面配置） |
| 后端 | Rust 2021、Tokio、Reqwest |
| 平台抽象 | `GitPlatform` trait + GitHub / GitLab / Gitee Adapter |
| Diff 渲染 | diff2html、highlight.js |
| AI | OpenAI 兼容 API、SSE Streaming |
| 凭证存储 | 系统 Keyring 优先，AES-256-GCM 加密文件降级 |
| 测试 | Vitest、Vue Test Utils、jsdom、Cargo Test、WireMock |

## 快速开始

### 环境要求

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install) stable（当前在 `1.96.1` 验证）
- 对应操作系统的 [Tauri 2 前置依赖](https://v2.tauri.app/start/prerequisites/)

### 安装与运行

```bash
git clone https://github.com/tisrop/mergepilot.git
cd mergepilot

npm install
npm run tauri -- dev
```

仅启动 Web 前端：

```bash
npm run dev
```

### 构建桌面应用

```bash
npm run tauri -- build
```

构建产物由 Tauri 写入 `src-tauri/target/release/bundle/`。
macOS 下 `.app` 包名基于 `productName`（当前为 `Merge Pilot.app`）。

## 使用说明

### 1. 登录代码托管平台

在登录页选择平台并填写 Personal Access Token。GitLab 或 Gitee 私有化部署可额外
填写服务器地址；未带协议时会自动补充 `https://`。

| 平台 | Token 创建地址 |
|---|---|
| GitHub | [github.com/settings/tokens](https://github.com/settings/tokens) |
| GitLab | [gitlab.com/-/user_settings/personal_access_tokens](https://gitlab.com/-/user_settings/personal_access_tokens) |
| Gitee | [gitee.com/profile/personal_access_tokens](https://gitee.com/profile/personal_access_tokens) |

Token 至少需要读取仓库、PR / MR 和 Issue 的权限；提交评审、评论或创建 Issue
时还需要相应的写权限。

### 2. 评审 PR / MR

1. 在左侧选择平台和仓库。
2. 从 PR 列表打开一条记录。
3. 在 **Diff** 页签查看变更并提交整体评审。
4. 在 GitHub 或 Gitee 上，可选中一段代码后右键打开快速评论框。
5. 在 **评审意见** 页签查看整体评审和行级评论。

### 3. 配置 AI 评审

1. 打开 **设置 → AI 评审设置**。
2. 选择预设或填写 OpenAI 兼容 API 端点。
3. 输入 API Key，点击 **保存设置**。
4. 点击 **获取模型** 并选择模型。
5. 可调整 Temperature、Max Tokens，并使用 **测试连接** 验证配置。
6. 打开 PR / MR 的 **AI 评审** 页签，选择聚焦模式后开始评审。

常用端点示例：

| 服务 | API 端点 | 默认模型示例 |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | `gpt-4o` |
| DeepSeek | `https://api.deepseek.com/v1` | `deepseek-chat` |
| 通义千问 | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `qwen-plus` |
| Moonshot | `https://api.moonshot.cn/v1` | `moonshot-v1-8k` |
| Ollama | `http://localhost:11434/v1` | `llama3` |

AI 请求会携带 PR / MR 标题、描述和 Diff。为控制输入大小，超过约 64 KiB 的
Diff 会在 UTF-8 字符边界安全截断，避免切断中文或 emoji。

## 本地数据与安全

- 平台 Token 优先保存到系统凭证库：macOS Keychain、Windows Credential Manager 或
  Linux Secret Service；service 固定为 `com.mergepilot`。
- 系统凭证库不可用时，Token 使用 AES-256-GCM 加密后写入
  `~/.mergepilot/config.json`；目录权限收紧为 `0700`，文件权限为 `0600`，并采用原子写入。
- 旧版明文 `token_{platform}` 会在首次读取时迁移，目标写入成功后才删除明文。
- 私有化实例地址仍保存在普通配置中；HTTP 地址仅应用于可信内网，Token 传输不会被加密。
- AI 配置保存在操作系统应用配置目录下的 `ai_config.json`。AI API Key 使用
  AES-256-GCM 加密，本轮未迁移到系统 Keyring。
- 登录 Token 只发送到所选代码托管平台；AI API Key 只发送到配置的 AI 端点。
- macOS 应用标识符为 `com.mergepilot`。

请保护本机账号及配置文件权限，不要提交本地配置文件或在不可信设备上保存凭据。

## 项目结构

```text
mergepilot/
├── src/
│   ├── api/                    # Tauri IPC 封装
│   ├── components/
│   │   ├── ai/                 # AI 设置、流式评审、建议卡片
│   │   ├── diff/               # Diff 渲染与快速评论
│   │   ├── issue/              # Issue 卡片与表单
│   │   ├── layout/             # 应用布局、平台与仓库侧边栏
│   │   ├── pr/                 # PR 卡片与筛选器
│   │   └── review/             # 整体评审、评论列表
│   ├── pages/                  # 登录、PR、Issue、设置页面
│   ├── router/                 # Vue Router
│   ├── stores/                 # Pinia Auth / Repo / PR Store
│   └── types/                  # 前端数据类型
├── src-tauri/
│   ├── src/
│   │   ├── ai/                 # OpenAI 兼容客户端、Prompt、配置
│   │   ├── commands/           # Tauri Commands（26 个）
│   │   ├── platform/           # GitPlatform trait 与三个平台 Adapter
│   │   ├── crypto.rs           # AES-256-GCM 加解密
│   │   ├── error.rs            # AppError 统一错误类型
│   │   ├── http_client.rs      # Reqwest 客户端封装
│   │   ├── local_store.rs      # SQLite 评论快照缓存
│   │   ├── main.rs             # 入口点
│   │   ├── models.rs           # 后端数据模型
│   │   ├── state.rs            # 共享状态与可取消 AI 任务注册表
│   │   └── vault.rs            # Keyring 优先、加密文件降级的 TokenVault
│   ├── tests/                  # WireMock 集成测试
│   ├── Cargo.toml
│   └── tauri.conf.json
├── AGENTS.md              # AI 编码代理的项目上下文与操作约束
├── CODE_STANDARDS.md      # 代码实现与评审规范
├── eslint.config.js       # ESLint 平面配置
├── .oxfmtrc.json          # oxfmt 配置
├── package.json
├── vite.config.ts
└── README.md
```

## 代码规范

开发和代码评审以 [`CODE_STANDARDS.md`](CODE_STANDARDS.md) 为基线。该规范覆盖 Vue/TypeScript、
Rust/Tauri 架构边界、跨平台行为、异步生命周期、凭据安全、测试要求和合并门禁。
AI 编码代理还需同时遵循 [`AGENTS.md`](AGENTS.md) 中的项目操作约束。

格式和静态检查分别由 ESLint、oxfmt、rustfmt 与 Clippy 执行；涉及认证、平台切换、分页、AI
请求生命周期或合并结果的改动必须同步增加回归测试。

## 开发与测试

```bash
# 前端类型检查、构建与测试
npm run build
npm test

# 代码检查与格式化
npm run lint
npm run format
npm run format:fix

# Rust 格式、静态检查与测试
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

主要 Tauri Commands（共 26 个）：

- 认证与仓库：`auth_login`、`auth_logout`、`auth_check`、`auth_has_any_token`、`auth_has_token`、`repo_list`
- PR / MR：`pr_list`、`pr_detail`、`pr_diff`、`pr_merge`、`pr_close`、`pr_reopen`
- 评审：`review_submit`、`review_list`、`review_comment_add`、`review_comments_list`
- Issue：`issue_list`、`issue_create`
- AI：`ai_get_config`、`ai_save_config`、`ai_save_api_key`、`ai_review`、`ai_review_stream`、`ai_review_cancel`、`ai_list_models`、`ai_test_connection`

## 已知限制

- GitLab 行级评论目前尚未接入远端 API。
- GitLab 和 Gitee 仅支持评论型整体评审；界面不会显示“批准 / 请求修改”，后端也会拒绝
  非 `comment` 事件。只有 GitHub 支持三种原生评审事件。
- 自托管服务器仍允许 HTTP，以兼容本地和内网实例；请勿在不可信网络中使用 HTTP 传输 Token。
- AI 返回内容需要是约定的 JSON 结构；不兼容该输出格式的模型可能导致解析失败。
- `AppError` 通过字符串返回前端，错误类型信息尚未结构化；后端日志仍以 `eprintln!` 为主。

## License

[Apache 2.0](LICENSE)
