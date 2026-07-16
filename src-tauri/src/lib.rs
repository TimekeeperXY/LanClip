mod clipboard;
mod crypto;
mod model;
mod network;
mod secrets;
mod state;
mod system;

use clipboard::SystemClipboard;
use model::{AppSnapshot, PairingSession, SystemStatus};
use network::SharedState;
use rand::Rng;
use state::{now_millis, AppState};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, State,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tokio::sync::RwLock;

#[tauri::command]
async fn get_snapshot(state: State<'_, SharedState>) -> Result<AppSnapshot, String> {
    Ok(state.write().await.snapshot())
}

#[tauri::command]
async fn start_pairing(state: State<'_, SharedState>) -> Result<PairingSession, String> {
    let session = PairingSession {
        code: format!("{:06}", rand::rng().random_range(0..1_000_000)),
        expires_at: now_millis() + 120_000,
    };
    state.write().await.pairing = Some(session.clone());
    Ok(session)
}

#[tauri::command]
async fn cancel_pairing(state: State<'_, SharedState>) -> Result<(), String> {
    state.write().await.pairing = None;
    Ok(())
}

#[tauri::command]
async fn pair_device(
    state: State<'_, SharedState>,
    device_id: String,
    code: String,
) -> Result<(), String> {
    network::pair_with_device(state.inner().clone(), &device_id, code.trim()).await
}

#[tauri::command]
async fn set_sync_enabled(state: State<'_, SharedState>, enabled: bool) -> Result<(), String> {
    let mut state = state.write().await;
    state.config.sync_enabled = enabled;
    state.save()
}

#[tauri::command]
async fn rename_device(state: State<'_, SharedState>, name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("设备名称需为 1–40 个字符".into());
    }
    let mut state = state.write().await;
    state.config.device_name = name.to_string();
    state.save()
}

#[tauri::command]
async fn unpair_device(state: State<'_, SharedState>, device_id: String) -> Result<(), String> {
    let mut state = state.write().await;
    secrets::delete(&device_id)?;
    state.config.peers.remove(&device_id);
    state.save()
}

#[tauri::command]
async fn clear_history(state: State<'_, SharedState>) -> Result<(), String> {
    state.write().await.transfers.clear();
    Ok(())
}

#[tauri::command]
fn get_system_status(app: tauri::AppHandle) -> Result<SystemStatus, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(SystemStatus {
        autostart_enabled: app.autolaunch().is_enabled().map_err(|e| e.to_string())?,
        firewall_ready: system::firewall_ready(&executable),
        secure_storage: secrets::is_available(),
        installed_mode: system::installed_mode(&executable),
    })
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch().enable().map_err(|e| e.to_string())
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn repair_firewall() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    system::request_firewall_access(&executable)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("config.json");
            let state = Arc::new(RwLock::new(
                AppState::load(config_path)
                    .map_err(|error| format!("加载安全配置失败：{error}"))?,
            ));
            let clipboard = Arc::new(
                SystemClipboard::new().map_err(|error| format!("初始化剪贴板失败：{error}"))?,
            );
            network::start(state.clone(), clipboard);
            app.manage(state);

            let open = MenuItem::with_id(app, "open", "打开 LanClip", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("LanClip 局域网剪贴板")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            if std::env::args().any(|argument| argument == "--background") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            start_pairing,
            cancel_pairing,
            pair_device,
            set_sync_enabled,
            rename_device,
            unpair_device,
            clear_history,
            get_system_status,
            set_autostart,
            repair_firewall,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LanClip");
}
