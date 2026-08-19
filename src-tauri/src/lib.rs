mod clipboard;
mod crypto;
mod mirror;
mod model;
mod network;
mod secrets;
mod state;
mod system;

use clipboard::SystemClipboard;
use model::{AppSnapshot, MirrorDeviceView, PairingSession, SystemStatus, TrustedMirrorDevice};
use network::SharedState;
use rand::Rng;
use state::{now_millis, AppState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, State, WindowEvent,
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
        platform: current_platform().into(),
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

#[tauri::command]
fn get_mirror_status() -> Result<mirror::MirrorStatus, String> {
    mirror::status()
}

#[tauri::command]
async fn get_mirror_devices(
    state: State<'_, SharedState>,
) -> Result<Vec<MirrorDeviceView>, String> {
    let candidates = tauri::async_runtime::spawn_blocking(mirror::discover_devices)
        .await
        .map_err(|error| format!("发现安卓设备失败：{error}"))??;
    let now = now_millis();
    let mut state = state.write().await;
    let mut changed = false;
    for candidate in &candidates {
        if let Some(saved) = state.config.mirror_devices.get_mut(&candidate.device_id) {
            let endpoint_changed = saved.device_name != candidate.device_name
                || saved.last_address != candidate.address
                || saved.last_port != candidate.port;
            if endpoint_changed {
                saved.device_name = candidate.device_name.clone();
                saved.last_address = candidate.address.clone();
                saved.last_port = candidate.port;
                saved.last_seen = now;
                changed = true;
            }
        }
    }
    if changed {
        state.save()?;
    }

    let mut views = Vec::new();
    for saved in state.config.mirror_devices.values() {
        if let Some(candidate) = candidates
            .iter()
            .find(|candidate| candidate.device_id == saved.device_id)
        {
            views.push(MirrorDeviceView {
                device_id: saved.device_id.clone(),
                device_name: candidate.device_name.clone(),
                online: true,
                address: candidate.address.clone(),
                port: candidate.port,
                transport: candidate.transport.clone(),
                bound: true,
                last_seen: now,
            });
        } else {
            views.push(MirrorDeviceView {
                device_id: saved.device_id.clone(),
                device_name: saved.device_name.clone(),
                online: false,
                address: saved.last_address.clone(),
                port: saved.last_port,
                transport: if saved.last_address.is_some() {
                    "wifi".into()
                } else {
                    "usb".into()
                },
                bound: true,
                last_seen: saved.last_seen,
            });
        }
    }
    for candidate in candidates {
        if state
            .config
            .mirror_devices
            .contains_key(&candidate.device_id)
        {
            continue;
        }
        views.push(MirrorDeviceView {
            device_id: candidate.device_id,
            device_name: candidate.device_name,
            online: true,
            address: candidate.address,
            port: candidate.port,
            transport: candidate.transport,
            bound: false,
            last_seen: now,
        });
    }
    views.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then(b.bound.cmp(&a.bound))
            .then(a.device_name.cmp(&b.device_name))
    });
    Ok(views)
}

#[tauri::command]
async fn unbind_mirror_device(
    state: State<'_, SharedState>,
    device_id: String,
) -> Result<(), String> {
    let mut state = state.write().await;
    state.config.mirror_devices.remove(&device_id);
    state.save()
}

#[tauri::command]
async fn start_mirror(
    state: State<'_, SharedState>,
    device_id: Option<String>,
    address: Option<String>,
    port: Option<u16>,
    control: bool,
    mouse_mode: String,
    max_fps: Option<u16>,
    audio: Option<bool>,
    screen_off: Option<bool>,
) -> Result<(), String> {
    let max_fps = max_fps.unwrap_or(60);
    let audio = audio.unwrap_or(false);
    let screen_off = screen_off.unwrap_or(false);
    let remembered = {
        let state = state.read().await;
        device_id
            .as_deref()
            .and_then(|id| state.config.mirror_devices.get(id))
            .cloned()
    };
    let requested_address = if device_id.is_some() {
        remembered
            .as_ref()
            .and_then(|device| device.last_address.clone())
    } else {
        address
            .clone()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                remembered
                    .as_ref()
                    .and_then(|device| device.last_address.clone())
            })
    };
    let requested_port = port
        .or_else(|| remembered.as_ref().and_then(|device| device.last_port))
        .unwrap_or(5555);
    let discovery_address = requested_address.clone();
    let candidates = tauri::async_runtime::spawn_blocking(move || {
        if let Some(address) = discovery_address {
            let _ = mirror::connect_endpoint(&address, requested_port);
        }
        mirror::discover_devices()
    })
    .await
    .map_err(|error| format!("发现安卓设备失败：{error}"))?;
    let candidates = candidates.unwrap_or_default();

    let selected = if let Some(device_id) = device_id.as_deref() {
        candidates
            .iter()
            .find(|candidate| candidate.device_id == device_id)
            .cloned()
    } else if let Some(address) = address
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized_address = address.trim_matches(['[', ']']);
        candidates
            .iter()
            .find(|candidate| {
                candidate.address.as_deref() == Some(normalized_address)
                    && candidate.port == Some(requested_port)
            })
            .cloned()
            .or_else(|| {
                candidates
                    .iter()
                    .find(|candidate| candidate.address.as_deref() == Some(normalized_address))
                    .cloned()
            })
    } else {
        candidates
            .iter()
            .find(|candidate| candidate.transport == "usb")
            .cloned()
            .or_else(|| candidates.first().cloned())
    };

    if device_id.is_some() && selected.is_none() {
        return Err("已绑定的安卓设备当前不在线，请先连接手机并打开 USB/无线调试".into());
    }

    if let Some(candidate) = selected.as_ref() {
        mirror::start_selected(candidate, control, mouse_mode, max_fps, audio, screen_off)?;
        let now = now_millis();
        let mut state = state.write().await;
        state.config.mirror_devices.insert(
            candidate.device_id.clone(),
            TrustedMirrorDevice {
                device_id: candidate.device_id.clone(),
                device_name: candidate.device_name.clone(),
                last_address: candidate.address.clone(),
                last_port: candidate.port,
                last_seen: now,
            },
        );
        state.save()
    } else {
        mirror::start_with_fps(
            address,
            Some(requested_port),
            control,
            mouse_mode,
            max_fps,
            audio,
            screen_off,
        )
    }
}

#[tauri::command]
fn stop_mirror() -> Result<(), String> {
    mirror::stop()
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "unsupported"
    }
}

fn reveal_main_window(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let quitting = Arc::new(AtomicBool::new(false));
    let quitting_from_menu = quitting.clone();
    let quitting_from_window = quitting.clone();
    let quitting_from_app = quitting.clone();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .setup(move |app| {
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
            let quit = MenuItem::with_id(app, "quit", "退出 LanClip", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let tray_quitting = quitting_from_menu.clone();
            TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("LanClip 局域网剪贴板")
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" => {
                        reveal_main_window(app);
                    }
                    "quit" => {
                        tray_quitting.store(true, Ordering::SeqCst);
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    let should_open = matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } | TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        }
                    );
                    if should_open {
                        reveal_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            if std::env::args().any(|argument| argument == "--background") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
            Ok(())
        })
        .on_window_event(move |window, event| {
            if window.label() != "main" || quitting_from_window.load(Ordering::SeqCst) {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                #[cfg(target_os = "macos")]
                {
                    let _ = window
                        .app_handle()
                        .set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
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
            get_mirror_status,
            get_mirror_devices,
            unbind_mirror_device,
            start_mirror,
            stop_mirror,
        ])
        .build(tauri::generate_context!())
        .expect("error while building LanClip");

    app.run(move |_app, event| match event {
        RunEvent::ExitRequested { api, .. } => {
            if !quitting_from_app.load(Ordering::SeqCst) {
                api.prevent_exit();
            }
        }
        RunEvent::Exit => {
            let _ = mirror::stop();
        }
        _ => {}
    });
}
