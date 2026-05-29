use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::session::ActiveSession;
use crate::{ColdCatError, Result};

const SEALED_STATE_VERSION: u32 = 1;
type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SealedSession {
    pub version: u32,
    pub payload: String,
    pub signature: String,
}

pub fn generate_key() -> Vec<u8> {
    let mut key = vec![0_u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

pub fn encode_key(key: &[u8]) -> String {
    STANDARD.encode(key)
}

pub fn decode_key(input: &str) -> Result<Vec<u8>> {
    Ok(STANDARD.decode(input.trim())?)
}

pub fn seal_session(session: &ActiveSession, key: &[u8]) -> Result<SealedSession> {
    let payload_json = serde_json::to_vec(session)?;
    let payload = STANDARD.encode(payload_json);
    let signature = sign_payload(payload.as_bytes(), key)?;
    Ok(SealedSession {
        version: SEALED_STATE_VERSION,
        payload,
        signature,
    })
}

pub fn open_session(sealed: &SealedSession, key: &[u8]) -> Result<ActiveSession> {
    if sealed.version != SEALED_STATE_VERSION {
        return Err(ColdCatError::UnsupportedStateVersion(sealed.version));
    }

    let expected = sign_payload(sealed.payload.as_bytes(), key)?;
    if !constant_time_eq(expected.as_bytes(), sealed.signature.as_bytes()) {
        return Err(ColdCatError::StateTampered);
    }

    let payload = STANDARD.decode(&sealed.payload)?;
    Ok(serde_json::from_slice(&payload)?)
}

fn sign_payload(payload: &[u8], key: &[u8]) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| ColdCatError::Crypto)?;
    mac.update(payload);
    Ok(STANDARD.encode(mac.finalize().into_bytes()))
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut diff = 0_u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session() -> ActiveSession {
        ActiveSession {
            id: "session".to_string(),
            started_at: 100,
            end_time: 200,
            sites: vec!["reddit.com".to_string()],
            apps: vec!["discord".to_string()],
        }
    }

    #[test]
    fn seals_and_opens_session() {
        let key = generate_key();
        let sealed = seal_session(&sample_session(), &key).unwrap();
        assert_eq!(open_session(&sealed, &key).unwrap(), sample_session());
    }

    #[test]
    fn detects_tampered_payload() {
        let key = generate_key();
        let mut sealed = seal_session(&sample_session(), &key).unwrap();
        sealed.payload.push('A');
        assert!(matches!(
            open_session(&sealed, &key),
            Err(ColdCatError::StateTampered)
        ));
    }

    #[test]
    fn detects_wrong_key() {
        let sealed = seal_session(&sample_session(), &generate_key()).unwrap();
        assert!(matches!(
            open_session(&sealed, &generate_key()),
            Err(ColdCatError::StateTampered)
        ));
    }
}
