use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::io::{self, Read, Write};

// ============================================
// TYPE DEFINITIONS
// ============================================

#[derive(Deserialize)]
struct Request {
    id: Option<u64>,
    method: String,
    params: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct Response {
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct NostrEvent {
    pubkey: String,
    created_at: u64,
    kind: u16,
    tags: Vec<Vec<String>>,
    content: String,
}

#[derive(Serialize)]
struct SignedEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u16,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

// ============================================
// SESSION STORAGE (in-memory)
// ============================================

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
static PUBKEYS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    token: String,
    account_id: String,
    created_at: u64,
    expires_at: u64,
}

// ============================================
// MAIN
// ============================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read input from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    
    // Parse request
    let request: Request = serde_json::from_str(&input)?;
    
    // Process method
    let response = match request.method.as_str() {
        "get_public_key" => handle_get_public_key(&request),
        "sign_event" => handle_sign_event(&request),
        "create_session" => handle_create_session(&request),
        "get_session" => handle_get_session(&request),
        _ => Response {
            id: request.id,
            result: None,
            error: Some(format!("Unknown method: {}", request.method)),
        },
    };
    
    // Write output to stdout
    let output = serde_json::to_string(&response)?;
    print!("{}", output);
    io::stdout().flush()?;
    
    Ok(())
}

// ============================================
// HANDLERS
// ============================================

fn handle_get_public_key(request: &Request) -> Response {
    // Get or create pubkey for the signer
    let account_id = "kampouse.near"; // In real impl, get from OutLayer env
    
    let pubkeys = PUBKEYS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut pubkeys = pubkeys.lock().unwrap();
    
    let pubkey = pubkeys.entry(account_id.to_string()).or_insert_with(|| {
        // Derive deterministic pubkey from account_id
        let mut hasher = Sha256::new();
        hasher.update(account_id.as_bytes());
        hasher.update(b"nostr-bunker-outlayer-v1");
        let hash = hasher.finalize();
        hex::encode(&hash[0..32])
    });
    
    Response {
        id: request.id,
        result: Some(serde_json::json!(pubkey)),
        error: None,
    }
}

fn handle_sign_event(request: &Request) -> Response {
    // Parse event from params
    let event: NostrEvent = match request.params.get(0) {
        Some(val) => match serde_json::from_value(val.clone()) {
            Ok(e) => e,
            Err(e) => return Response {
                id: request.id,
                result: None,
                error: Some(format!("Invalid event: {}", e)),
            },
        },
        None => return Response {
            id: request.id,
            result: None,
            error: Some("Missing event parameter".to_string()),
        },
    };
    
    // Compute event ID (sha256 of serialized event)
    let serialized = serialize_event(&event);
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let event_id = hex::encode(hasher.finalize());
    
    // In real impl, would sign with v1.signer MPC
    // For now, generate deterministic "signature" 
    let sig = format!("sig_{}", &event_id[..64]);
    
    let signed = SignedEvent {
        id: event_id,
        pubkey: event.pubkey,
        created_at: event.created_at,
        kind: event.kind,
        tags: event.tags,
        content: event.content,
        sig,
    };
    
    Response {
        id: request.id,
        result: Some(serde_json::to_value(signed).unwrap()),
        error: None,
    }
}

fn handle_create_session(request: &Request) -> Response {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let account_id = "kampouse.near";
    let token = format!("sess_{}", hex::encode(&Sha256::digest(format!("{}:{}", account_id, now).as_bytes())[..16]));
    
    let session = Session {
        token: token.clone(),
        account_id: account_id.to_string(),
        created_at: now,
        expires_at: now + 86400 * 30, // 30 days
    };
    
    // Store session
    let sessions = SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sessions = sessions.lock().unwrap();
    sessions.insert(token.clone(), session.clone());
    
    Response {
        id: request.id,
        result: Some(serde_json::to_value(&session).unwrap()),
        error: None,
    }
}

fn handle_get_session(request: &Request) -> Response {
    let token = match request.params.get(0).and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Response {
            id: request.id,
            result: None,
            error: Some("Missing token parameter".to_string()),
        },
    };
    
    let sessions = SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let sessions = sessions.lock().unwrap();
    
    match sessions.get(token) {
        Some(session) => Response {
            id: request.id,
            result: Some(serde_json::to_value(session).unwrap()),
            error: None,
        },
        None => Response {
            id: request.id,
            result: None,
            error: Some("Session not found".to_string()),
        },
    }
}

// ============================================
// HELPERS
// ============================================

fn serialize_event(event: &NostrEvent) -> String {
    // NIP-01 serialization: [0, pubkey, created_at, kind, tags, content]
    let serialized = serde_json::to_string(&serde_json::json!([
        0,
        event.pubkey,
        event.created_at,
        event.kind,
        event.tags,
        event.content
    ])).unwrap();
    serialized
}
