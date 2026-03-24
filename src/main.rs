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

fn derive_pubkey(account_id: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    account_id.hash(&mut hasher);
    format!("{:016x}{:016x}{:016x}{:016x}", 
        hasher.finish(), hasher.finish(), hasher.finish(), hasher.finish())
}

fn hex_to_npub(hex: &str) -> String {
    format!("npub1{}", &hex[..32])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: Request = serde_json::from_str(&input)?;
    
    let near_account = std::env::var("NEAR_SENDER_ID")
        .or_else(|_| std::env::var("OUTLAYER_PROJECT_OWNER"))
        .unwrap_or_else(|_| "kampouse.near".to_string());
    let pubkey = derive_pubkey(&near_account);
    
    let response = match request.method.as_str() {
        "ping" => Response {
            id: request.id,
            result: Some(serde_json::json!({
                "version": "1.2.0",
                "methods": ["ping", "get_public_key", "get_identity", "create_identity_proof", "sign_event", "create_session"]
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
                "near_account": near_account,
                "nostr_pubkey": pubkey,
                "nostr_npub": hex_to_npub(&pubkey),
                "verification_url": format!("https://near.social/#/kampouse.near")
            })),
            error: None,
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
                        ["i", &near_account, "NEAR"],
                        ["proxy", format!("bunker://{}@nostr-bunker-bridge.kj95hgdgnn.workers.dev", near_account)]
                    ],
                    "content": serde_json::to_string(&serde_json::json!({
                        "name": near_account.split('.').next().unwrap_or(&near_account),
                        "about": format!("NEAR account: {}", near_account),
                        "picture": format!("https://near.social/img/{}", near_account)
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
                let pk = event["pubkey"].as_str().unwrap_or(&pubkey);
                let created_at = event["created_at"].as_u64().unwrap_or(0);
                let kind = event["kind"].as_u64().unwrap_or(0) as u16;
                let tags = event["tags"].as_array().cloned().unwrap_or_default();
                let content = event["content"].as_str().unwrap_or("");
                
                let serialized = serde_json::to_string(&serde_json::json!([0, pk, created_at, kind, tags, content]))?;
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                serialized.hash(&mut hasher);
                let event_id = format!("{:064x}", hasher.finish());
                let sig = format!("sig_{}_{}", &event_id[..32], &pk[..32]);
                
                Response {
                    id: request.id,
                    result: Some(serde_json::json!({
                        "id": event_id, "pubkey": pk, "created_at": created_at,
                        "kind": kind, "tags": tags, "content": content, "sig": sig
                    })),
                    error: None,
                }
            } else {
                Response { id: request.id, result: None, error: Some("Missing event".to_string()) }
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
