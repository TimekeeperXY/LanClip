use crate::model::{
    AppSnapshot, DiscoveredDevice, PairingSession, PeerView, PersistedConfig, TransferRecord,
    ONLINE_TTL_MILLIS,
};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct AppState {
    pub config_path: PathBuf,
    pub config: PersistedConfig,
    pub discovered: HashMap<String, DiscoveredDevice>,
    pub pairing: Option<PairingSession>,
    pub transfers: VecDeque<TransferRecord>,
    pub last_observed_hash: Option<String>,
    pub last_remote_hash: Option<String>,
}

impl AppState {
    pub fn load(config_path: PathBuf) -> Result<Self, String> {
        let mut config = fs::read_to_string(&config_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_else(PersistedConfig::new);
        crate::secrets::load_and_migrate(&mut config)?;
        let state = Self {
            config_path,
            config,
            discovered: HashMap::new(),
            pairing: None,
            transfers: VecDeque::new(),
            last_observed_hash: None,
            last_remote_hash: None,
        };
        state.save()?;
        Ok(state)
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&self.config).map_err(|e| e.to_string())?;
        fs::write(&self.config_path, json).map_err(|e| e.to_string())
    }

    pub fn clean_expired(&mut self) {
        let now = now_millis();
        if self
            .pairing
            .as_ref()
            .is_some_and(|session| session.expires_at <= now)
        {
            self.pairing = None;
        }
        self.discovered
            .retain(|_, device| now.saturating_sub(device.last_seen) < 15_000);
    }

    pub fn snapshot(&mut self) -> AppSnapshot {
        self.clean_expired();
        let now = now_millis();
        let mut peers: Vec<_> = self
            .config
            .peers
            .values()
            .map(|peer| {
                let discovered = self.discovered.get(&peer.device_id);
                let recently_seen = peer
                    .last_seen
                    .is_some_and(|seen| now.saturating_sub(seen) < ONLINE_TTL_MILLIS);
                PeerView {
                    device_id: peer.device_id.clone(),
                    device_name: peer.device_name.clone(),
                    online: discovered.is_some_and(|device| {
                        now.saturating_sub(device.last_seen) < ONLINE_TTL_MILLIS
                    }) || recently_seen,
                    address: discovered
                        .map(|device| device.address.clone())
                        .or_else(|| peer.last_address.clone()),
                    paired_at: peer.paired_at,
                    last_seen: discovered.map(|device| device.last_seen).or(peer.last_seen),
                }
            })
            .collect();
        peers.sort_by(|a, b| {
            b.online
                .cmp(&a.online)
                .then(a.device_name.cmp(&b.device_name))
        });

        let mut discovered: Vec<_> = self
            .discovered
            .values()
            .filter(|device| !self.config.peers.contains_key(&device.device_id))
            .cloned()
            .collect();
        discovered.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));

        AppSnapshot {
            device_id: self.config.device_id.clone(),
            device_name: self.config.device_name.clone(),
            sync_enabled: self.config.sync_enabled,
            pairing: self.pairing.clone(),
            peers,
            discovered,
            transfers: self.transfers.iter().cloned().collect(),
        }
    }

    pub fn add_transfer(&mut self, transfer: TransferRecord) {
        self.transfers.push_front(transfer);
        self.transfers.truncate(30);
    }
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
