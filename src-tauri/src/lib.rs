pub mod ai;
mod commands;
pub mod crypto;
pub mod error;
pub mod http_client;
pub mod local_store;
pub mod models;
pub mod platform;
mod state;
pub mod vault;

use commands::{ai as ai_cmds, auth, issue, pr, review};
use local_store::CommentSnapshotStore;
use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().unwrap_or_default();
            let comment_store = CommentSnapshotStore::new(&app_dir.join("comment_cache.db"));
            app.manage(comment_store);

            let settings = MenuItem::with_id(app, "open-settings", "设置...", true, Some("Cmd+,"))?;
            let app_menu = Submenu::with_items(
                app,
                "MergePilot",
                true,
                &[&settings, &PredefinedMenuItem::separator(app)?, &PredefinedMenuItem::quit(app, None)?],
            )?;
            let edit_menu = Submenu::with_items(
                app,
                "编辑",
                true,
                &[
                    &PredefinedMenuItem::undo(app, Some("撤销"))?,
                    &PredefinedMenuItem::redo(app, Some("重做"))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, Some("剪切"))?,
                    &PredefinedMenuItem::copy(app, Some("复制"))?,
                    &PredefinedMenuItem::paste(app, Some("粘贴"))?,
                    &PredefinedMenuItem::select_all(app, Some("全选"))?,
                ],
            )?;
            let menu = Menu::with_items(app, &[&app_menu, &edit_menu])?;
            app.set_menu(menu)?;

            let menu_ready = Arc::new(AtomicBool::new(false));
            let menu_ready_clone = menu_ready.clone();

            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                menu_ready_clone.store(true, Ordering::SeqCst);
            });

            app.on_menu_event(move |handle, event| {
                if event.id() == "open-settings" && menu_ready.load(Ordering::SeqCst) {
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.eval("if(typeof window.__goToSettings==='function'){window.__goToSettings()}");
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Auth
            auth::auth_login,
            auth::auth_logout,
            auth::auth_check,
            auth::auth_has_any_token,
            auth::auth_has_token,
            // Repo
            auth::repo_list,
            // PR
            pr::pr_list,
            pr::pr_detail,
            pr::pr_diff,
            pr::pr_merge,
            pr::pr_close,
            pr::pr_reopen,
            // Review
            review::review_submit,
            review::review_comment_add,
            review::review_list,
            review::review_comments_list,
            // Issue
            issue::issue_list,
            issue::issue_create,
            // AI
            ai_cmds::ai_get_config,
            ai_cmds::ai_save_config,
            ai_cmds::ai_save_api_key,
            ai_cmds::ai_review,
            ai_cmds::ai_review_stream,
            ai_cmds::ai_review_cancel,
            ai_cmds::ai_list_models,
            ai_cmds::ai_test_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
