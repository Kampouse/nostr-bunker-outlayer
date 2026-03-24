use std::io::{self, Read, Write};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

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

// Derive deterministic keys from NEAR account
fn derive_keys(account_id: &str) -> (String, String) {
    // Derive pubkey (32 bytes)
    let data = format!("nostr-bunker:{}:v2:pubkey", account_id);
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let pubkey = hex::encode(hasher.finalize());
    
    // Derive private key seed (32 bytes)
    // This seed would be used to register with v1.signer
    let data = format!("nostr-bunker:{}:v2:privkey", account_id);
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let privkey = hex::encode(hasher.finalize());
    
    (pubkey, privkey)
}

// Calculate event ID (SHA256 of serialized event)
fn calculate_event_id(pubkey: &str, created_at: u64, kind: u16, tags: &[Vec<String>], content: &str) -> String {
    let serialized = serde_json::to_string(&serde_json::json!([
        0, pubkey, created_at, kind, tags, content
    ])).unwrap_or_default();
    
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    hex::encode(hasher.finalize())
}

// Call v1.signer MPC to sign
// This would make an HTTP request to NEAR RPC
async fn sign_with_mpc(account_id: &str, message_hash: &str) -> Result<String, String> {
    // In OutLayer, we'd make HTTP request to NEAR RPC
    // For now, return placeholder indicating MPC is needed
    
    // The actual implementation would:
    // 1. POST to https://rpc.mainnet.near.org
    // 2. Call v1.signer contract
    // 3. Get Schnorr signature from MPC nodes
    
    // Placeholder: indicate this needs OutLayer HTTP support
    Err("MPC signing requires OutLayer HTTP support. Use get_private_key to export key for signing elsewhere.".to_string())
}

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
    
    let (pubkey, privkey) = derive_keys(&account_id);
    
    let response = match request.method.as_str() {
        "ping" => Response {
            id: request.id,
            result: Some(serde_json::json!({
                "version": "3.2.0",
                "methods": ["ping", "get_public_key", "get_identity", "get_private_key", "create_identity_proof", "sign_event", "create_session"],
                "multi_user": true,
                "signing": "v1.signer MPC (requires HTTP)"
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
                "websocket_url": "wss://nostr-bunker-bridge.kj95hgdgnn.workers.dev",
                "verification_url": format!("https://near.social/#/{}", account_id),
                "mpc_account": format!("{}.v1.signer", account_id)
            })),
            error: None,
        },
        
        "get_private_key" => {
            Response {
                id: request.id,
                result: Some(serde_json::json!({
                    "private_key": privkey,
                    "nsec": format!("nsec1{}", &privkey[..58]),
                    "warning": "⚠️ Never share this private key! Use to import into Nostr clients."
                })),
                error: None,
            }
        },
        
        "create_identity_proof" => {
            let created_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?.as_secs();
            
            Response {
                id: request.id,
                result: Some(serde_json::json!({
                    "pubkey": pubkey,
                    "created_at": created_at,
                    "kind": 0,
                    "tags": [
                        ["i", account_id, "NEAR"],
                        ["proxy", format!("bunker://{}@nostr-bunker-bridge.kj95hgdgnn.workers.dev", account_id)]
                    ],
                    "content": serde_json::to_string(&serde_json::json!({
                        "name": account_id.split('.').next().unwrap_or(account_id),
                        "about": format!("NEAR account: {}", account_id),
                        "picture": format!("https://near.social/img/{}", account_id)
                    })).unwrap()
                })),
                error: None,
            }
        },
        
        "create_session" => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?.as_secs();
            Response {
                id: request.id,
                result: Some(serde_json::json!({
                    "token": format!("sess_{}", &pubkey[..16]),
                    "account_id": account_id,
                    "pubkey": pubkey,
                    "created_at": now,
                    "expires_at": now + 86400 * 30
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
                
                // For MPC signing, we'd call v1.signer here
                // For now, return error indicating export key
                Response {
                    id: request.id,
                    result: None,
                    error: Some("MPC signing not yet implemented. Use get_private_key to export key and sign in Cloudflare Worker.".to_string()),
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
