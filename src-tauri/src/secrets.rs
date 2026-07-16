use crate::model::PersistedConfig;

const SERVICE: &str = "app.lanclip.desktop";

fn entry(device_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, &format!("peer:{device_id}")).map_err(|e| e.to_string())
}

pub fn load_and_migrate(config: &mut PersistedConfig) -> Result<bool, String> {
    let mut migrated = false;
    for peer in config.peers.values_mut() {
        let credential = entry(&peer.device_id)?;
        if !peer.shared_key.is_empty() {
            credential
                .set_password(&peer.shared_key)
                .map_err(|e| format!("保存设备 {} 的系统凭据失败：{e}", peer.device_name))?;
            migrated = true;
        } else {
            peer.shared_key = credential
                .get_password()
                .map_err(|e| format!("读取设备 {} 的系统凭据失败：{e}", peer.device_name))?;
        }
    }
    Ok(migrated)
}

pub fn store(device_id: &str, secret: &str) -> Result<(), String> {
    entry(device_id)?
        .set_password(secret)
        .map_err(|e| format!("写入系统安全凭据失败：{e}"))
}

pub fn delete(device_id: &str) -> Result<(), String> {
    match entry(device_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("删除系统安全凭据失败：{error}")),
    }
}

pub fn is_available() -> bool {
    let account = format!("healthcheck:{}", std::process::id());
    let Ok(entry) = keyring::Entry::new(SERVICE, &account) else {
        return false;
    };
    if entry.set_password("ok").is_err() {
        return false;
    }
    let available = entry.get_password().is_ok_and(|value| value == "ok");
    let _ = entry.delete_credential();
    available
}
