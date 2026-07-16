use crate::{
    clipboard::{thumbnail_data_url, SystemClipboard},
    crypto::{
        decode_key, decrypt, encode_key, encrypt, pairing_key, pairing_proof, random_key,
        verify_pairing_proof,
    },
    model::{
        ClipboardContent, DiscoveredDevice, DiscoveryBeacon, TransferRecord, TrustedPeer,
        WireMessage, DISCOVERY_PORT, TRANSFER_PORT,
    },
    state::{now_millis, AppState},
};
use std::{io, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::RwLock,
    time::{sleep, timeout, Duration},
};

pub type SharedState = Arc<RwLock<AppState>>;
const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;
const HEARTBEAT_PAYLOAD: &[u8] = b"lanclip-heartbeat-v1";

pub fn start(state: SharedState, clipboard: Arc<SystemClipboard>) {
    tauri::async_runtime::spawn(discovery_sender(state.clone()));
    tauri::async_runtime::spawn(discovery_receiver(state.clone()));
    tauri::async_runtime::spawn(transfer_listener(state.clone(), clipboard.clone()));
    tauri::async_runtime::spawn(heartbeat_monitor(state.clone()));
    tauri::async_runtime::spawn(clipboard_monitor(state, clipboard));
}

async fn discovery_sender(state: SharedState) {
    let socket = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(socket) => socket,
        Err(_) => return,
    };
    let _ = socket.set_broadcast(true);
    loop {
        let beacon = {
            let mut state = state.write().await;
            state.clean_expired();
            DiscoveryBeacon {
                protocol: 1,
                device_id: state.config.device_id.clone(),
                device_name: state.config.device_name.clone(),
                port: TRANSFER_PORT,
                pairing: state.pairing.is_some(),
            }
        };
        if let Ok(payload) = serde_json::to_vec(&beacon) {
            let _ = socket
                .send_to(&payload, (std::net::Ipv4Addr::BROADCAST, DISCOVERY_PORT))
                .await;
        }
        sleep(Duration::from_secs(2)).await;
    }
}

async fn discovery_receiver(state: SharedState) {
    let socket = match UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT)).await {
        Ok(socket) => socket,
        Err(_) => return,
    };
    let mut buffer = [0u8; 4096];
    loop {
        let Ok((length, sender)) = socket.recv_from(&mut buffer).await else {
            continue;
        };
        let Ok(beacon) = serde_json::from_slice::<DiscoveryBeacon>(&buffer[..length]) else {
            continue;
        };
        let mut state = state.write().await;
        if beacon.protocol != 1 || beacon.device_id == state.config.device_id {
            continue;
        }
        let now = now_millis();
        let address = sender.ip().to_string();
        state.discovered.insert(
            beacon.device_id.clone(),
            DiscoveredDevice {
                device_id: beacon.device_id.clone(),
                device_name: beacon.device_name.clone(),
                address: address.clone(),
                port: beacon.port,
                pairing: beacon.pairing,
                last_seen: now,
            },
        );
        let mut endpoint_changed = false;
        if let Some(peer) = state.config.peers.get_mut(&beacon.device_id) {
            endpoint_changed = peer.last_address.as_deref() != Some(&address)
                || peer.last_port != Some(beacon.port);
            peer.device_name = beacon.device_name;
            peer.last_seen = Some(now);
            peer.last_address = Some(address);
            peer.last_port = Some(beacon.port);
        }
        if endpoint_changed {
            let _ = state.save();
        }
    }
}

async fn transfer_listener(state: SharedState, clipboard: Arc<SystemClipboard>) {
    let listener = match TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, TRANSFER_PORT)).await {
        Ok(listener) => listener,
        Err(_) => return,
    };
    loop {
        let Ok((stream, remote_address)) = listener.accept().await else {
            continue;
        };
        let state = state.clone();
        let clipboard = clipboard.clone();
        tauri::async_runtime::spawn(async move {
            let _ = handle_connection(stream, remote_address, state, clipboard).await;
        });
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    remote_address: std::net::SocketAddr,
    state: SharedState,
    clipboard: Arc<SystemClipboard>,
) -> Result<(), String> {
    let message = read_frame(&mut stream).await.map_err(|e| e.to_string())?;
    match message {
        WireMessage::PairRequest {
            device_id,
            device_name,
            proof,
        } => {
            let response = {
                let mut state = state.write().await;
                state.clean_expired();
                if state.pairing.is_none() {
                    write_frame(
                        &mut stream,
                        &WireMessage::PairRejected {
                            reason: "配对码已过期".into(),
                        },
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    return Ok(());
                }

                let session = state.pairing.clone().expect("checked above");
                let pair_key = pairing_key(&session.code, &state.config.device_id);
                if !verify_pairing_proof(&pair_key, &device_id, &device_name, &proof) {
                    WireMessage::PairRejected {
                        reason: "配对码不正确".into(),
                    }
                } else {
                    let shared_key = random_key();
                    let encoded_key = encode_key(&shared_key);
                    state.config.peers.insert(
                        device_id.clone(),
                        TrustedPeer {
                            device_id: device_id.clone(),
                            device_name: device_name.clone(),
                            shared_key: encoded_key.clone(),
                            paired_at: now_millis(),
                            last_seen: Some(now_millis()),
                            last_address: Some(remote_address.ip().to_string()),
                            last_port: Some(TRANSFER_PORT),
                        },
                    );
                    crate::secrets::store(&device_id, &encoded_key)?;
                    state.pairing = None;
                    let _ = state.save();
                    let (nonce, ciphertext) = encrypt(&pair_key, encoded_key.as_bytes())?;
                    WireMessage::PairAccepted {
                        device_id: state.config.device_id.clone(),
                        device_name: state.config.device_name.clone(),
                        nonce,
                        ciphertext,
                    }
                }
            };
            write_frame(&mut stream, &response)
                .await
                .map_err(|e| e.to_string())?;
        }
        WireMessage::Push {
            device_id,
            nonce,
            ciphertext,
        } => {
            let (key, peer_name, enabled) = {
                let state = state.read().await;
                let Some(peer) = state.config.peers.get(&device_id) else {
                    write_frame(
                        &mut stream,
                        &WireMessage::Error {
                            reason: "设备未受信任".into(),
                        },
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    return Ok(());
                };
                (
                    decode_key(&peer.shared_key)?,
                    peer.device_name.clone(),
                    state.config.sync_enabled,
                )
            };
            if !enabled {
                write_frame(
                    &mut stream,
                    &WireMessage::Error {
                        reason: "接收端已暂停同步".into(),
                    },
                )
                .await
                .map_err(|e| e.to_string())?;
                return Ok(());
            }
            let plaintext = decrypt(&key, &nonce, &ciphertext)?;
            let content: ClipboardContent =
                serde_json::from_slice(&plaintext).map_err(|e| e.to_string())?;
            let thumbnail_data_url = thumbnail_data_url(&content);
            let hash = clipboard.write(&content)?;
            let mut state = state.write().await;
            let endpoint_changed = remember_peer_endpoint(
                &mut state,
                &device_id,
                &peer_name,
                remote_address.ip().to_string(),
                TRANSFER_PORT,
            );
            state.last_remote_hash = Some(hash.clone());
            state.last_observed_hash = Some(hash);
            state.add_transfer(TransferRecord {
                id: uuid::Uuid::new_v4().to_string(),
                direction: "received".into(),
                peer_name,
                content_type: content.kind().into(),
                preview: content.preview(),
                byte_size: content.byte_size(),
                created_at: now_millis(),
                success: true,
                thumbnail_data_url,
            });
            if endpoint_changed {
                let _ = state.save();
            }
            drop(state);
            write_frame(&mut stream, &WireMessage::Ack)
                .await
                .map_err(|e| e.to_string())?;
        }
        WireMessage::Ping {
            device_id,
            nonce,
            ciphertext,
        } => {
            let (key, peer_name) = {
                let state = state.read().await;
                let Some(peer) = state.config.peers.get(&device_id) else {
                    write_frame(
                        &mut stream,
                        &WireMessage::Error {
                            reason: "设备未受信任".into(),
                        },
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    return Ok(());
                };
                (decode_key(&peer.shared_key)?, peer.device_name.clone())
            };
            let plaintext = decrypt(&key, &nonce, &ciphertext)?;
            if !is_heartbeat_payload(&plaintext) {
                write_frame(
                    &mut stream,
                    &WireMessage::Error {
                        reason: "无效的心跳消息".into(),
                    },
                )
                .await
                .map_err(|e| e.to_string())?;
                return Ok(());
            }
            let mut state = state.write().await;
            let endpoint_changed = remember_peer_endpoint(
                &mut state,
                &device_id,
                &peer_name,
                remote_address.ip().to_string(),
                TRANSFER_PORT,
            );
            if endpoint_changed {
                let _ = state.save();
            }
            drop(state);
            write_frame(&mut stream, &WireMessage::Ack)
                .await
                .map_err(|e| e.to_string())?;
        }
        _ => {
            write_frame(
                &mut stream,
                &WireMessage::Error {
                    reason: "不支持的消息".into(),
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

async fn heartbeat_monitor(state: SharedState) {
    sleep(Duration::from_secs(1)).await;
    loop {
        let (local_device_id, targets) = {
            let state = state.read().await;
            (
                state.config.device_id.clone(),
                state
                    .config
                    .peers
                    .values()
                    .filter_map(|peer| {
                        let (address, port) =
                            resolve_peer_endpoint(peer, state.discovered.get(&peer.device_id))?;
                        Some((peer.clone(), address, port))
                    })
                    .collect::<Vec<_>>(),
            )
        };

        for (peer, address, port) in targets {
            if ping_peer(&local_device_id, &peer, &address, port)
                .await
                .is_ok()
            {
                let mut state = state.write().await;
                let endpoint_changed = remember_peer_endpoint(
                    &mut state,
                    &peer.device_id,
                    &peer.device_name,
                    address,
                    port,
                );
                if endpoint_changed {
                    let _ = state.save();
                }
            }
        }
        sleep(Duration::from_secs(3)).await;
    }
}

async fn ping_peer(
    local_device_id: &str,
    peer: &TrustedPeer,
    address: &str,
    port: u16,
) -> Result<(), String> {
    let key = decode_key(&peer.shared_key)?;
    let (nonce, ciphertext) = encrypt(&key, HEARTBEAT_PAYLOAD)?;
    let mut stream = timeout(
        Duration::from_millis(1_500),
        TcpStream::connect(format!("{address}:{port}")),
    )
    .await
    .map_err(|_| "心跳连接超时".to_string())?
    .map_err(|e| e.to_string())?;
    write_frame(
        &mut stream,
        &WireMessage::Ping {
            device_id: local_device_id.to_string(),
            nonce,
            ciphertext,
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    match timeout(Duration::from_millis(1_500), read_frame(&mut stream))
        .await
        .map_err(|_| "心跳响应超时".to_string())?
        .map_err(|e| e.to_string())?
    {
        WireMessage::Ack => Ok(()),
        WireMessage::Error { reason } => Err(reason),
        _ => Err("设备返回了无效心跳响应".into()),
    }
}

fn is_heartbeat_payload(payload: &[u8]) -> bool {
    payload == HEARTBEAT_PAYLOAD
}

pub async fn pair_with_device(
    state: SharedState,
    target_device_id: &str,
    code: &str,
) -> Result<(), String> {
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Err("请输入 6 位数字配对码".into());
    }
    let (target, local_id, local_name) = {
        let state = state.read().await;
        let target = state
            .discovered
            .get(target_device_id)
            .cloned()
            .ok_or_else(|| "目标设备已离线，请稍后重试".to_string())?;
        (
            target,
            state.config.device_id.clone(),
            state.config.device_name.clone(),
        )
    };
    if !target.pairing {
        return Err("请先在目标设备上点击“允许新设备”".into());
    }

    let pair_key = pairing_key(code, &target.device_id);
    let proof = pairing_proof(&pair_key, &local_id, &local_name);
    let request = WireMessage::PairRequest {
        device_id: local_id,
        device_name: local_name,
        proof,
    };
    let address = format!("{}:{}", target.address, target.port);
    let mut stream = timeout(Duration::from_secs(5), TcpStream::connect(address))
        .await
        .map_err(|_| "连接目标设备超时".to_string())?
        .map_err(|e| e.to_string())?;
    write_frame(&mut stream, &request)
        .await
        .map_err(|e| e.to_string())?;
    let response = timeout(Duration::from_secs(5), read_frame(&mut stream))
        .await
        .map_err(|_| "目标设备没有响应".to_string())?
        .map_err(|e| e.to_string())?;

    match response {
        WireMessage::PairAccepted {
            device_id,
            device_name,
            nonce,
            ciphertext,
        } => {
            let shared_key = String::from_utf8(decrypt(&pair_key, &nonce, &ciphertext)?)
                .map_err(|e| e.to_string())?;
            decode_key(&shared_key)?;
            let mut state = state.write().await;
            state.config.peers.insert(
                device_id.clone(),
                TrustedPeer {
                    device_id: device_id.clone(),
                    device_name,
                    shared_key: shared_key.clone(),
                    paired_at: now_millis(),
                    last_seen: Some(now_millis()),
                    last_address: Some(target.address),
                    last_port: Some(target.port),
                },
            );
            crate::secrets::store(&device_id, &shared_key)?;
            state.save()
        }
        WireMessage::PairRejected { reason } | WireMessage::Error { reason } => Err(reason),
        _ => Err("目标设备返回了无效响应".into()),
    }
}

async fn clipboard_monitor(state: SharedState, clipboard: Arc<SystemClipboard>) {
    let mut initialized = false;
    let mut last_sequence = clipboard.sequence_number();
    loop {
        sleep(Duration::from_millis(250)).await;
        let sequence = clipboard.sequence_number();
        let sequence_changed = sequence != 0 && sequence != last_sequence;

        // On Windows the sequence number is authoritative. Avoid repeatedly opening the
        // clipboard while it is unchanged, which can race with delayed-rendering tools.
        if initialized && sequence != 0 && !sequence_changed {
            continue;
        }
        if sequence_changed {
            sleep(Duration::from_millis(40)).await;
        }

        let mut observed = clipboard.read();
        let image_announced = clipboard.has_image_format();
        let image_not_ready = image_announced
            && observed
                .as_ref()
                .is_some_and(|(content, _)| matches!(content, ClipboardContent::Text { .. }));
        if sequence_changed && (observed.is_none() || image_not_ready) {
            for _ in 0..5 {
                sleep(Duration::from_millis(80)).await;
                observed = clipboard.read();
                if observed
                    .as_ref()
                    .is_some_and(|(content, _)| matches!(content, ClipboardContent::Image { .. }))
                    || (!image_announced && observed.is_some())
                {
                    break;
                }
            }
        }
        let Some((content, hash)) = observed else {
            continue;
        };
        if image_announced && matches!(content, ClipboardContent::Text { .. }) {
            continue;
        }
        if sequence != 0 {
            last_sequence = sequence;
        }

        let (local_device_id, targets) = {
            let mut state = state.write().await;
            if !initialized {
                initialized = true;
                state.last_observed_hash = Some(hash);
                continue;
            }
            if state.last_observed_hash.as_deref() == Some(&hash) {
                continue;
            }
            state.last_observed_hash = Some(hash.clone());
            if state.last_remote_hash.as_deref() == Some(&hash) {
                state.last_remote_hash = None;
                continue;
            }
            if !state.config.sync_enabled {
                continue;
            }
            (
                state.config.device_id.clone(),
                state
                    .config
                    .peers
                    .values()
                    .filter_map(|peer| {
                        let (address, port) =
                            resolve_peer_endpoint(peer, state.discovered.get(&peer.device_id))?;
                        Some((peer.clone(), address, port))
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let thumbnail_data_url = thumbnail_data_url(&content);

        for (peer, address, port) in targets {
            let mut result = send_content(&local_device_id, &peer, &address, port, &content).await;
            if result.is_err() {
                sleep(Duration::from_millis(350)).await;
                result = send_content(&local_device_id, &peer, &address, port, &content).await;
            }
            let mut state = state.write().await;
            if result.is_ok() {
                let endpoint_changed = remember_peer_endpoint(
                    &mut state,
                    &peer.device_id,
                    &peer.device_name,
                    address.clone(),
                    port,
                );
                if endpoint_changed {
                    let _ = state.save();
                }
            }
            state.add_transfer(TransferRecord {
                id: uuid::Uuid::new_v4().to_string(),
                direction: "sent".into(),
                peer_name: peer.device_name,
                content_type: content.kind().into(),
                preview: content.preview(),
                byte_size: content.byte_size(),
                created_at: now_millis(),
                success: result.is_ok(),
                thumbnail_data_url: thumbnail_data_url.clone(),
            });
        }
    }
}

fn resolve_peer_endpoint(
    peer: &TrustedPeer,
    discovered: Option<&DiscoveredDevice>,
) -> Option<(String, u16)> {
    if let Some(device) = discovered {
        return Some((device.address.clone(), device.port));
    }
    Some((
        peer.last_address.clone()?,
        peer.last_port.unwrap_or(TRANSFER_PORT),
    ))
}

fn remember_peer_endpoint(
    state: &mut AppState,
    device_id: &str,
    device_name: &str,
    address: String,
    port: u16,
) -> bool {
    let now = now_millis();
    let mut endpoint_changed = false;
    if let Some(peer) = state.config.peers.get_mut(device_id) {
        endpoint_changed =
            peer.last_address.as_deref() != Some(&address) || peer.last_port != Some(port);
        peer.device_name = device_name.to_string();
        peer.last_seen = Some(now);
        peer.last_address = Some(address.clone());
        peer.last_port = Some(port);
    }
    state.discovered.insert(
        device_id.to_string(),
        DiscoveredDevice {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            address,
            port,
            pairing: false,
            last_seen: now,
        },
    );
    endpoint_changed
}

async fn send_content(
    local_device_id: &str,
    peer: &TrustedPeer,
    address: &str,
    port: u16,
    content: &ClipboardContent,
) -> Result<(), String> {
    let key = decode_key(&peer.shared_key)?;
    let payload = serde_json::to_vec(content).map_err(|e| e.to_string())?;
    let (nonce, ciphertext) = encrypt(&key, &payload)?;
    let mut stream = timeout(
        Duration::from_secs(5),
        TcpStream::connect(format!("{address}:{port}")),
    )
    .await
    .map_err(|_| "连接超时".to_string())?
    .map_err(|e| e.to_string())?;
    write_frame(
        &mut stream,
        &WireMessage::Push {
            device_id: local_device_id.to_string(),
            nonce,
            ciphertext,
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    match read_frame(&mut stream).await.map_err(|e| e.to_string())? {
        WireMessage::Ack => Ok(()),
        WireMessage::Error { reason } => Err(reason),
        _ => Err("接收端返回了无效响应".into()),
    }
}

async fn read_frame(stream: &mut TcpStream) -> io::Result<WireMessage> {
    let length = stream.read_u32().await? as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame size",
        ));
    }
    let mut payload = vec![0u8; length];
    stream.read_exact(&mut payload).await?;
    serde_json::from_slice(&payload).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

async fn write_frame(stream: &mut TcpStream, message: &WireMessage) -> io::Result<()> {
    let payload =
        serde_json::to_vec(message).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    stream.write_u32(payload.len() as u32).await?;
    stream.write_all(&payload).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(address: Option<&str>) -> TrustedPeer {
        TrustedPeer {
            device_id: "peer-1".into(),
            device_name: "测试设备".into(),
            shared_key: "key".into(),
            paired_at: 1,
            last_seen: None,
            last_address: address.map(str::to_string),
            last_port: Some(45000),
        }
    }

    #[test]
    fn saved_endpoint_is_used_when_discovery_is_missing() {
        assert_eq!(
            resolve_peer_endpoint(&peer(Some("192.168.1.8")), None),
            Some(("192.168.1.8".into(), 45000))
        );
    }

    #[test]
    fn live_discovery_takes_priority_over_saved_endpoint() {
        let discovered = DiscoveredDevice {
            device_id: "peer-1".into(),
            device_name: "测试设备".into(),
            address: "192.168.1.9".into(),
            port: TRANSFER_PORT,
            pairing: false,
            last_seen: now_millis(),
        };
        assert_eq!(
            resolve_peer_endpoint(&peer(Some("192.168.1.8")), Some(&discovered)),
            Some(("192.168.1.9".into(), TRANSFER_PORT))
        );
    }

    #[test]
    fn only_exact_heartbeat_payload_is_accepted() {
        assert!(is_heartbeat_payload(b"lanclip-heartbeat-v1"));
        assert!(!is_heartbeat_payload(b"lanclip-heartbeat-v2"));
        assert!(!is_heartbeat_payload(b""));
    }
}
