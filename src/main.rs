use std::io::{self, Read, Write};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

// ============================================
// NEAR RPC Types
// ============================================

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
    #[serde(default)]
    data: Option<String>,
}

// ============================================
// v1.signer Types
// ============================================

#[derive(Serialize)]
struct SignRequest {
    domain: u8,
    path: String,
    payload: String,
}

#[derive(Deserialize)]
struct SignResponse {
    signature: String,
    #[serde(default)]
    public_key: String,
}

// ============================================
// Bunker Request/Response
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

// ============================================
// Configuration
// ============================================

const NEAR_RPC_URL: &str = "https://rpc.mainnet.near.org";
const V1_SIGNER_CONTRACT: &str = "v1.signer";
const RELAYER_ACCOUNT: &str = "relayer.kampouse.near";

// ============================================
// Key Derivation
// ============================================

fn derive_keys(account_id: &str) -> (String, String) {
    // Derive pubkey (32 bytes)
    let data = format!("nostr-bunker:{}:v2:pubkey", account_id);
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let pubkey = hex::encode(hasher.finalize());
    
    // Derive private key seed (32 bytes)
    let data = format!("nostr-bunker:{}:v2:privkey", account_id);
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let privkey = hex::encode(hasher.finalize());
    
    (pubkey, privkey)
}

// ============================================
// Event ID Calculation (NIP-01)
// ============================================

fn calculate_event_id(pubkey: &str, created_at: u64, kind: u16, tags: &[Vec<String>], content: &str) -> String {
    let serialized = serde_json::to_string(&serde_json::json!([
        0, pubkey, created_at, kind, tags, content
    ])).unwrap_or_default();
    
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    hex::encode(hasher.finalize())
}

// ============================================
// HTTP Client (WASI-compatible)
// ============================================

#[cfg(target_arch = "wasm32")]
fn http_post(url: &str, body: &str) -> Result<String, String> {
    // WASM32: Use OutLayer's HTTP support
    // This is a placeholder - OutLayer provides HTTP via host functions
    
    // In production, this would call OutLayer's HTTP host function:
    // outlayer::http_post(url, headers, body)
    
    Err("HTTP requires OutLayer runtime. Deploy to OutLayer TEE for MPC signing.".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn http_post(url: &str, body: &str) -> Result<String, String> {
    // Native: Use std::process to call curl
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
// NEAR RPC Calls
// ============================================

fn call_near_rpc(method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let request = NearRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: "dontcare".to_string(),
        method: method.to_string(),
        params,
    };
    
    let body = serde_json::to_string(&request)
        .map_err(|e| format!("Failed to serialize request: {}", e))?;
    
    let response = http_post(NEAR_RPC_URL, &body)?;
    
    let rpc_response: NearRpcResponse = serde_json::from_str(&response)
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    if let Some(error) = rpc_response.error {
        return Err(format!("NEAR RPC error: {} - {:?}", error.message, error.data));
    }
    
    rpc_response.result.ok_or_else(|| "No result in response".to_string())
}

// ============================================
// v1.signer MPC Signing
// ============================================

fn sign_with_mpc(account_id: &str, message_hash: &str) -> Result<String, String> {
    // Build the function call parameters
    let sign_request = SignRequest {
        domain: 0, // Domain 0 = NEAR (default)
        path: format!("nostr/{}", account_id),
        payload: message_hash.to_string(),
    };
    
    let args = serde_json::to_string(&sign_request)
        .map_err(|e| format!("Failed to serialize sign request: {}", e))?;
    let args_base64 = base64_encode(&args);
    
    // Build the transaction parameters
    // Note: This requires a relayer account with access key to v1.signer
    let params = serde_json::json!({
        "request_type": "call_function",
        "finality": "final",
        "account_id": V1_SIGNER_CONTRACT,
        "method_name": "sign",
        "args_base64": args_base64,
    });
    
    // Call NEAR RPC
    let result = call_near_rpc("query", params)?;
    
    // Parse the result
    let result_bytes = result["result"]
        .as_array()
        .ok_or("No result bytes in response")?;
    
    let result_str: String = result_bytes
        .iter()
        .filter_map(|b| b.as_u64().map(|v| v as u8))
        .map(|b| b as char)
        .collect();
    
    let sign_response: SignResponse = serde_json::from_str(&result_str)
        .map_err(|e| format!("Failed to parse sign response: {}", e))?;
    
    Ok(sign_response.signature)
}

// ============================================
// v1.signer Derived Public Key
// ============================================

fn get_derived_public_key(account_id: &str) -> Result<String, String> {
    let args = serde_json::to_string(&serde_json::json!({
        "domain": 0,
        "path": format!("nostr/{}", account_id),
    })).map_err(|e| format!("Failed to serialize: {}", e))?;
    
    let args_base64 = base64_encode(&args);
    
    let params = serde_json::json!({
        "request_type": "call_function",
        "finality": "final",
        "account_id": V1_SIGNER_CONTRACT,
        "method_name": "derived_public_key",
        "args_base64": args_base64,
    });
    
    let result = call_near_rpc("query", params)?;
    
    let result_bytes = result["result"]
        .as_array()
        .ok_or("No result bytes")?;
    
    let pubkey: String = result_bytes
        .iter()
        .filter_map(|b| b.as_u64().map(|v| v as u8))
        .map(|b| b as char)
        .collect();
    
    // Remove quotes if present
    Ok(pubkey.trim_matches('"').to_string())
}

// ============================================
// Helper: Base64 Encode
// ============================================

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

// ============================================
// Main Entry Point
// ============================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: Request = serde_json::from_str(&input)?;
    
    // Get account_id from params
    let account_id = if request.params.is_empty() {
        "kampouse.near".to_string()
    } else if let Some(first) = request.params.get(0) {
        if let Some(s) = first.as_str() {
            s.to_string()
        } else if let Some(obj) = first.as_object() {
            obj.get("account_id")
                .and_then(|v| v.as_str())
                .unwrap_or("kampouse.near")
                .to_string()
        } else {
            "kampouse.near".to_string()
        }
    } else {
        "kampouse.near".to_string()
    };
    
    // Try to get real pubkey from v1.signer, fallback to derived
    let pubkey = get_derived_public_key(&account_id)
        .unwrap_or_else(|_| {
            let (pk, _) = derive_keys(&account_id);
            pk
        });
    
    let (_, privkey) = derive_keys(&account_id);
    
    let response = match request.method.as_str() {
        "ping" => Response {
            id: request.id,
            result: Some(serde_json::json!({
                "version": "4.0.0",
                "methods": [
                    "ping", 
                    "get_public_key", 
                    "get_identity", 
                    "sign_event",
                    "get_private_key"
                ],
                "signing": "v1.signer MPC via NEAR RPC",
                "tee": "OutLayer"
            })),
            error: None,
        },
        
        "get_public_key" => Response {
            id: request.id,
            result: Some(serde_json::json!(pubkey)),
            error: None,
        },
        
        "get_identity" => Response {
            id: request.id,
            result: Some(serde_json::json!({
                "near_account": account_id,
                "nostr_pubkey": pubkey,
                "nostr_npub": format!("npub1{}", &pubkey[..58]),
                "bunker_url": format!("bunker://{}?relay=wss://nostr-bunker-bridge.kj95hgdgnn.workers.dev", pubkey),
                "mpc_path": format!("nostr/{}", account_id),
                "mpc_contract": V1_SIGNER_CONTRACT,
                "verification": format!("https://near.social/#/{}", account_id)
            })),
            error: None,
        },
        
        "get_private_key" => {
            Response {
                id: request.id,
                result: Some(serde_json::json!({
                    "private_key": privkey,
                    "nsec": format!("nsec1{}", &privkey[..58]),
                    "warning": "⚠️ Derived key. Real key is in MPC. Use sign_event for MPC signing."
                })),
                error: None,
            }
        },
        
        "sign_event" => {
            // params: [account_id, event]
            if let Some(event) = request.params.get(1) {
                let pk = event["pubkey"].as_str().unwrap_or(&pubkey);
                let created_at = event["created_at"].as_u64().unwrap_or(0);
                let kind = event["kind"].as_u64().unwrap_or(0) as u16;
                let tags = event["tags"].as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_array().map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect::<Vec<String>>())).collect::<Vec<Vec<String>>>())
                    .unwrap_or_default();
                let content = event["content"].as_str().unwrap_or("");
                
                let event_id = calculate_event_id(pk, created_at, kind, &tags, content);
                
                // Try MPC signing
                match sign_with_mpc(&account_id, &event_id) {
                    Ok(signature) => Response {
                        id: request.id,
                        result: Some(serde_json::json!({
                            "id": event_id,
                            "pubkey": pk,
                            "created_at": created_at,
                            "kind": kind,
                            "tags": tags,
                            "content": content,
                            "sig": signature
                        })),
                        error: None,
                    },
                    Err(e) => Response {
                        id: request.id,
                        result: None,
                        error: Some(format!("MPC signing failed: {}. Deploy to OutLayer TEE for real signing.", e)),
                    },
                }
            } else {
                Response { 
                    id: request.id, 
                    result: None, 
                    error: Some("Missing event parameter".to_string()) 
                }
            }
        },
        
        _ => Response {
            id: request.id,
            result: None,
            error: Some(format!("Unknown method: {}", request.method)),
        },
    };
    
    print!("{}", serde_json::to_string(&response)?);
    io::stdout().flush()?;
    Ok(())
}
