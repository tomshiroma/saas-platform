use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;
use subtle::ConstantTimeEq;

use crate::error::{ApiError, ApiResult};

const TOTP_STEP_SECONDS: u64 = 30;
const TOTP_DIGITS: u32 = 1_000_000;

pub fn generate_secret() -> Vec<u8> {
    let mut secret = vec![0_u8; 20];
    rand::thread_rng().fill_bytes(&mut secret);
    secret
}

pub fn encode_secret(secret: &[u8]) -> String {
    BASE32_NOPAD.encode(secret)
}

pub fn provisioning_uri(secret: &[u8], email: &str, tenant_name: &str) -> String {
    let issuer = percent_encode("SaaS Platform");
    let account = percent_encode(&format!("{tenant_name}:{email}"));
    format!(
        "otpauth://totp/{issuer}:{account}?secret={}&issuer={issuer}&algorithm=SHA1&digits=6&period=30",
        encode_secret(secret)
    )
}

pub fn encrypt_secret(key: &[u8; 32], secret: &[u8]) -> ApiResult<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(ApiError::internal)?;
    let mut nonce_bytes = [0_u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), secret)
        .map_err(ApiError::internal)?;
    let mut result = nonce_bytes.to_vec();
    result.extend(encrypted);
    Ok(result)
}

pub fn decrypt_secret(key: &[u8; 32], encrypted: &[u8]) -> ApiResult<Vec<u8>> {
    if encrypted.len() <= 12 {
        return Err(ApiError::internal("invalid encrypted MFA secret"));
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(ApiError::internal)?;
    cipher
        .decrypt(Nonce::from_slice(&encrypted[..12]), &encrypted[12..])
        .map_err(ApiError::internal)
}

pub fn verify_totp(secret: &[u8], code: &str) -> bool {
    let Ok(elapsed) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return false;
    };
    verify_totp_at(secret, code, elapsed.as_secs())
}

fn verify_totp_at(secret: &[u8], code: &str, timestamp: u64) -> bool {
    if code.len() != 6 || !code.bytes().all(|value| value.is_ascii_digit()) {
        return false;
    }
    let expected = code.as_bytes();
    let counter = timestamp / TOTP_STEP_SECONDS;
    [counter.saturating_sub(1), counter, counter + 1]
        .into_iter()
        .any(|value| {
            format!("{:06}", totp_code(secret, value))
                .as_bytes()
                .ct_eq(expected)
                .into()
        })
}

fn totp_code(secret: &[u8], counter: u64) -> u32 {
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(secret).expect("HMAC accepts any key size");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    binary % TOTP_DIGITS
}

pub fn generate_recovery_codes() -> Vec<String> {
    (0..10)
        .map(|_| {
            let mut bytes = [0_u8; 8];
            rand::thread_rng().fill_bytes(&mut bytes);
            let encoded = BASE32_NOPAD.encode(&bytes);
            format!("{}-{}", &encoded[..6], &encoded[6..])
        })
        .collect()
}

pub fn normalize_recovery_code(code: &str) -> String {
    code.trim().replace('-', "").to_ascii_uppercase()
}

pub fn challenge_token(tenant_id: uuid::Uuid) -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("{tenant_id}.{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                vec![char::from(byte)].into_iter()
            } else {
                format!("%{byte:02X}")
                    .chars()
                    .collect::<Vec<_>>()
                    .into_iter()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{decrypt_secret, encrypt_secret, normalize_recovery_code, verify_totp_at};

    #[test]
    fn totp_accepts_rfc_6238_sha1_vector() {
        assert!(verify_totp_at(b"12345678901234567890", "287082", 59));
    }

    #[test]
    fn secret_round_trip_uses_authenticated_encryption() {
        let key = [7_u8; 32];
        let encrypted = encrypt_secret(&key, b"secret").unwrap();
        assert_ne!(encrypted, b"secret");
        assert_eq!(decrypt_secret(&key, &encrypted).unwrap(), b"secret");
    }

    #[test]
    fn recovery_code_normalization_ignores_separator_and_case() {
        assert_eq!(normalize_recovery_code("abcd-efgh"), "ABCDEFGH");
    }
}
