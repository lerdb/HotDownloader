mod commands;
mod download;
mod events;
mod platforms;
mod storage;
mod utils;

use download::engine::DownloadEngine;
use storage::store_wrapper;
use tauri::Manager;

#[cfg(desktop)]
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug) // 可调整为 Info 或 Warn
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_android_fs::init())
        .plugin(tauri_plugin_os::init()) // 注册 OS 插件，提供平台检测能力
        .plugin(tauri_plugin_notification::init()) // 注册通知插件，支持下载完成系统通知
        .setup(|app| {
            let engine = DownloadEngine::new(app.handle().clone());
            let max_concurrent = match store_wrapper::load_string(app.handle(), "settings") {
                Ok(json) => serde_json::from_str::<serde_json::Value>(&json)
                    .ok()
                    .and_then(|v| v.get("maxConcurrent")?.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(3),
                Err(_) => 3,
            };
            engine.set_concurrency(max_concurrent);
            app.manage(engine.clone());

            let engine_clone = engine;
            tauri::async_runtime::spawn(async move {
                engine_clone.run_scheduler().await;
            });

            // ========== 系统托盘（仅桌面端） ==========
            #[cfg(desktop)]
            {
                // 提供系统托盘图标，支持显示/隐藏主窗口和退出应用
                let toggle_item =
                    MenuItem::with_id(app, "toggle", "显示/隐藏主窗口", true, None::<&str>)?;
                let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
                let menu = Menu::with_items(
                    app,
                    &[
                        &toggle_item,
                        &PredefinedMenuItem::separator(app)?,
                        &quit_item,
                    ],
                )?;

                let tray = TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .menu(&menu)
                    .show_menu_on_left_click(false) // 禁止左键弹菜单，左键改为显示窗口
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "toggle" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    window.hide().unwrap();
                                } else {
                                    window.show().unwrap();
                                    window.set_focus().unwrap();
                                }
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                window.show().unwrap();
                                window.set_focus().unwrap();
                            }
                        }
                    })
                    .build(app)?;

                // 保存托盘实例，防止被 drop
                app.manage(tray);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::history::load_history,
            commands::history::save_history,
            commands::tasks::load_tasks,
            commands::tasks::save_tasks,
            commands::tasks::add_download_task,
            commands::tasks::check_download_path,
            commands::tasks::enqueue_task,
            commands::tasks::pause_task,
            commands::tasks::resume_task,
            commands::tasks::cancel_task,
            commands::tasks::remove_task,
            commands::tasks::set_max_concurrent,
            commands::file_ops::get_default_download_dir,
            commands::file_ops::create_directory,
            commands::file_ops::open_file_location,
            commands::file_ops::pick_saf_folder,
            commands::file_ops::delete_saf_file,
            commands::api::search::search_songs,
            commands::api::search::fetch_cover,
            commands::api::download::fetch_download_link,
            commands::api::suggest::fetch_hot_keywords,
            commands::api::suggest::fetch_suggestions,
            commands::api::playlist::fetch_playlist_songs,
            commands::api::update::check_update,
            commands::api::lyrics::get_lyric_by_id,
            commands::api::login::create_qr_login,
            commands::api::login::check_qr_login,
            commands::api::login::login_with_uin_authst,
            commands::api::login::logout,
            commands::api::login::get_login_status,
            commands::notify::request_notification_permission,
            commands::notify::check_notification_permission,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
