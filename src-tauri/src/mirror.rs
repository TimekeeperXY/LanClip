use serde::Serialize;
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
                PathBuf::from(local_app_data)
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
    let output = Command::new(path).arg("--version").output().ok()?;
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

pub fn start(address: Option<String>, port: Option<u16>, control: bool) -> Result<(), String> {
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
        "--max-fps=60".to_string(),
        "--no-audio".to_string(),
    ];
    if !control {
        args.push("--no-control".to_string());
    }
    if let Some(address) = address
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        if !valid_endpoint(&address) {
            return Err("安卓设备地址只能包含 IP、主机名和端口分隔符".into());
        }
        args.push(format!("--tcpip={address}:{}", port.unwrap_or(5555)));
    }

    let child = Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("启动 scrcpy 失败：{error}"))?;
    *slot = Some(child);
    Ok(())
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
