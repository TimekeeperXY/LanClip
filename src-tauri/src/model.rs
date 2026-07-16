use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DISCOVERY_PORT: u16 = 44777;
pub const TRANSFER_PORT: u16 = 44778;
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
pub const ONLINE_TTL_MILLIS: u64 = 8_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedConfig {
    pub device_id: String,
    pub device_name: String,
    pub sync_enabled: bool,
    pub peers: HashMap<String, TrustedPeer>,
}

impl PersistedConfig {
    pub fn new() -> Self {
        let computer = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "这台电脑".into());
        Self {
            device_id: uuid::Uuid::new_v4().to_string(),
            device_name: computer,
            sync_enabled: true,
            peers: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedPeer {
    pub device_id: String,
    pub device_name: String,
    #[serde(default, skip_serializing)]
    pub shared_key: String,
    pub paired_at: u64,
    pub last_seen: Option<u64>,
    #[serde(default)]
    pub last_address: Option<String>,
    #[serde(default)]
    pub last_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredDevice {
    pub device_id: String,
    pub device_name: String,
    pub address: String,
    pub port: u16,
    pub pairing: bool,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingSession {
    pub code: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferRecord {
    pub id: String,
    pub direction: String,
    pub peer_name: String,
    pub content_type: String,
    pub preview: String,
    pub byte_size: usize,
    pub created_at: u64,
    pub success: bool,
    #[serde(default)]
    pub thumbnail_data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClipboardContent {
    Text {
        text: String,
    },
    Image {
        width: usize,
        height: usize,
        rgba_base64: String,
    },
}

impl ClipboardContent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::Image { .. } => "image",
        }
    }

    pub fn byte_size(&self) -> usize {
        match self {
            Self::Text { text } => text.len(),
            Self::Image { rgba_base64, .. } => rgba_base64.len() * 3 / 4,
        }
    }

    pub fn preview(&self) -> String {
        match self {
            Self::Text { text } => {
                let compact = text.replace(['\r', '\n'], " ");
                compact.chars().take(72).collect()
            }
            Self::Image { width, height, .. } => format!("图片 {} × {}", width, height),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryBeacon {
    pub protocol: u8,
    pub device_id: String,
    pub device_name: String,
    pub port: u16,
    pub pairing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WireMessage {
    PairRequest {
        device_id: String,
        device_name: String,
        proof: String,
    },
    PairAccepted {
        device_id: String,
        device_name: String,
        nonce: String,
        ciphertext: String,
    },
    PairRejected {
        reason: String,
    },
    Push {
        device_id: String,
        nonce: String,
        ciphertext: String,
    },
    Ping {
        device_id: String,
        nonce: String,
        ciphertext: String,
    },
    Ack,
    Error {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub device_id: String,
    pub device_name: String,
    pub sync_enabled: bool,
    pub pairing: Option<PairingSession>,
    pub peers: Vec<PeerView>,
    pub discovered: Vec<DiscoveredDevice>,
    pub transfers: Vec<TransferRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub autostart_enabled: bool,
    pub firewall_ready: bool,
    pub secure_storage: bool,
    pub installed_mode: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerView {
    pub device_id: String,
    pub device_name: String,
    pub online: bool,
    pub address: Option<String>,
    pub paired_at: u64,
    pub last_seen: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_peer_from_previous_version_remains_compatible() {
        let json = r#"{
            "deviceId":"peer-1",
            "deviceName":"旧设备",
            "sharedKey":"key",
            "pairedAt":1,
            "lastSeen":null
        }"#;
        let peer: TrustedPeer = serde_json::from_str(json).unwrap();
        assert_eq!(peer.device_id, "peer-1");
        assert_eq!(peer.last_address, None);
        assert_eq!(peer.last_port, None);
    }

    #[test]
    fn trusted_peer_secret_is_never_serialized() {
        let peer = TrustedPeer {
            device_id: "peer-1".into(),
            device_name: "设备".into(),
            shared_key: "must-not-leak".into(),
            paired_at: 1,
            last_seen: None,
            last_address: None,
            last_port: None,
        };
        let json = serde_json::to_string(&peer).unwrap();
        assert!(!json.contains("must-not-leak"));
        assert!(!json.contains("sharedKey"));
    }
}
