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

// Derive valid secp256k1 Schnorr keypair from NEAR account
fn derive_keypair(account_id: &str) -> (String, SigningKey) {
    // Use SHA-256 to derive a 32-byte seed
    let data = format!("nostr-bunker:{}:v2", account_id);
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let seed = hasher.finalize();
    
    // Create Schnorr signing key from seed
    let signing_key = SigningKey::from_bytes((&seed[..]).into()).unwrap();
    
    // Get public key (32 bytes for Schnorr/x-only)
    let pubkey_bytes = signing_key.verifying_key().to_bytes();
    let pubkey_hex = hex::encode(pubkey_bytes);
    
    (pubkey_hex, signing_key)
}

// Derive just the pubkey
fn derive_pubkey(account_id: &str) -> String {
    let (pubkey, _) = derive_keypair(account_id);
    pubkey
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
                "version": "2.0.0",
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
                "nostr_npub": format!("npub1{}", &pubkey[..58]),
                "bunker_url": format!("bunker://{}?relay=wss://nostr-relay-production.up.railway.app", pubkey),
                "websocket_url": "wss://nostr-bunker-bridge.kj95hgdgnn.workers.dev",
                "verification_url": format!("https://near.social/#/kampouse.near")
            })),
            error: None,
        },
        "create_identity_proof" => {
            let created_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?.as_secs();
            
            let event = serde_json::json!({
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
            });
            
            Response {
                id: request.id,
                result: Some(event),
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
                
                // NIP-01 serialization: [0, pubkey, created_at, kind, tags, content]
                let serialized = serde_json::to_string(&serde_json::json!([
                    0, pk, created_at, kind, &tags, content
                ]))?;
                
                // Event ID = SHA256 of serialized
                let mut hasher = Sha256::new();
                hasher.update(serialized.as_bytes());
                let event_id_bytes = hasher.finalize();
                let event_id = hex::encode(&event_id_bytes);
                
                // Sign with Schnorr (BIP-340)
                let (_, signing_key) = derive_keypair(&near_account);
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
