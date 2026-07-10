## Why

当前设置页面 (`/settings`) 放在主界面左侧导航栏中，与 PR 列表、Issue 等功能并列。这与桌面应用的 UX 习惯不符 — 设置属于全局配置，不应挤占核心工作区域的导航空间。macOS 原生应用通常将偏好设置放到菜单栏（`⌘,` 快捷键），更符合用户预期。

## What Changes

- 在 Rust 后端添加 Tauri 原生菜单栏，包含 "设置..." 菜单项（映射 `⌘,` 快捷键）
- 移除侧边栏导航中的 "⚙ 设置" 链接
- 前端监听菜单触发的 `open-settings` 事件，跳转到 `/settings` 路由
- `/settings` 路由和 SettingsPage 组件保留不变，仍可 URL 访问

## Capabilities

### New Capabilities
- `native-menu`: Tauri 原生菜单栏，支持菜单项点击 → 事件通知前端

### Modified Capabilities
- *(无 spec 级别变更)*

## Impact

- `src-tauri/src/lib.rs` — 添加 Tauri 菜单创建逻辑和事件发射
- `src/components/layout/Sidebar.vue` — 移除设置导航链接
- `src/App.vue` 或 `src/main.ts` — 添加菜单事件监听
- `Cargo.toml` — 可能需要添加 `tauri-plugin-menu` 依赖
