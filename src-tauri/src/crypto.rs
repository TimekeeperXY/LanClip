use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub fn random_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::rng().fill_bytes(&mut key);
    key
}

pub fn encode_key(key: &[u8; 32]) -> String {
    STANDARD.encode(key)
}

pub fn decode_key(value: &str) -> Result<[u8; 32], String> {
    let bytes = STANDARD.decode(value).map_err(|e| e.to_string())?;
    bytes.try_into().map_err(|_| "无效的设备密钥".to_string())
}

pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<(String, String), String> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce = [0u8; 24];
    rand::rng().fill_bytes(&mut nonce);
    let encrypted = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| "加密失败".to_string())?;
    Ok((STANDARD.encode(nonce), STANDARD.encode(encrypted)))
}

pub fn decrypt(key: &[u8; 32], nonce: &str, ciphertext: &str) -> Result<Vec<u8>, String> {
    let nonce = STANDARD.decode(nonce).map_err(|e| e.to_string())?;
    let encrypted = STANDARD.decode(ciphertext).map_err(|e| e.to_string())?;
    if nonce.len() != 24 {
        return Err("无效的消息随机数".into());
    }
    XChaCha20Poly1305::new(key.into())
        .decrypt(XNonce::from_slice(&nonce), encrypted.as_ref())
        .map_err(|_| "消息认证失败".to_string())
}

pub fn content_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn pairing_key(code: &str, target_device_id: &str) -> [u8; 32] {
    let digest = Sha256::digest(format!("lanclip-pair-v1:{target_device_id}:{code}").as_bytes());
    digest.into()
}

pub fn pairing_proof(key: &[u8; 32], device_id: &str, device_name: &str) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(b"lanclip-pair-request-v1");
    mac.update(device_id.as_bytes());
    mac.update(device_name.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}

pub fn verify_pairing_proof(
    key: &[u8; 32],
    device_id: &str,
    device_name: &str,
    proof: &str,
) -> bool {
    pairing_proof(key, device_id, device_name) == proof
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_payload_round_trips() {
        let key = random_key();
        let (nonce, encrypted) = encrypt(&key, b"LanClip").unwrap();
        assert_eq!(decrypt(&key, &nonce, &encrypted).unwrap(), b"LanClip");
    }

    #[test]
    fn wrong_key_is_rejected() {
        let (nonce, encrypted) = encrypt(&random_key(), b"secret").unwrap();
        assert!(decrypt(&random_key(), &nonce, &encrypted).is_err());
    }
}
