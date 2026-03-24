use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================
// TYPE DEFINITIONS
// ============================================

#[derive(Deserialize, Debug)]
struct Request {
    id: Option<u64>,
    method: String,
    params: Vec<serde_json::Value>,
}

#[derive(Serialize, Debug)]
struct Response<T: Serialize> {
    id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct NostrEvent {
    pubkey: String,
    created_at: u64,
    kind: u16,
    tags: Vec<Vec<String>>,
    content: String,
}

#[derive(Serialize, Debug)]
struct SignedEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u16,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

// Session management
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Session {
    account_id: String,
    created_at: u64,
    expires_at: u64,
    token: String,
}

// NIP-04 encrypted message
#[derive(Serialize, Deserialize, Debug)]
struct EncryptedMessage {
    ciphertext: String,
    nonce: String,
    ephemeral_key: String,
}

// ============================================
// GLOBAL STATE (In production, use OutLayer storage)
// ============================================

static mut SESSIONS: Option<HashMap<String, Session>> = None;
static mut PUBKEY_CACHE: Option<HashMap<String, (String, u64)>> = None;

// ============================================
// MAIN ENTRY POINT
// ============================================

fn main() {
    let result = handle_request();

    let response = match result {
        Ok(r) => Response {
            id: None,
            result: Some(r),
            error: None,
        },
        Err(e) => Response {
            id: None,
            result: None,
            error: Some(e.to_string()),
        },
    };

    println!("{}", serde_json::to_string(&response).unwrap());
}

// ============================================
// REQUEST HANDLER
// ============================================

fn handle_request() -> Result<String, Box<dyn std::error::Error>> {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let request: Request = serde_json::from_str(input.trim())?;

    // Get account ID from environment
    let account_id = get_account_id();

    // Log request (monitoring)
    log_audit(&format!("Request: {}", request.method), &account_id);

    match request.method.as_str() {
        "connect" | "get_public_key" => {
            let pubkey = derive_nostr_pubkey(&account_id)?;
            Ok(pubkey)
        }

        "sign_event" => {
            // Check authentication
            if !is_authenticated(&account_id) {
                return Err("Not authenticated. Call create_session first.".into());
            }

            let event: NostrEvent = serde_json::from_value(
                request.params.get(0)
                    .ok_or("Missing event parameter")?
                    .clone()
            ).map_err(|e| format!("Invalid event: {}", e))?;

            let signed = sign_nostr_event(&account_id, &event)?;
            Ok(serde_json::to_string(&signed)?)
        }

        // Session management
        "create_session" => {
            let session = create_session(&account_id)?;
            Ok(serde_json::to_string(&session)?)
        }

        "validate_session" => {
            let token = request.params.get(0)
                .and_then(|v| v.as_str())
                .ok_or("Missing token parameter")?;

            let valid = validate_session(&account_id, token);
            Ok(serde_json::to_string(&serde_json::json!({ "valid": valid }))?)
        }

        "revoke_session" => {
            revoke_session(&account_id);
            Ok(serde_json::to_string(&serde_json::json!({ "revoked": true }))?)
        }

        // NIP-04 encryption/decryption
        "nip04_encrypt" => {
            let pubkey = request.params.get(0)
                .and_then(|v| v.as_str())
                .ok_or("Missing pubkey parameter")?;
            let plaintext = request.params.get(1)
                .and_then(|v| v.as_str())
                .ok_or("Missing plaintext parameter")?;

            let encrypted = nip04_encrypt(&account_id, pubkey, plaintext)?;
            Ok(serde_json::to_string(&encrypted)?)
        }

        "nip04_decrypt" => {
            let pubkey = request.params.get(0)
                .and_then(|v| v.as_str())
                .ok_or("Missing pubkey parameter")?;
            let ciphertext = request.params.get(1)
                .and_then(|v| v.as_str())
                .ok_or("Missing ciphertext parameter")?;

            let decrypted = nip04_decrypt(&account_id, pubkey, ciphertext)?;
            Ok(decrypted)
        }

        _ => Err(format!("Unknown method: {}", request.method).into())
    }
}

// ============================================
// KEY DERIVATION & SIGNING
// ============================================

fn derive_nostr_pubkey(account_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Check cache first
    if let Some(cached) = get_cached_pubkey(account_id) {
        return Ok(cached);
    }

    // Sanitize account ID
    let account_id = sanitize_account_id(account_id)?;

    // In production: Call v1.signer.derived_public_key via NEAR RPC
    // For now: Deterministic derivation
    let derivation_path = format!("nostr/{}", account_id);
    let mut hasher = Sha256::new();
    hasher.update(derivation_path.as_bytes());
    let pubkey = hex::encode(hasher.finalize());

    // Cache for 1 hour
    cache_pubkey(&account_id, &pubkey, 3600);

    Ok(pubkey)
}

fn sign_nostr_event(account_id: &str, event: &NostrEvent) -> Result<SignedEvent, Box<dyn std::error::Error>> {
    // 1. Serialize event (NIP-01)
    let serialized = serialize_event(event);

    // 2. Hash with SHA-256
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let event_id = hex::encode(hasher.finalize());

    // 3. Get relayer credentials
    let relayer_id = std::env::var("RELAYER_ACCOUNT_ID")
        .unwrap_or_else(|_| "relayer.near".to_string());
    let relayer_key = std::env::var("RELAYER_PRIVATE_KEY")
        .unwrap_or_else(|_| "".to_string());

    // 4. Sign via v1.signer (placeholder for NEAR RPC call)
    let signature = if relayer_key.is_empty() {
        // Fallback: placeholder signature
        format!("sig_{}_{}", event_id, account_id)
    } else {
        // Production: Call v1.signer via NEAR RPC
        call_v1_signer(&relayer_id, &relayer_key, account_id, &event_id)?
    };

    Ok(SignedEvent {
        id: event_id,
        pubkey: event.pubkey.clone(),
        created_at: event.created_at,
        kind: event.kind,
        tags: event.tags.clone(),
        content: event.content.clone(),
        sig: signature,
    })
}

fn call_v1_signer(
    relayer_id: &str,
    _relayer_key: &str,
    account_id: &str,
    event_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // TODO: Implement actual NEAR RPC call
    // This would use the near-sdk or JSON-RPC client
    //
    // Example (pseudocode):
    // let response = near_rpc::call(
    //     relayer_id,
    //     relayer_key,
    //     "v1.signer",
    //     "sign",
    //     json!({
    //         "domain": 0,
    //         "path": format!("nostr/{}", account_id),
    //         "payload": event_id,
    //     }),
    //     "0",  // deposit
    //     "30000000000000",  // gas
    //     "FINAL",
    // )?;

    // For now: return placeholder
    Ok(format!("sig_{}_{}", event_id, relayer_id))
}

// ============================================
// SESSION MANAGEMENT
// ============================================

fn create_session(account_id: &str) -> Result<Session, Box<dyn std::error::Error>> {
    let now = current_timestamp();
    let expires_at = now + (30 * 24 * 60 * 60); // 30 days

    // Generate secure token
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}:{}:{}", account_id, now, rand_u64(), "secret_salt"));
    let token = hex::encode(hasher.finalize());

    let session = Session {
        account_id: account_id.to_string(),
        created_at: now,
        expires_at,
        token: token.clone(),
    };

    // Store session
    unsafe {
        if SESSIONS.is_none() {
            SESSIONS = Some(HashMap::new());
        }
        if let Some(ref mut sessions) = SESSIONS {
            sessions.insert(account_id.to_string(), session.clone());
        }
    }

    log_audit(&format!("Session created, expires {}", expires_at), account_id);

    Ok(session)
}

fn validate_session(account_id: &str, token: &str) -> bool {
    unsafe {
        if let Some(ref sessions) = SESSIONS {
            if let Some(session) = sessions.get(account_id) {
                return session.token == token && session.expires_at > current_timestamp();
            }
        }
    }
    false
}

fn revoke_session(account_id: &str) {
    unsafe {
        if let Some(ref mut sessions) = SESSIONS {
            sessions.remove(account_id);
        }
    }
    log_audit("Session revoked", account_id);
}

fn is_authenticated(account_id: &str) -> bool {
    // In production, check for valid session
    // For now, return true (no auth required for basic usage)
    true
}

// ============================================
// NIP-04 ENCRYPTION/DECRYPTION
// ============================================

fn nip04_encrypt(
    _account_id: &str,
    pubkey: &str,
    plaintext: &str,
) -> Result<EncryptedMessage, Box<dyn std::error::Error>> {
    // TODO: Implement actual encryption using x25519-dalek + AES-GCM
    //
    // Steps:
    // 1. Generate ephemeral key pair
    // 2. Compute shared secret via X25519
    // 3. Encrypt plaintext with AES-256-GCM
    // 4. Return: ciphertext, nonce, ephemeral_pubkey

    // For now: return placeholder
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}:{}", pubkey, plaintext, rand_u64()));
    let ciphertext = hex::encode(hasher.finalize());

    Ok(EncryptedMessage {
        ciphertext,
        nonce: "0123456789abcdef".to_string(),
        ephemeral_key: pubkey.to_string(),
    })
}

fn nip04_decrypt(
    _account_id: &str,
    _pubkey: &str,
    ciphertext: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // TODO: Implement actual decryption
    //
    // Steps:
    // 1. Parse ciphertext, nonce, ephemeral_pubkey
    // 2. Compute shared secret via X25519
    // 3. Decrypt with AES-256-GCM
    // 4. Return plaintext

    // For now: return placeholder
    Ok(format!("decrypted_{}", ciphertext))
}

// ============================================
// CACHING
// ============================================

fn cache_pubkey(account_id: &str, pubkey: &str, ttl_seconds: u64) {
    let expires_at = current_timestamp() + ttl_seconds;

    unsafe {
        if PUBKEY_CACHE.is_none() {
            PUBKEY_CACHE = Some(HashMap::new());
        }
        if let Some(ref mut cache) = PUBKEY_CACHE {
            cache.insert(account_id.to_string(), (pubkey.to_string(), expires_at));
        }
    }
}

fn get_cached_pubkey(account_id: &str) -> Option<String> {
    unsafe {
        if let Some(ref cache) = PUBKEY_CACHE {
            if let Some((pubkey, expires_at)) = cache.get(account_id) {
                if *expires_at > current_timestamp() {
                    return Some(pubkey.clone());
                }
            }
        }
    }
    None
}

// ============================================
// UTILITY FUNCTIONS
// ============================================

fn get_account_id() -> String {
    std::env::var("NEAR_ACCOUNT_ID")
        .unwrap_or_else(|_| "alice.near".to_string())
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn rand_u64() -> u64 {
    // Simple pseudo-random (in production, use proper RNG)
    current_timestamp().wrapping_mul(2654435761)
}

fn serialize_event(event: &NostrEvent) -> String {
    format!(
        "[0,\"{}\",{},{},{},\"{}\"]",
        event.pubkey,
        event.created_at,
        event.kind,
        serde_json::to_string(&event.tags).unwrap_or_else(|_| "[]".to_string()),
        event.content
    )
}

fn sanitize_account_id(account_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    if account_id.len() > 64 {
        return Err("Account ID too long".into());
    }
    if !account_id.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_') {
        return Err("Invalid characters in account ID".into());
    }
    Ok(account_id.to_string())
}

fn log_audit(event: &str, account_id: &str) {
    eprintln!("[AUDIT] {} - {} - {}", current_timestamp(), account_id, event);
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_pubkey() {
        let pubkey = derive_nostr_pubkey("alice.near").unwrap();
        assert_eq!(pubkey.len(), 64);

        let pubkey2 = derive_nostr_pubkey("alice.near").unwrap();
        assert_eq!(pubkey, pubkey2);

        let pubkey3 = derive_nostr_pubkey("bob.near").unwrap();
        assert_ne!(pubkey, pubkey3);
    }

    #[test]
    fn test_session_creation() {
        let session = create_session("alice.near").unwrap();
        assert!(session.token.len() == 64);
        assert!(session.expires_at > session.created_at);
    }

    #[test]
    fn test_session_validation() {
        let session = create_session("alice.near").unwrap();
        assert!(validate_session("alice.near", &session.token));
        assert!(!validate_session("alice.near", "invalid_token"));
    }

    #[test]
    fn test_session_revocation() {
        let session = create_session("bob.near").unwrap();
        assert!(validate_session("bob.near", &session.token));
        revoke_session("bob.near");
        assert!(!validate_session("bob.near", &session.token));
    }

    #[test]
    fn test_serialize_event() {
        let event = NostrEvent {
            pubkey: "abc123".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "Hello!".to_string(),
        };

        let serialized = serialize_event(&event);
        assert!(serialized.starts_with("[0,"));
        assert!(serialized.contains("abc123"));
    }

    #[test]
    fn test_sign_event() {
        let event = NostrEvent {
            pubkey: "abc123".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "Hello!".to_string(),
        };

        let signed = sign_nostr_event("alice.near", &event).unwrap();
        assert_eq!(signed.id.len(), 64);
        assert!(signed.sig.starts_with("sig_"));
    }

    #[test]
    fn test_nip04_encrypt() {
        let encrypted = nip04_encrypt("alice.near", "bob_pubkey", "Hello!").unwrap();
        assert!(!encrypted.ciphertext.is_empty());
        assert!(!encrypted.nonce.is_empty());
    }

    #[test]
    fn test_sanitize_account_id() {
        assert!(sanitize_account_id("alice.near").is_ok());
        assert!(sanitize_account_id("a".repeat(65).as_str()).is_err());
        assert!(sanitize_account_id("invalid@account").is_err());
    }

    #[test]
    fn test_pubkey_caching() {
        let pubkey1 = derive_nostr_pubkey("cache_test.near").unwrap();
        let pubkey2 = get_cached_pubkey("cache_test.near").unwrap();
        assert_eq!(pubkey1, pubkey2);
    }
}
