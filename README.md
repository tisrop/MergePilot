# MergePilot

[![Rust](https://img.shields.io/badge/Rust-1.96.1%20verified-orange.svg)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-blue.svg)](https://v2.tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.x-42b883.svg)](https://vuejs.org)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

基于 **Tauri 2 + Vue 3 + Rust** 的跨平台 Code Merge 桌面客户端。
使用统一界面连接 GitHub、GitLab 和 Gitee，并可通过 OpenAI 兼容接口对 PR / MR
的代码变更进行 AI 辅助评审。

> 当前版本：`0.1.0`

## 功能概览

- **多平台仓库管理**
  - 使用 Personal Access Token 登录 GitHub、GitLab、Gitee
  - 在多个平台之间切换并浏览当前账号可访问的仓库
  - GitLab 与 Gitee 支持填写私有化部署地址
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
  - 支持普通响应和 SSE 流式响应
  - 可聚焦全部、安全、性能、逻辑或代码风格
  - 以 Critical / Major / Minor / Info 输出结构化建议

## 平台能力

| 能力 | GitHub | GitLab | Gitee |
|---|:---:|:---:|:---:|
| PAT 登录与仓库列表 | ✅ | ✅ | ✅ |
| 私有化实例地址 | — | ✅ | ✅ |
| PR / MR 列表、详情与 Diff | ✅ | ✅ | ✅ |
| 整体评审 | 原生 Review | 以 MR Note 提交 | 以 PR Comment 提交 |
| 查看整体评审 | ✅ | ✅ | ✅ |
| Diff 行级评论 | ✅ | 🚧 | 🚧 |
| Issue 列表与创建 | ✅ | ✅ | ✅ |
| Fork 上游识别 | ✅ | ✅ | ✅ |

> GitLab 和 Gitee 的行级评论接口尚未实现；当前快速行评功能仅在 GitHub
> 上会真正提交到远端。

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3、Composition API、Pinia、Vue Router |
| 前端构建 | TypeScript、Vite 6 |
| 后端 | Rust 2021、Tokio、Reqwest |
| 平台抽象 | `GitPlatform` trait + GitHub / GitLab / Gitee Adapter |
| Diff 渲染 | diff2html、highlight.js |
| AI | OpenAI 兼容 API、SSE Streaming |
| API Key 加密 | AES-256-GCM、SHA-256 |
| 测试 | Cargo Test、WireMock、vue-tsc |

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
4. 在 GitHub 上，可选中一段代码后右键打开快速评论框。
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
Diff 当前只发送前 64 KiB。

## 本地数据与安全

- 平台 Token 和私有化实例地址保存在 `~/.mergepilot/config.json`。
- **平台 Token 当前以明文保存在本地配置文件中，尚未接入系统 Keychain。**
- AI 配置保存在操作系统应用配置目录下的 `ai_config.json`。
- AI API Key 使用 AES-256-GCM 加密；密钥由应用固定值与当前系统用户名派生。
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
│   │   ├── commands/           # Tauri Commands
│   │   ├── platform/           # GitPlatform trait 与三个平台 Adapter
│   │   ├── crypto.rs           # AES-256-GCM 加解密
│   │   ├── error.rs            # AppError 统一错误类型
│   │   ├── http_client.rs      # Reqwest 客户端封装
│   │   ├── models.rs           # 后端数据模型
│   │   ├── state.rs            # 共享应用状态（HttpClient + TokenVault + AiConfigManager）
│   │   └── vault.rs            # Token 与实例地址存储
│   ├── tests/                  # WireMock 集成测试
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
├── vite.config.ts
└── README.md
```

## 开发与测试

```bash
# 前端类型检查并构建
npm run build

# Rust 测试
cd src-tauri
cargo test
```

主要 Tauri Commands：

- 认证与仓库：`auth_login`、`auth_logout`、`auth_check`、`repo_list`
- PR / MR：`pr_list`、`pr_detail`、`pr_diff`
- 评审：`review_submit`、`review_list`、`review_comment_add`、`review_comments_list`
- Issue：`issue_list`、`issue_create`
- AI：`ai_get_config`、`ai_save_config`、`ai_save_api_key`、`ai_review`、`ai_review_stream`、`ai_list_models`、`ai_test_connection`

## 已知限制

- GitLab / Gitee 行级评论目前尚未接入远端 API。
- GitLab 的“批准 / 请求修改”当前以带标记的 MR Note 表达，不会改变 GitLab
  原生审批状态；Gitee 同样以 PR Comment 表达整体评审。
- 仓库列表分页界面尚未提供翻页入口；GitLab / Gitee Adapter 当前也未返回准确的
  仓库总页数。
- 当前自动化集成测试主要覆盖 GitHub Adapter。
- AI 返回内容需要是约定的 JSON 结构；不兼容该输出格式的模型可能导致解析失败。

## License

[MIT](LICENSE)
