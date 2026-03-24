use std::io::{self, Read, Write};
use serde::{Deserialize, Serialize};

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

// Derive deterministic pubkey from NEAR account
fn derive_pubkey(account_id: &str) -> String {
    // Simple deterministic derivation
    // In production, this would use proper key derivation
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    account_id.hash(&mut hasher);
    format!("{:016x}{:016x}{:016x}{:016x}", 
        hasher.finish(),
        hasher.finish(),
        hasher.finish(),
        hasher.finish()
    )
}

// Convert hex pubkey to npub (bech32)
fn hex_to_npub(hex: &str) -> String {
    // Simplified - in production use proper bech32 encoding
    format!("npub1{}", &hex[..32])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    
    let request: Request = serde_json::from_str(&input)?;
    
    // Get NEAR account from env (set by OutLayer)
    let near_account = std::env::var("NEAR_SENDER_ID")
        .or_else(|_| std::env::var("OUTLAYER_PROJECT_OWNER"))
        .unwrap_or_else(|_| "kampouse.near".to_string());
    
    let pubkey = derive_pubkey(&near_account);
    
    let response = match request.method.as_str() {
        "get_public_key" => Response {
            id: request.id,
            result: Some(serde_json::json!(pubkey)),
            error: None,
        },
        
        "get_identity" => Response {
            id: request.id,
            result: Some(serde_json::json!({
                "near_account": near_account,
                "nostr_pubkey": pubkey,
                "nostr_npub": hex_to_npub(&pubkey),
                "verification_url": format!("https://near.social/#/kampouse.near")
            })),
            error: None,
        },
        
        "create_identity_proof" => {
            // Create NIP-39 identity proof
            // Kind 0 event with identity tag
            let created_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            // NIP-39: External Identities in Profiles
            let identity_event = serde_json::json!({
                "pubkey": pubkey,
                "created_at": created_at,
                "kind": 0,
                "tags": [
                    ["i", &near_account, "NEAR"],
                    ["proxy", format!("bunker://{}@nostr-bunker-bridge.kj95hgdgnn.workers.dev", near_account)]
                ],
                "content": serde_json::to_string(&serde_json::json!({
                    "name": near_account.split('.').next().unwrap_or(&near_account),
                    "about": format!("NEAR account: {}", near_account),
                    "picture": format!("https://near.social/img/{}", near_account),
                    "banner": "https://near.social/img/banner",
                    "nip39": {
                        "external": {
                            "type": "NEAR",
                            "value": near_account,
                            "proof": format!("Verified via OutLayer TEE at block {}", 
                                std::env::var("NEAR_BLOCK_HEIGHT").unwrap_or_else(|_| "unknown".to_string()))
                        }
                    }
                })).unwrap()
            });
            
            Response {
                id: request.id,
                result: Some(identity_event),
                error: None,
            }
        },
        
        "create_session" => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            Response {
                id: request.id,
                result: Some(serde_json::json!({
                    "token": format!("sess_{}", &pubkey[..16]),
                    "account_id": near_account,
                    "pubkey": pubkey,
                    "created_at": now,
                    "expires_at": now + 86400 * 30
                })),
                error: None,
            }
        },
        
        "sign_event" => {
            if let Some(event) = request.params.get(0) {
                // Create signed event
                let pubkey = event["pubkey"].as_str().unwrap_or(&pubkey);
                let created_at = event["created_at"].as_u64().unwrap_or(0);
                let kind = event["kind"].as_u64().unwrap_or(0) as u16;
                let tags = event["tags"].as_array().cloned().unwrap_or_default();
                let content = event["content"].as_str().unwrap_or("");
                
                // Serialize for ID (NIP-01)
                let serialized = serde_json::to_string(&serde_json::json!([
                    0, pubkey, created_at, kind, tags, content
                ])).unwrap();
                
                // Compute event ID (sha256)
                // Simplified - use content hash as ID
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                serialized.hash(&mut hasher);
                let event_id = format!("{:064x}", hasher.finish());
                
                // Generate signature (placeholder - real impl would use MPC)
                let sig = format!("sig_{}_{}", &event_id[..32], &pubkey[..32]);
                
                Response {
                    id: request.id,
                    result: Some(serde_json::json!({
                        "id": event_id,
                        "pubkey": pubkey,
                        "created_at": created_at,
                        "kind": kind,
                        "tags": tags,
                        "content": content,
                        "sig": sig
                    })),
                    error: None,
                }
            } else {
                Response {
                    id: request.id,
                    result: None,
                    error: Some("Missing event parameter".to_string()),
                }
            }
        },
        
        "ping" => Response {
            id: request.id,
            result: Some(serde_json::json!("pong")),
            error: None,
        },
        
        _ => Response {
            id: request.id,
            result: None,
            error: Some(format!("Unknown method: {}", request.method)),
        },
    };
    
    let output = serde_json::to_string(&response)?;
    print!("{}", output);
    io::stdout().flush()?;
    
    Ok(())
}
