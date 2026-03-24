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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    
    let request: Request = serde_json::from_str(&input)?;
    
    let response = match request.method.as_str() {
        "get_public_key" => Response {
            id: request.id,
            result: Some(serde_json::json!("718c2e54e57288ce3fdb370ba90ed2cafb7d2d0787572cfc75f663f18d6dcb81")),
            error: None,
        },
        "create_session" => Response {
            id: request.id,
            result: Some(serde_json::json!({
                "token": "sess_test123",
                "account_id": "kampouse.near",
                "expires_at": 9999999999_u64
            })),
            error: None,
        },
        "sign_event" => {
            if let Some(event) = request.params.get(0) {
                Response {
                    id: request.id,
                    result: Some(serde_json::json!({
                        "id": "test_event_id",
                        "pubkey": event["pubkey"].as_str().unwrap_or(""),
                        "created_at": event["created_at"].as_u64().unwrap_or(0),
                        "kind": event["kind"].as_u64().unwrap_or(0) as u16,
                        "tags": event["tags"].as_array().unwrap_or(&vec![]),
                        "content": event["content"].as_str().unwrap_or(""),
                        "sig": "test_signature"
                    })),
                    error: None,
                }
            } else {
                Response {
                    id: request.id,
                    result: None,
                    error: Some("Missing event".to_string()),
                }
            }
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
