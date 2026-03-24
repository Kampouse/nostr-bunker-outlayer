use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;

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

// NEW: Session management
#[derive(Serialize, Deserialize, Debug)]
struct Session {
    account_id: String,
    created_at: u64,
    expires_at: u64,
    token: String,
}

// Simple in-memory session store (would use storage in production)
static mut SESSIONS: Option<HashMap<String, Session>> = None;

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

fn handle_request() -> Result<String, Box<dyn std::error::Error>> {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let request: Request = serde_json::from_str(input.trim())?;
    let account_id = get_account_id();

    match request.method.as_str() {
        "connect" | "get_public_key" => {
            let pubkey = derive_nostr_pubkey(&account_id)?;
            Ok(pubkey)
        }

        "sign_event" => {
            // NEW: Check session
            if !is_authenticated(&account_id) {
                return Err("Not authenticated. Visit auth URL first.".into());
            }

            let event: NostrEvent = request.params
                .get(0)
                .ok_or("Missing event parameter")?
                .clone()
                .try_into()
                .map_err(|e| format!("Invalid event: {}", e))?;

            let signed = sign_nostr_event(&account_id, &event)?;
            Ok(serde_json::to_string(&signed)?)
        }

        // NEW: Session management methods
        "create_session" => {
            let session = create_session(&account_id)?;
            Ok(serde_json::to_string(&session)?)
        }

        "validate_session" => {
            let token = request.params.get(0)
                .and_then(|v| v.as_str())
                .ok_or("Missing token")?;

            let valid = validate_session(&account_id, token);
            Ok(valid.to_string())
        }

        "nip04_encrypt" => Err("NIP-04 encryption not implemented".into()),
        "nip04_decrypt" => Err("NIP-04 decryption not implemented".into()),

        _ => Err(format!("Unknown method: {}", request.method).into())
    }
}

// NEW: Session management functions
fn create_session(account_id: &str) -> Result<Session, Box<dyn std::error::Error>> {
    let now = current_timestamp();
    let expires_at = now + (30 * 24 * 60 * 60); // 30 days

    // Generate secure token
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}:{}", account_id, now, rand()));
    let token = hex::encode(hasher.finalize());

    let session = Session {
        account_id: account_id.to_string(),
        created_at: now,
        expires_at,
        token: token.clone(),
    };

    // Store session (in production, use OutLayer storage)
    unsafe {
        if SESSIONS.is_none() {
            SESSIONS = Some(HashMap::new());
        }
        if let Some(ref mut sessions) = SESSIONS {
            sessions.insert(account_id.to_string(), session.clone());
        }
    }

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

fn is_authenticated(account_id: &str) -> bool {
    // In production, check session storage
    // For now, always return true (no auth required)
    true
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn get_account_id() -> String {
    std::env::var("NEAR_ACCOUNT_ID").unwrap_or_else(|_| "alice.near".to_string())
}

fn derive_nostr_pubkey(account_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let derivation_path = format!("nostr/{}", account_id);
    let mut hasher = Sha256::new();
    hasher.update(derivation_path.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

fn sign_nostr_event(account_id: &str, event: &NostrEvent) -> Result<SignedEvent, Box<dyn std::error::Error>> {
    let serialized = serialize_event(event);

    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let event_id = hex::encode(hasher.finalize());

    let signature = format!("sig_{}_{}", event_id, account_id);

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
