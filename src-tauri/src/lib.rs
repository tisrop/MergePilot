pub mod ai;
mod commands;
pub mod crypto;
pub mod error;
pub mod http_client;
pub mod local_store;
pub mod models;
pub mod platform;
mod single_instance;
mod state;
pub mod vault;
mod window_state;

use commands::{ai as ai_cmds, auth, capabilities, issue, pr, review, support, update};
use local_store::CommentSnapshotStore;
use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::Manager;
use tauri_plugin_window_state::{StateFlags, WindowExt};

pub fn run() {
    let activation = Arc::new(single_instance::ActivationCoordinator::default());
    let second_instance_activation = activation.clone();
    let setup_activation = activation.clone();

    tauri::Builder::default()
        // 官方要求 single-instance 必须是首个注册的插件。
        .plugin(tauri_plugin_single_instance::init(move |app, _args, _cwd| {
            if second_instance_activation.request_activation() {
                single_instance::activate_main_window(app);
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::POSITION | StateFlags::SIZE | StateFlags::MAXIMIZED)
                .skip_initial_state("main")
                .build(),
        )
        .manage(AppState::new())
        .setup(move |app| {
            let app_dir = app.path().app_data_dir().unwrap_or_default();
            let comment_store = CommentSnapshotStore::new(&app_dir.join("comment_cache.db"));
            app.manage(comment_store);

            if let Some(window) = app.get_webview_window("main") {
                let restored = window
                    .restore_state(StateFlags::POSITION | StateFlags::SIZE)
                    .and_then(|()| window_state::ensure_visible(&window))
                    .and_then(|()| window.restore_state(StateFlags::MAXIMIZED));
                if let Err(error) = restored {
                    eprintln!("恢复窗口状态失败：{error}");
                }
            }
            if setup_activation.mark_ready() {
                single_instance::activate_main_window(app.handle());
            }

            let settings = MenuItem::with_id(app, "open-settings", "设置...", true, Some("Cmd+,"))?;
            let app_menu = Submenu::with_items(
                app,
                "MergeBeacon",
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
            // Support / platform capabilities
            support::support_info,
            support::copy_support_info,
            support::app_version,
            update::update_check,
            update::update_download_and_install,
            update::update_restart,
            capabilities::platform_capabilities,
            // Repo
            auth::repo_list,
            // PR
            pr::pr_list,
            pr::pr_detail,
            pr::pr_merge_readiness,
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
