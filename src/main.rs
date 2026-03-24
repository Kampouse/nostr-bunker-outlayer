use std::io::{self, Read, Write};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use k256::schnorr::{SigningKey, Signature, signature::Signer};

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

// Derive keypair from any NEAR account
fn derive_keypair(account_id: &str) -> (String, SigningKey) {
    let data = format!("nostr-bunker:{}:v2", account_id);
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let seed = hasher.finalize();
    
    let signing_key = SigningKey::from_bytes((&seed[..]).into()).unwrap();
    let pubkey_bytes = signing_key.verifying_key().to_bytes();
    let pubkey_hex = hex::encode(pubkey_bytes);
    
    (pubkey_hex, signing_key)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: Request = serde_json::from_str(&input)?;
    
    // Get account_id from first param (all methods require it now)
    let account_id = request.params.get(0)
        .and_then(|p| p.as_str())
        .unwrap_or("kampouse.near");
    
    let (pubkey, _) = derive_keypair(account_id);
    
    let response = match request.method.as_str() {
        "ping" => Response {
            id: request.id,
            result: Some(serde_json::json!({
                "version": "3.0.0",
                "methods": ["ping", "get_public_key", "get_identity", "get_private_key", "create_identity_proof", "sign_event", "create_session"],
                "multi_user": true
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
                "verification_url": format!("https://near.social/#/{}", account_id)
            })),
            error: None,
        },
        
        "get_private_key" => {
            let (_, signing_key) = derive_keypair(account_id);
            let privkey_bytes = signing_key.to_bytes();
            let privkey_hex = hex::encode(privkey_bytes);
            
            Response {
                id: request.id,
                result: Some(serde_json::json!({
                    "private_key": privkey_hex,
                    "nsec": format!("nsec1{}", &privkey_hex[..58]),
                    "warning": "⚠️ Never share this private key with anyone!"
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
                let tags = event["tags"].as_array().cloned().unwrap_or_default();
                let content = event["content"].as_str().unwrap_or("");
                
                let serialized = serde_json::to_string(&serde_json::json!([
                    0, pk, created_at, kind, &tags, content
                ]))?;
                
                let mut hasher = Sha256::new();
                hasher.update(serialized.as_bytes());
                let event_id_bytes = hasher.finalize();
                let event_id = hex::encode(&event_id_bytes);
                
                let (_, signing_key) = derive_keypair(account_id);
                let signature: Signature = signing_key.sign(&event_id_bytes);
                let sig_hex = hex::encode(signature.to_bytes());
                
                Response {
                    id: request.id,
                    result: Some(serde_json::json!({
                        "id": event_id,
                        "pubkey": pk,
                        "created_at": created_at,
                        "kind": kind,
                        "tags": tags,
                        "content": content,
                        "sig": sig_hex
                    })),
                    error: None,
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
