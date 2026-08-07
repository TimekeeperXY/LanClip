use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorStatus {
    pub available: bool,
    pub scrcpy_path: Option<String>,
    pub scrcpy_version: Option<String>,
    pub adb_available: bool,
    pub running: bool,
}

/// A currently connected Android device as reported by ADB.
///
/// `device_id` is intentionally derived from the device serial property rather than
/// its IP address.  IPs and wireless ADB ports can change whenever the phone joins a
/// different network, while the Android serial normally remains stable.
#[derive(Debug, Clone)]
pub struct MirrorCandidate {
    pub device_id: String,
    pub device_name: String,
    pub serial: String,
    pub address: Option<String>,
    pub port: Option<u16>,
    pub transport: String,
}

static SCRCPY_PROCESS: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

fn process_slot() -> &'static Mutex<Option<Child>> {
    SCRCPY_PROCESS.get_or_init(|| Mutex::new(None))
}

fn command_name(name: &str) -> &'static str {
    #[cfg(target_os = "windows")]
    {
        let _ = name;
        "where"
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = name;
        "which"
    }
}

fn resolve_tool(name: &str) -> Option<String> {
    if let Some(path) = bundled_tool_paths(name)
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
    {
        return Some(path);
    }

    if let Ok(output) = Command::new(command_name(name)).arg(name).output() {
        if output.status.success() {
            if let Some(path) = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(ToOwned::to_owned)
            {
                return Some(path);
            }
        }
    }

    common_tool_paths(name)
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
}

fn bundled_resources_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        #[cfg(target_os = "macos")]
        if let Some(contents_dir) = exe
            .parent()
            .and_then(|macos_dir| macos_dir.parent())
            .filter(|path| path.ends_with("Contents"))
        {
            let resources_dir = contents_dir.join("Resources");
            dirs.push(resources_dir.join("resources"));
            dirs.push(resources_dir);
        }
        #[cfg(target_os = "windows")]
        if let Some(exe_dir) = exe.parent() {
            dirs.push(exe_dir.join("resources"));
            dirs.push(exe_dir.to_path_buf());
        }
    }
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources"));
    dirs
}

fn bundled_tool_paths(name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(target_os = "macos")]
    {
        let relative = match name {
            // The Android platform-tools `adb` binary is a universal macOS binary,
            // so the existing aarch64 copy also works on Intel Macs. scrcpy itself
            // is architecture-specific and is selected below.
            "adb" => Some("android-tools/macos-aarch64/platform-tools/adb"),
            "scrcpy-server" => Some(if cfg!(target_arch = "x86_64") {
                "android-tools/macos-x86_64/scrcpy/scrcpy-server"
            } else {
                "android-tools/macos-aarch64/scrcpy/scrcpy-server"
            }),
            "scrcpy" => Some(if cfg!(target_arch = "x86_64") {
                "android-tools/macos-x86_64/scrcpy/scrcpy"
            } else {
                "android-tools/macos-aarch64/scrcpy/scrcpy"
            }),
            _ => None,
        };
        if let Some(relative) = relative {
            for resources_dir in bundled_resources_dirs() {
                paths.push(resources_dir.join(relative));
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let relative = match name {
            "adb" => Some("android-tools/windows-x86_64/platform-tools/adb.exe"),
            "scrcpy" => Some("android-tools/windows-x86_64/scrcpy/scrcpy.exe"),
            "scrcpy-server" => Some("android-tools/windows-x86_64/scrcpy/scrcpy-server"),
            _ => None,
        };
        if let Some(relative) = relative {
            for resources_dir in bundled_resources_dirs() {
                paths.push(resources_dir.join(relative));
            }
        }
    }
    paths
}

fn common_tool_paths(name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(target_os = "macos")]
    {
        paths.push(PathBuf::from(format!("/opt/homebrew/bin/{name}")));
        paths.push(PathBuf::from(format!("/usr/local/bin/{name}")));
        paths.push(PathBuf::from(format!("/opt/local/bin/{name}")));
    }
    #[cfg(target_os = "windows")]
    {
        let executable = format!("{name}.exe");
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            paths.push(
                PathBuf::from(&local_app_data)
                    .join("scrcpy")
                    .join(&executable),
            );
            paths.push(
                PathBuf::from(&local_app_data)
                    .join("Programs")
                    .join("scrcpy")
                    .join(&executable),
            );
        }
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            paths.push(
                PathBuf::from(program_files)
                    .join("scrcpy")
                    .join(&executable),
            );
        }
    }
    paths
}

fn tool_version(path: &str) -> Option<String> {
    let mut command = Command::new(path);
    add_android_tool_env(&mut command);
    let output = command.arg("--version").output().ok()?;
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr).into_owned()
    } else {
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

fn adb_path() -> Result<String, String> {
    resolve_tool("adb").ok_or_else(|| {
        "未找到 adb。请先安装 Android platform-tools，并将 adb 加入 PATH 后重启 LanClip。".into()
    })
}

fn run_adb(path: &str, args: &[String]) -> Result<std::process::Output, String> {
    let mut command = Command::new(path);
    add_android_tool_env(&mut command);
    command
        .args(args)
        .output()
        .map_err(|error| format!("执行 adb 失败：{error}"))
}

fn add_android_tool_env(command: &mut Command) {
    if let Some(adb) = resolve_tool("adb") {
        command.env("ADB", adb);
    }
    if let Some(server) = bundled_tool_paths("scrcpy-server")
        .into_iter()
        .find(|path| path.is_file())
    {
        command.env("SCRCPY_SERVER_PATH", server);
    }
}

fn split_endpoint(value: &str) -> Option<(String, u16)> {
    let (address, port) = value.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    if address.is_empty() {
        return None;
    }
    Some((address.trim_matches(['[', ']']).to_string(), port))
}

fn mdns_endpoints(path: &str) -> Result<Vec<(String, u16)>, String> {
    let output = run_adb(path, &["mdns".into(), "services".into()])?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut endpoints = Vec::new();
    for line in text.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.len() < 3 || !parts[1].contains("_adb") {
            continue;
        }
        if let Some(endpoint) = split_endpoint(parts[2]) {
            if !endpoints.contains(&endpoint) {
                endpoints.push(endpoint);
            }
        }
    }
    Ok(endpoints)
}

fn connect_endpoint_with_path(path: &str, address: &str, port: u16) -> Result<(), String> {
    if !valid_endpoint(address) {
        return Err("安卓设备地址只能包含 IP、主机名和端口分隔符".into());
    }
    let endpoint = format!("{address}:{port}");
    let output = run_adb(path, &["connect".into(), endpoint.clone()])?;
    if output.status.success() {
        Ok(())
    } else {
        let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if details.is_empty() {
            format!("无法连接安卓设备 {endpoint}")
        } else {
            format!("无法连接安卓设备 {endpoint}：{details}")
        })
    }
}

/// Connect to a manually entered ADB endpoint.  This is also used as a fallback
/// before refreshing the remembered-device list.
pub fn connect_endpoint(address: &str, port: u16) -> Result<(), String> {
    let path = adb_path()?;
    connect_endpoint_with_path(&path, address.trim(), port)
}

fn getprop(path: &str, serial: &str, property: &str) -> Option<String> {
    let args = vec![
        "-s".to_string(),
        serial.to_string(),
        "shell".to_string(),
        "getprop".to_string(),
        property.to_string(),
    ];
    let output = run_adb(path, &args).ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches(|character| character == '\r' || character == '\n')
        .to_string();
    if value.is_empty() || matches!(value.to_ascii_lowercase().as_str(), "unknown" | "?" | "0") {
        None
    } else {
        Some(value)
    }
}

fn parse_model(parts: &[&str]) -> Option<String> {
    parts
        .iter()
        .find_map(|part| part.strip_prefix("model:"))
        .filter(|model| !model.is_empty())
        .map(|model| model.replace('_', " "))
}

fn parse_devices(path: &str, text: &str) -> Vec<MirrorCandidate> {
    let mut devices: HashMap<String, MirrorCandidate> = HashMap::new();
    for line in text.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.len() < 2 || parts[1] != "device" {
            continue;
        }
        let serial = parts[0].to_string();
        let transport = if serial.contains(':') {
            "wifi".to_string()
        } else {
            "usb".to_string()
        };
        let (address, port) = split_endpoint(&serial)
            .map_or((None, None), |(address, port)| (Some(address), Some(port)));
        let stable_id = getprop(path, &serial, "ro.serialno")
            .or_else(|| getprop(path, &serial, "ro.boot.serialno"))
            .unwrap_or_else(|| serial.clone());
        let device_name = getprop(path, &serial, "ro.product.model")
            .or_else(|| parse_model(&parts))
            .unwrap_or_else(|| "安卓设备".to_string());
        let candidate = MirrorCandidate {
            device_id: stable_id.clone(),
            device_name,
            serial,
            address,
            port,
            transport: transport.clone(),
        };

        // If a phone is connected over USB and Wi-Fi at the same time, expose it
        // once. Prefer the Wi-Fi transport so remembered devices keep working after
        // the cable is removed, while USB remains the fallback when Wi-Fi is absent.
        match devices.get(&stable_id) {
            Some(existing) if existing.transport == "wifi" && transport == "usb" => {}
            _ => {
                devices.insert(stable_id, candidate);
            }
        }
    }
    let mut devices: Vec<_> = devices.into_values().collect();
    devices.sort_by(|a, b| a.device_name.cmp(&b.device_name));
    devices
}

/// Discover connected ADB devices and refresh paired wireless endpoints through
/// Android's ADB mDNS services.  Pairing itself remains a one-time Android 11+
/// step (`adb pair`); once paired, mDNS advertises the changing connection port.
pub fn discover_devices() -> Result<Vec<MirrorCandidate>, String> {
    let path = adb_path()?;
    if let Ok(endpoints) = mdns_endpoints(&path) {
        for (address, port) in endpoints {
            let _ = connect_endpoint_with_path(&path, &address, port);
        }
    }
    let output = run_adb(&path, &["devices".into(), "-l".into()])?;
    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if details.is_empty() {
            "读取 adb 设备列表失败".into()
        } else {
            format!("读取 adb 设备列表失败：{details}")
        });
    }
    Ok(parse_devices(
        &path,
        &String::from_utf8_lossy(&output.stdout),
    ))
}

fn running() -> Result<bool, String> {
    let mut slot = process_slot()
        .lock()
        .map_err(|_| "投屏进程状态不可用".to_string())?;
    let Some(child) = slot.as_mut() else {
        return Ok(false);
    };
    match child
        .try_wait()
        .map_err(|error| format!("读取投屏进程状态失败：{error}"))?
    {
        Some(_) => {
            *slot = None;
            Ok(false)
        }
        None => Ok(true),
    }
}

pub fn status() -> Result<MirrorStatus, String> {
    let scrcpy_path = resolve_tool("scrcpy");
    let adb_available = resolve_tool("adb").is_some();
    Ok(MirrorStatus {
        available: scrcpy_path.is_some(),
        scrcpy_version: scrcpy_path.as_deref().and_then(tool_version),
        scrcpy_path,
        adb_available,
        running: running()?,
    })
}

fn valid_endpoint(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".:-_[]".contains(character))
}

fn start_process(
    target_serial: Option<&str>,
    address: Option<String>,
    port: Option<u16>,
    control: bool,
    mouse_mode: String,
    max_fps: u16,
    audio: bool,
    screen_off: bool,
) -> Result<(), String> {
    if !(15..=240).contains(&max_fps) {
        return Err("投屏帧率需在 15–240 FPS 之间".into());
    }
    let path = resolve_tool("scrcpy").ok_or_else(|| {
        "未找到 scrcpy。请先安装官方 scrcpy，并将其加入 PATH 后重启 LanClip。".to_string()
    })?;

    let mut slot = process_slot()
        .lock()
        .map_err(|_| "投屏进程状态不可用".to_string())?;
    if let Some(child) = slot.as_mut() {
        if child
            .try_wait()
            .map_err(|error| format!("读取已有投屏进程失败：{error}"))?
            .is_none()
        {
            return Err("已有一个安卓投屏会话正在运行".into());
        }
        *slot = None;
    }

    let mut args = vec![
        "--window-title=LanClip · 安卓投屏".to_string(),
        "--max-size=1920".to_string(),
        format!("--max-fps={max_fps}"),
    ];
    if audio {
        // The `output` source captures the complete device output and, unlike
        // `playback`/`--audio-dup`, disables playback on the Android speaker.
        args.push("--audio-source=output".to_string());
    } else {
        args.push("--no-audio".to_string());
    }
    if screen_off {
        // Keep mirroring while powering off the Android display. This is not the
        // same as showing the Android lock screen; it is the scrcpy best-effort
        // display power-off mode.
        args.push("--turn-screen-off".to_string());
    }
    if !control {
        args.push("--no-control".to_string());
    } else {
        match mouse_mode.as_str() {
            "sdk" | "uhid" => args.push(format!("--mouse={mouse_mode}")),
            _ => return Err("未知的鼠标控制模式".into()),
        }
        if mouse_mode == "uhid" {
            args.push("--keyboard=uhid".to_string());
        }
    }
    if let Some(serial) = target_serial {
        args.push(format!("--serial={serial}"));
    } else if let Some(address) = address
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        if !valid_endpoint(&address) {
            return Err("安卓设备地址只能包含 IP、主机名和端口分隔符".into());
        }
        args.push(format!("--tcpip=+{address}:{}", port.unwrap_or(5555)));
    } else {
        // When both USB and wireless ADB are connected, scrcpy otherwise reports
        // multiple devices. The desktop mirror action without an address means USB.
        args.push("--select-usb".to_string());
    }

    let mut command = Command::new(path);
    add_android_tool_env(&mut command);
    let child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("启动 scrcpy 失败：{error}"))?;
    *slot = Some(child);
    Ok(())
}

pub fn start_with_fps(
    address: Option<String>,
    port: Option<u16>,
    control: bool,
    mouse_mode: String,
    max_fps: u16,
    audio: bool,
    screen_off: bool,
) -> Result<(), String> {
    start_process(
        None, address, port, control, mouse_mode, max_fps, audio, screen_off,
    )
}

pub fn start_selected(
    candidate: &MirrorCandidate,
    control: bool,
    mouse_mode: String,
    max_fps: u16,
    audio: bool,
    screen_off: bool,
) -> Result<(), String> {
    start_process(
        Some(&candidate.serial),
        None,
        None,
        control,
        mouse_mode,
        max_fps,
        audio,
        screen_off,
    )
}

pub fn stop() -> Result<(), String> {
    let mut slot = process_slot()
        .lock()
        .map_err(|_| "投屏进程状态不可用".to_string())?;
    if let Some(mut child) = slot.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::valid_endpoint;

    #[test]
    fn endpoint_accepts_ip_and_hostname_characters() {
        assert!(valid_endpoint("192.168.1.25"));
        assert!(valid_endpoint("phone.local"));
        assert!(valid_endpoint("[fe80::1]"));
    }

    #[test]
    fn endpoint_rejects_shell_like_input() {
        assert!(!valid_endpoint("192.168.1.25;whoami"));
        assert!(!valid_endpoint("$(whoami)"));
        assert!(!valid_endpoint(""));
    }
}
