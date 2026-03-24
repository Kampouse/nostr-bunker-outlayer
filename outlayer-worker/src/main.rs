//! Nostr Bunker - OutLayer TEE Worker with v1.signer MPC
//! 
//! Architecture:
//! ┌─────────────────────┐
//! │ OutLayer TEE        │
//! │ ┌───────────────┐   │
//! │ │ WASM Code     │   │
//! │ │ - Derive keys │   │
//! │ │ - Calc event  │   │
//! │ │ - HTTP to RPC ├───┼──→ NEAR RPC
//! │ └───────────────┘   │
//! └─────────────────────┘
//!            │
//!            ▼
//! ┌───────────────┐
//! │ v1.signer     │
//! │ (MPC contract)│
//! └───────┬───────┘
//!         │
//!         ▼
//! ┌───────────────┐
//! │ MPC Network   │
//! │ (threshold)   │
//! │ signs event   │
//! └───────────────┘

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================
// Configuration
// ============================================

const NEAR_RPC_URL: &str = "https://rpc.mainnet.near.org";
const V1_SIGNER_CONTRACT: &str = "v1.signer";

// ============================================
// Type Definitions
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

#[derive(Serialize)]
struct NearRpcRequest {
    jsonrpc: String,
    id: String,
    method: String,
    params: serde_json::Value,
}

#[derive(Deserialize)]
struct NearRpcResponse {
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<NearRpcError>,
}

#[derive(Deserialize)]
struct NearRpcError {
    message: String,
}

#[derive(Serialize)]
struct SignRequest {
    domain: u8,
    path: String,
    payload: String,
}

// ============================================
// Global State
// ============================================

static mut SESSIONS: Option<HashMap<String, String>> = None;

// ============================================
// Main Entry Point
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
// Request Handler
// ============================================

fn handle_request() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let request: Request = serde_json::from_str(input.trim())?;
    let account_id = get_account_id();

    log_audit(&format!("Request: {}", request.method), &account_id);

    match request.method.as_str() {
        "ping" => Ok(serde_json::json!({
            "version": "4.0.0",
            "methods": ["ping", "get_public_key", "get_identity", "sign_event", "create_session"],
            "signing": "v1.signer MPC",
            "tee": "OutLayer"
        })),

        "get_public_key" => {
            let pubkey = get_derived_public_key(&account_id)?;
            Ok(serde_json::json!(pubkey))
        }

        "get_identity" => {
            let pubkey = get_derived_public_key(&account_id)?;
            Ok(serde_json::json!({
                "near_account": account_id,
                "nostr_pubkey": pubkey,
                "mpc_path": format!("nostr/{}", account_id),
                "mpc_contract": V1_SIGNER_CONTRACT
            }))
        }

        "create_session" => {
            let token = create_session(&account_id);
            Ok(serde_json::json!({
                "token": token,
                "account_id": account_id,
                "expires_in": 86400 * 30
            }))
        }

        "sign_event" => {
            let event: NostrEvent = request.params.get(0)
                .ok_or("Missing event parameter")?
                .clone()
                .try_into()
                .map_err(|e: serde_json::Error| format!("Invalid event: {}", e))?;

            let signed = sign_event(&account_id, &event)?;
            Ok(serde_json::to_value(signed)?)
        }

        _ => Err(format!("Unknown method: {}", request.method).into())
    }
}

// ============================================
// v1.signer MPC Integration
// ============================================

fn get_derived_public_key(account_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let args = serde_json::json!({
        "domain": 0,
        "path": format!("nostr/{}", account_id),
    });
    
    let result = call_near_view(V1_SIGNER_CONTRACT, "derived_public_key", &args)?;
    
    // Parse bytes to string
    let bytes: Vec<u8> = serde_json::from_value(result)?;
    let pubkey = String::from_utf8(bytes)?;
    
    Ok(pubkey.trim_matches('"').to_string())
}

fn sign_event(account_id: &str, event: &NostrEvent) -> Result<SignedEvent, Box<dyn std::error::Error>> {
    // 1. Calculate event ID (NIP-01)
    let serialized = format!(
        "[0,\"{}\",{},{},\"{}\"]",
        event.pubkey,
        event.created_at,
        event.kind,
        serde_json::to_string(&event.tags).unwrap_or_else(|_| "[]".to_string()),
        event.content
    );
    
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let event_id = hex::encode(hasher.finalize());
    
    // 2. Call v1.signer MPC
    let args = serde_json::json!({
        "domain": 0,
        "path": format!("nostr/{}", account_id),
        "payload": event_id,
    });
    
    let result = call_near_view(V1_SIGNER_CONTRACT, "sign", &args)?;
    
    // 3. Parse signature
    let bytes: Vec<u8> = serde_json::from_value(result)?;
    let sig = String::from_utf8(bytes)?;
    
    // 4. Return signed event
    Ok(SignedEvent {
        id: event_id,
        pubkey: event.pubkey.clone(),
        created_at: event.created_at,
        kind: event.kind,
        tags: event.tags.clone(),
        content: event.content.clone(),
        sig,
    })
}

// ============================================
// NEAR RPC Client
// ============================================

fn call_near_view(
    contract_id: &str,
    method: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let args_base64 = base64_encode(&serde_json::to_string(args)?);
    
    let request = NearRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: "dontcare".to_string(),
        method: "query".to_string(),
        params: serde_json::json!({
            "request_type": "call_function",
            "finality": "final",
            "account_id": contract_id,
            "method_name": method,
            "args_base64": args_base64,
        }),
    };
    
    let body = serde_json::to_string(&request)?;
    let response = http_post(NEAR_RPC_URL, &body)?;
    
    let rpc_response: NearRpcResponse = serde_json::from_str(&response)?;
    
    if let Some(error) = rpc_response.error {
        return Err(format!("NEAR RPC error: {}", error.message).into());
    }
    
    rpc_response.result
        .map(|r| r["result"].clone())
        .ok_or_else(|| "No result in response".into())
}

// ============================================
// HTTP Client
// ============================================

#[cfg(target_arch = "wasm32")]
fn http_post(url: &str, body: &str) -> Result<String, String> {
    // OutLayer provides HTTP via host function
    // Call signature: outlayer_http_post(url: ptr, url_len: u32, body: ptr, body_len: u32) -> ptr
    
    // This is a placeholder - the actual implementation would be:
    // #[link(wasm_import_module = "outlayer")]
    // extern "C" {
    //     fn http_post(url: *const u8, url_len: u32, body: *const u8, body_len: u32, out: *mut u8, out_len: *mut u32) -> u32;
    // }
    
    // For now, return error indicating deployment needed
    Err("Deploy to OutLayer TEE for HTTP support. Local WASM cannot make HTTP requests.".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn http_post(url: &str, body: &str) -> Result<String, String> {
    use std::process::Command;
    
    let output = Command::new("curl")
        .args([
            "-s", "-X", "POST",
            "-H", "Content-Type: application/json",
            "-d", body,
            url
        ])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;
    
    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| format!("Invalid UTF-8: {}", e))
    } else {
        Err(format!("curl error: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

// ============================================
// Session Management
// ============================================

fn create_session(account_id: &str) -> String {
    let now = current_timestamp();
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}:session", account_id, now));
    let token = hex::encode(hasher.finalize());
    
    unsafe {
        if SESSIONS.is_none() {
            SESSIONS = Some(HashMap::new());
        }
        if let Some(ref mut sessions) = SESSIONS {
            sessions.insert(account_id.to_string(), token.clone());
        }
    }
    
    token
}

// ============================================
// Utility Functions
// ============================================

fn get_account_id() -> String {
    std::env::var("NEAR_ACCOUNT_ID")
        .unwrap_or_else(|_| "kampouse.near".to_string())
}

fn current_timestamp() -> u64 {
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

fn log_audit(event: &str, account_id: &str) {
    eprintln!("[AUDIT] {} - {} - {}", current_timestamp(), account_id, event);
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64() {
        assert_eq!(base64_encode("hello"), "aGVsbG8=");
        assert_eq!(base64_encode("{\"test\":1}"), "eyJ0ZXN0IjoxfQ==");
    }

    #[test]
    fn test_session() {
        let token = create_session("test.near");
        assert_eq!(token.len(), 64);
    }
}
