//! Multi-User Nostr Bunker with TEE + MPC
//!
//! Architecture:
//! 1. User authorizes TEE relayer in v1.signer (one-time)
//! 2. TEE holds relayer.near private key
//! 3. Apps send sign requests to TEE
//! 4. TEE signs tx as relayer, calls v1.signer
//! 5. MPC produces Schnorr signature

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::io::{self, Read, Write};

// ============================================
// Configuration
// ============================================

const NEAR_RPC_URL: &str = "https://rpc.mainnet.near.org";
const V1_SIGNER_CONTRACT: &str = "v1.signer";
const RELAYER_ACCOUNT: &str = "bunker-relayer.near"; // TEE's NEAR account

// ============================================
// Types
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

#[derive(Deserialize, Serialize, Clone)]
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

#[derive(Serialize)]
struct NearTransaction {
    signer_id: String,
    receiver_id: String,
    method_name: String,
    args: serde_json::Value,
    gas: u64,
    deposit: u64,
}

#[derive(Serialize)]
struct DelegationRequest {
    relayer: String,
    user_path: String,
    max_usage: Option<u64>,
    expires_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone)]
struct UserSession {
    account_id: String,
    nostr_pubkey: String,
    authorized: bool,
    created_at: u64,
}

// ============================================
// Storage (in TEE, encrypted)
// ============================================

static mut SESSIONS: Option<HashMap<String, UserSession>> = None;
static mut RELAYER_KEY: Option<String> = None;

// ============================================
// Main Entry Point
// ============================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: Request = serde_json::from_str(&input)?;
    
    // Initialize relayer key from environment (TEE provides this)
    let relayer_key = std::env::var("RELAYER_PRIVATE_KEY")
        .unwrap_or_else(|_| "".to_string());
    
    unsafe {
        RELAYER_KEY = Some(relayer_key);
    }
    
    let response = handle_request(request)?;
    
    print!("{}", serde_json::to_string(&response)?);
    io::stdout().flush()?;
    Ok(())
}

// ============================================
// Request Handler
// ============================================

fn handle_request(request: Request) -> Result<Response, String> {
    match request.method.as_str() {
        "ping" => Ok(Response {
            id: request.id,
            result: Some(serde_json::json!({
                "version": "5.0.0",
                "type": "multi-user-bunker",
                "methods": [
                    "ping",
                    "connect",
                    "get_public_key",
                    "get_identity",
                    "sign_event",
                    "check_authorization",
                    "revoke_authorization"
                ],
                "relayer": RELAYER_ACCOUNT,
                "mpc_contract": V1_SIGNER_CONTRACT
            })),
            error: None,
        }),
        
        // Step 1: User initiates connection
        "connect" => {
            let account_id = get_account_id(&request.params)?;
            handle_connect(&account_id, request.id)
        },
        
        // Step 2: Check if user has authorized relayer
        "check_authorization" => {
            let account_id = get_account_id(&request.params)?;
            handle_check_auth(&account_id, request.id)
        },
        
        // Get user's Nostr pubkey
        "get_public_key" => {
            let account_id = get_account_id(&request.params)?;
            handle_get_pubkey(&account_id, request.id)
        },
        
        // Get full identity
        "get_identity" => {
            let account_id = get_account_id(&request.params)?;
            handle_get_identity(&account_id, request.id)
        },
        
        // Sign event (requires prior authorization)
        "sign_event" => {
            let account_id = get_account_id(&request.params)?;
            let event = request.params.get(1)
                .ok_or("Missing event parameter")?;
            handle_sign_event(&account_id, event, request.id)
        },
        
        // Revoke relayer authorization
        "revoke_authorization" => {
            let account_id = get_account_id(&request.params)?;
            handle_revoke(&account_id, request.id)
        },
        
        _ => Ok(Response {
            id: request.id,
            result: None,
            error: Some(format!("Unknown method: {}", request.method)),
        }),
    }
}

// ============================================
// Handlers
// ============================================

fn handle_connect(account_id: &str, request_id: Option<u64>) -> Result<Response, String> {
    // Generate user's Nostr path
    let user_path = format!("nostr/{}", account_id);
    
    // Try to get their pubkey from v1.signer
    let pubkey = get_derived_public_key(account_id)?;
    
    // Create session
    let session = UserSession {
        account_id: account_id.to_string(),
        nostr_pubkey: pubkey.clone(),
        authorized: false, // Will be true after user authorizes
        created_at: current_timestamp(),
    };
    
    store_session(account_id, session);
    
    // Return connection info with delegation instructions
    Ok(Response {
        id: request_id,
        result: Some(serde_json::json!({
            "status": "pending_authorization",
            "nostr_pubkey": pubkey,
            "nostr_npub": format!("npub1{}", &pubkey[..58]),
            "relayer": RELAYER_ACCOUNT,
            "authorization_required": true,
            "authorization_instructions": {
                "method": "v1.signer.add_allowance",
                "args": {
                    "allowance": {
                        "allowance": {
                            "account_id": RELAYER_ACCOUNT,
                            "allowance": "1000000000000000000000000" // 1M gas worth
                        }
                    }
                },
                "description": "Call this from your NEAR wallet to authorize the bunker to sign on your behalf"
            }
        })),
        error: None,
    })
}

fn handle_check_auth(account_id: &str, request_id: Option<u64>) -> Result<Response, String> {
    // Check if user has authorized the relayer in v1.signer
    let is_authorized = check_user_allowance(account_id)?;
    
    // Update session
    if let Some(mut session) = get_session(account_id) {
        session.authorized = is_authorized;
        store_session(account_id, session);
    }
    
    Ok(Response {
        id: request_id,
        result: Some(serde_json::json!({
            "account_id": account_id,
            "authorized": is_authorized,
            "relayer": RELAYER_ACCOUNT,
            "next_step": if is_authorized {
                "Ready to sign! Use sign_event method."
            } else {
                "Authorize relayer in v1.signer first."
            }
        })),
        error: None,
    })
}

fn handle_get_pubkey(account_id: &str, request_id: Option<u64>) -> Result<Response, String> {
    let pubkey = get_derived_public_key(account_id)?;
    
    Ok(Response {
        id: request_id,
        result: Some(serde_json::json!(pubkey)),
        error: None,
    })
}

fn handle_get_identity(account_id: &str, request_id: Option<u64>) -> Result<Response, String> {
    let pubkey = get_derived_public_key(account_id)?;
    let is_authorized = check_user_allowance(account_id)?;
    
    Ok(Response {
        id: request_id,
        result: Some(serde_json::json!({
            "near_account": account_id,
            "nostr_pubkey": pubkey,
            "nostr_npub": format!("npub1{}", &pubkey[..58]),
            "bunker_url": format!("bunker://{}@bunker-relayer.near?relay=wss://nostr-bunker.kj95hgdgnn.workers.dev", pubkey),
            "mpc_path": format!("nostr/{}", account_id),
            "relayer": RELAYER_ACCOUNT,
            "authorized": is_authorized,
            "verification": format!("https://near.social/#/{}", account_id)
        })),
        error: None,
    })
}

fn handle_sign_event(account_id: &str, event_value: &serde_json::Value, request_id: Option<u64>) -> Result<Response, String> {
    // Check if authorized
    let is_authorized = check_user_allowance(account_id)?;
    if !is_authorized {
        return Ok(Response {
            id: request_id,
            result: None,
            error: Some("Not authorized. Call connect and authorize relayer in v1.signer first.".to_string()),
        });
    }
    
    // Parse event
    let event: NostrEvent = serde_json::from_value(event_value.clone())
        .map_err(|e| format!("Invalid event: {}", e))?;
    
    // Calculate event ID
    let serialized = serde_json::to_string(&serde_json::json!([
        0, &event.pubkey, event.created_at, event.kind, &event.tags, &event.content
    ])).unwrap_or_default();
    
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let event_id = hex::encode(hasher.finalize());
    
    // Sign via v1.signer (TEE signs as relayer)
    let signature = sign_with_mpc(account_id, &event_id)?;
    
    Ok(Response {
        id: request_id,
        result: Some(serde_json::to_value(SignedEvent {
            id: event_id,
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags,
            content: event.content,
            sig: signature,
        })?),
        error: None,
    })
}

fn handle_revoke(account_id: &str, request_id: Option<u64>) -> Result<Response, String> {
    // Remove local session
    remove_session(account_id);
    
    // Note: User must revoke in v1.signer themselves (can't be done by relayer)
    Ok(Response {
        id: request_id,
        result: Some(serde_json::json!({
            "status": "session_revoked",
            "note": "Also revoke in v1.signer by calling remove_allowance"
        })),
        error: None,
    })
}

// ============================================
// v1.signer Integration
// ============================================

fn get_derived_public_key(account_id: &str) -> Result<String, String> {
    let args = serde_json::json!({
        "domain": 0,
        "path": format!("nostr/{}", account_id),
    });
    
    let result = call_near_view(V1_SIGNER_CONTRACT, "derived_public_key", &args)?;
    
    // Parse bytes to string
    let bytes: Vec<u8> = serde_json::from_value(result.clone())
        .unwrap_or_default();
    let pubkey = String::from_utf8(bytes)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();
    
    // Remove "secp256k1:" prefix if present
    Ok(pubkey.replace("secp256k1:", ""))
}

fn check_user_allowance(account_id: &str) -> Result<bool, String> {
    // Check if user has authorized relayer
    let args = serde_json::json!({
        "account_id": account_id,
    });
    
    // This calls v1.signer to check allowance
    // Note: This is a view call, so it should work
    match call_near_view(V1_SIGNER_CONTRACT, "get_allowance", &args) {
        Ok(result) => {
            // If allowance > 0, user has authorized
            let allowance: u128 = serde_json::from_value(result.clone())
                .unwrap_or(0);
            Ok(allowance > 0)
        },
        Err(_) => {
            // If call fails, assume not authorized
            Ok(false)
        }
    }
}

fn sign_with_mpc(account_id: &str, message_hash: &str) -> Result<String, String> {
    // Get relayer key from TEE storage
    let relayer_key = unsafe {
        RELAYER_KEY.clone().unwrap_or_default()
    };
    
    if relayer_key.is_empty() {
        return Err("Relayer key not configured. Set RELAYER_PRIVATE_KEY in TEE.".to_string());
    }
    
    // Build transaction
    let args = serde_json::json!({
        "domain": 0,
        "path": format!("nostr/{}", account_id),
        "payload": message_hash,
    });
    
    // Sign and send transaction as relayer
    let result = send_near_transaction(
        RELAYER_ACCOUNT,
        V1_SIGNER_CONTRACT,
        "sign",
        &args,
        300_000_000_000_000, // 300 TGas
        0,                   // No deposit
        &relayer_key,
    )?;
    
    // Extract signature from result
    let signature = result["status"]["SuccessValue"]
        .as_str()
        .ok_or("No signature in response")?;
    
    // Decode base64 signature
    let sig_bytes = base64_decode(signature);
    Ok(String::from_utf8(sig_bytes).unwrap_or_default())
}

// ============================================
// NEAR RPC Client
// ============================================

fn call_near_view(contract_id: &str, method: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let args_base64 = base64_encode(&serde_json::to_string(args).unwrap_or_default());
    
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "dontcare",
        "method": "query",
        "params": {
            "request_type": "call_function",
            "finality": "final",
            "account_id": contract_id,
            "method_name": method,
            "args_base64": args_base64,
        }
    });
    
    let response = http_post(NEAR_RPC_URL, &serde_json::to_string(&request).unwrap())?;
    let rpc_response: serde_json::Value = serde_json::from_str(&response)
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    if let Some(error) = rpc_response.get("error") {
        return Err(format!("NEAR RPC error: {}", error));
    }
    
    Ok(rpc_response["result"]["result"].clone())
}

fn send_near_transaction(
    signer_id: &str,
    receiver_id: &str,
    method: &str,
    args: &serde_json::Value,
    gas: u64,
    deposit: u64,
    _private_key: &str,
) -> Result<serde_json::Value, String> {
    // TODO: Implement actual transaction signing
    // This requires:
    // 1. Build transaction (nonce, block hash, actions)
    // 2. Sign with ed25519 private key
    // 3. Serialize to base64
    // 4. Send via broadcast_tx_commit
    
    // For now, return error indicating implementation needed
    Err("Transaction signing not yet implemented. Need to add ed25519 signing logic.".to_string())
}

// ============================================
// HTTP Client
// ============================================

#[cfg(target_arch = "wasm32")]
fn http_post(url: &str, body: &str) -> Result<String, String> {
    // OutLayer provides HTTP via host function
    Err("HTTP requires OutLayer runtime. Deploy to TEE.".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn http_post(url: &str, body: &str) -> Result<String, String> {
    use std::process::Command;
    
    let output = Command::new("curl")
        .args(["-s", "-X", "POST", "-H", "Content-Type: application/json", "-d", body, url])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;
    
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8: {}", e))
    } else {
        Err(format!("curl error: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

// ============================================
// Session Management
// ============================================

fn store_session(account_id: &str, session: UserSession) {
    unsafe {
        if SESSIONS.is_none() {
            SESSIONS = Some(HashMap::new());
        }
        if let Some(ref mut sessions) = SESSIONS {
            sessions.insert(account_id.to_string(), session);
        }
    }
}

fn get_session(account_id: &str) -> Option<UserSession> {
    unsafe {
        SESSIONS.as_ref()?.get(account_id).cloned()
    }
}

fn remove_session(account_id: &str) {
    unsafe {
        if let Some(ref mut sessions) = SESSIONS {
            sessions.remove(account_id);
        }
    }
}

// ============================================
// Utilities
// ============================================

fn get_account_id(params: &[serde_json::Value]) -> Result<String, String> {
    if params.is_empty() {
        return Err("Missing account_id parameter".to_string());
    }
    
    let first = &params[0];
    
    if let Some(s) = first.as_str() {
        Ok(s.to_string())
    } else if let Some(obj) = first.as_object() {
        obj.get("account_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "Missing account_id in object".to_string())
    } else {
        Err("Invalid account_id parameter".to_string())
    }
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn base64_encode(input: &str) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();
    
    for chunk in bytes.chunks(3) {
        let mut n = 0u32;
        for (i, &byte) in chunk.iter().enumerate() {
            n |= (byte as u32) << (16 - i * 8);
        }
        
        for i in 0..4 {
            if i * 6 < chunk.len() * 8 + 8 {
                let idx = ((n >> (18 - i * 6)) & 0x3F) as usize;
                result.push(CHARSET[idx] as char);
            } else {
                result.push('=');
            }
        }
    }
    
    result
}

fn base64_decode(input: &str) -> Vec<u8> {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    
    let input = input.trim_end_matches('=');
    let mut result = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    
    for c in input.chars() {
        if let Some(pos) = CHARSET.iter().position(|&x| x as char == c) {
            buffer = (buffer << 6) | pos as u32;
            bits += 6;
            
            if bits >= 8 {
                bits -= 8;
                result.push((buffer >> bits) as u8);
            }
        }
    }
    
    result
}
