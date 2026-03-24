use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;

// ============================================
// TYPE DEFINITIONS
// ============================================

#[derive(Deserialize)]
struct Request {
    method: String,
    params: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct Response<T: Serialize> {
    id: Option<u64>,
    result: Option<T>,
    error: Option<String>,
}

#[derive(Deserialize)]
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
// OUTLAYER IMPORTS
// ============================================

// Import OutLayer host functions
wit_bindgen::generate!({
    path: "../wit",
    world: "outlayer-host",
});

// ============================================
// MAIN ENTRY POINT
// ============================================

fn main() {
    let result = run();
    
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
    
    // Return output to OutLayer
    let output = serde_json::to_string(&response).unwrap();
    unsafe {
        outlayer_env_output(output.as_ptr(), output.len());
    }
}

// ============================================
// REQUEST HANDLER
// ============================================

fn run() -> Result<String, Box<dyn std::error::Error>> {
    // Read input from OutLayer
    let input = unsafe {
        let mut buffer = Vec::with_capacity(4096);
        let len = outlayer_env_input(buffer.as_mut_ptr(), buffer.capacity());
        buffer.set_len(len);
        String::from_utf8(buffer)?
    };
    
    let request: Request = serde_json::from_str(&input)?;
    
    match request.method.as_str() {
        "get_public_key" | "connect" => {
            let account_id = get_caller_account()?;
            let pubkey = get_or_create_pubkey(&account_id)?;
            Ok(pubkey)
        }
        
        "sign_event" => {
            let account_id = get_caller_account()?;
            let event: NostrEvent = serde_json::from_value(
                request.params.get(0).cloned().unwrap_or(serde_json::json!({}))
            )?;
            let signed_event = sign_event(&account_id, &event)?;
            Ok(serde_json::to_string(&signed_event)?)
        }
        
        "nip04_encrypt" => {
            Err("NIP-04 encryption not implemented".into())
        }
        
        "nip04_decrypt" => {
            Err("NIP-04 decryption not implemented".into())
        }
        
        _ => Err(format!("Unknown method: {}", request.method).into())
    }
}

// ============================================
// KEY MANAGEMENT
// ============================================

fn get_caller_account() -> Result<String, Box<dyn std::error::Error>> {
    unsafe {
        let mut buffer = Vec::with_capacity(256);
        let len = outlayer_env_var(
            b"NEAR_SENDER_ID\0".as_ptr(),
            buffer.as_mut_ptr(),
            buffer.capacity(),
        );
        
        if len == 0 {
            return Err("No caller account".into());
        }
        
        buffer.set_len(len);
        Ok(String::from_utf8(buffer)?)
    }
}

fn get_or_create_pubkey(account_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let storage_key = format!("pubkey:{}", account_id);
    
    // Check storage first
    let existing = unsafe {
        let mut buffer = Vec::with_capacity(128);
        let len = outlayer_storage_get(
            storage_key.as_ptr(),
            storage_key.len(),
            buffer.as_mut_ptr(),
            buffer.capacity(),
        );
        
        if len > 0 {
            buffer.set_len(len);
            return Ok(String::from_utf8(buffer)?);
        }
        None
    };
    
    if let Some(pubkey) = existing {
        return Ok(pubkey);
    }
    
    // Call v1.signer to derive pubkey
    let args = serde_json::json!({
        "domain": 0,
        "path": format!("nostr/{}", account_id),
    });
    
    let args_str = serde_json::to_string(&args)?;
    
    let (result, error) = unsafe {
        let mut result_buf = Vec::with_capacity(1024);
        let mut error_buf = Vec::with_capacity(512);
        
        let result_len = near_rpc_view(
            b"v1.signer\0".as_ptr(),
            b"derived_public_key\0".as_ptr(),
            args_str.as_ptr(),
            args_str.len(),
            b"final\0".as_ptr(),
            result_buf.as_mut_ptr(),
            result_buf.capacity(),
            error_buf.as_mut_ptr(),
            error_buf.capacity(),
        );
        
        result_buf.set_len(result_len);
        let result = String::from_utf8(result_buf)?;
        
        (result, String::new())
    };
    
    if !error.is_empty() {
        return Err(error.into());
    }
    
    // Parse result
    let response: serde_json::Value = serde_json::from_str(&result)?;
    let pubkey_bytes = response["result"]
        .as_array()
        .ok_or("Invalid pubkey format")?;
    
    let pubkey_hex: String = pubkey_bytes
        .iter()
        .map(|b| format!("{:02x}", b.as_u64().unwrap()))
        .collect();
    
    // Store for future use
    unsafe {
        outlayer_storage_set(
            storage_key.as_ptr(),
            storage_key.len(),
            pubkey_hex.as_ptr(),
            pubkey_hex.len(),
        );
    }
    
    Ok(pubkey_hex)
}

// ============================================
// EVENT SIGNING
// ============================================

fn sign_event(account_id: &str, event: &NostrEvent) -> Result<SignedEvent, Box<dyn std::error::Error>> {
    // 1. Serialize event (NIP-01)
    let serialized = serde_json::to_string(&[
        0u8,
        &event.pubkey,
        &event.created_at,
        &event.kind,
        &event.tags,
        &event.content,
    ])?;
    
    // 2. Hash with SHA-256
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let hash = hasher.finalize();
    let event_id = hex::encode(hash);
    
    // 3. Get relayer credentials
    let (relayer_id, relayer_key) = get_relayer_credentials()?;
    
    // 4. Call v1.signer via NEAR RPC
    let args = serde_json::json!({
        "domain": 0,
        "path": format!("nostr/{}", account_id),
        "payload": event_id,
    });
    
    let args_str = serde_json::to_string(&args)?;
    
    let (tx_hash, error) = unsafe {
        let mut tx_hash_buf = Vec::with_capacity(128);
        let mut error_buf = Vec::with_capacity(512);
        
        let tx_hash_len = near_rpc_call(
            relayer_id.as_ptr(),
            relayer_id.len(),
            relayer_key.as_ptr(),
            relayer_key.len(),
            b"v1.signer\0".as_ptr(),
            b"sign\0".as_ptr(),
            args_str.as_ptr(),
            args_str.len(),
            b"0\0".as_ptr(),
            b"30000000000000\0".as_ptr(),
            b"FINAL\0".as_ptr(),
            tx_hash_buf.as_mut_ptr(),
            tx_hash_buf.capacity(),
            error_buf.as_mut_ptr(),
            error_buf.capacity(),
        );
        
        tx_hash_buf.set_len(tx_hash_len);
        let tx_hash = String::from_utf8(tx_hash_buf)?;
        let error = String::from_utf8(error_buf)?;
        
        (tx_hash, error)
    };
    
    if !error.is_empty() {
        return Err(error.into());
    }
    
    // 5. Get transaction result to extract signature
    let signature = extract_signature_from_tx(&tx_hash, &relayer_id)?;
    
    Ok(SignedEvent {
        id: event_id,
        pubkey: event.pubkey.clone(),
        created_at: event.created_at,
        kind: event.kind,
        tags: event.tags.clone(),
        content: event.content.clone(),
        sig: signature,
    })
}

fn get_relayer_credentials() -> Result<(String, String), Box<dyn std::error::Error>> {
    unsafe {
        let mut id_buf = Vec::with_capacity(256);
        let mut key_buf = Vec::with_capacity(512);
        
        let id_len = outlayer_env_var(
            b"RELAYER_ACCOUNT_ID\0".as_ptr(),
            id_buf.as_mut_ptr(),
            id_buf.capacity(),
        );
        
        let key_len = outlayer_env_var(
            b"RELAYER_PRIVATE_KEY\0".as_ptr(),
            key_buf.as_mut_ptr(),
            key_buf.capacity(),
        );
        
        if id_len == 0 || key_len == 0 {
            return Err("Relayer credentials not configured".into());
        }
        
        id_buf.set_len(id_len);
        key_buf.set_len(key_len);
        
        Ok((String::from_utf8(id_buf)?, String::from_utf8(key_buf)?))
    }
}

fn extract_signature_from_tx(tx_hash: &str, signer_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Get transaction status
    let (result, error) = unsafe {
        let mut result_buf = Vec::with_capacity(4096);
        let mut error_buf = Vec::with_capacity(512);
        
        let result_len = near_rpc_tx_status(
            tx_hash.as_ptr(),
            tx_hash.len(),
            signer_id.as_ptr(),
            signer_id.len(),
            b"FINAL\0".as_ptr(),
            result_buf.as_mut_ptr(),
            result_buf.capacity(),
            error_buf.as_mut_ptr(),
            error_buf.capacity(),
        );
        
        result_buf.set_len(result_len);
        let result = String::from_utf8(result_buf)?;
        let error = String::from_utf8(error_buf)?;
        
        (result, error)
    };
    
    if !error.is_empty() {
        return Err(error.into());
    }
    
    // Parse signature from transaction logs
    let tx: serde_json::Value = serde_json::from_str(&result)?;
    
    let signature = tx["result"]["receipts_outcome"][0]["outcome"]["logs"][0]
        .as_str()
        .ok_or("No signature in transaction logs")?
        .to_string();
    
    Ok(signature)
}

// ============================================
// OUTLAYER HOST FUNCTIONS (STUBS)
// ============================================

// These are provided by OutLayer runtime
extern "C" {
    fn outlayer_env_input(ptr: *mut u8, len: usize) -> usize;
    fn outlayer_env_output(ptr: *const u8, len: usize);
    fn outlayer_env_var(key: *const u8, ptr: *mut u8, len: usize) -> usize;
    
    fn outlayer_storage_get(key: *const u8, key_len: usize, ptr: *mut u8, len: usize) -> usize;
    fn outlayer_storage_set(key: *const u8, key_len: usize, value: *const u8, value_len: usize);
    
    fn near_rpc_view(
        contract_id: *const u8,
        method_name: *const u8,
        args_json: *const u8,
        args_len: usize,
        finality: *const u8,
        result: *mut u8,
        result_len: usize,
        error: *mut u8,
        error_len: usize,
    ) -> usize;
    
    fn near_rpc_call(
        signer_id: *const u8,
        signer_key: *const u8,
        receiver_id: *const u8,
        method_name: *const u8,
        args_json: *const u8,
        args_len: usize,
        deposit: *const u8,
        gas: *const u8,
        wait_until: *const u8,
        tx_hash: *mut u8,
        tx_hash_len: usize,
        error: *mut u8,
        error_len: usize,
    ) -> usize;
    
    fn near_rpc_tx_status(
        tx_hash: *const u8,
        tx_hash_len: usize,
        sender_id: *const u8,
        sender_id_len: usize,
        wait_until: *const u8,
        result: *mut u8,
        result_len: usize,
        error: *mut u8,
        error_len: usize,
    ) -> usize;
}
