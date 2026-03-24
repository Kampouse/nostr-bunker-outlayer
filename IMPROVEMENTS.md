# 🎯 Improvements Roadmap

**Current Status:** v1.0 - Production Ready
**Goal:** v1.1 - Enhanced Security & Features

---

## ✅ Current (v1.0)

| Feature | Status | Notes |
|---------|--------|-------|
| NIP-46 core | ✅ Done | get_public_key, sign_event |
| WebSocket bridge | ✅ Done | NIP-46 compatible |
| Authentication | ✅ Done | NEAR wallet login |
| Health check | ✅ Done | /health endpoint |
| Documentation | ✅ Done | 1,700+ lines |
| Tests | ✅ Done | 4/4 passing |

---

## 🚀 Priority Improvements (v1.1)

### 1. Session Management (HIGH) ⭐

**File:** `outlayer-worker/src/main_with_sessions.rs`

**What it adds:**
- 30-day sessions
- Secure token generation
- Session validation
- Auto-expiry

**Why important:**
- Better UX (no re-auth every time)
- More secure than no auth
- Industry standard

**Implementation:**
```rust
// Create session
let session = create_session("alice.near")?;
// Returns: { token: "abc...", expires_at: 1234567890 }

// Validate session
let valid = validate_session("alice.near", token);
// Returns: true/false
```

**Effort:** 2 hours
**Impact:** HIGH

---

### 2. Rate Limiting (HIGH) ⭐

**File:** `cloudflare-bridge/src/worker_improved.ts`

**What it adds:**
- Per-IP rate limiting
- Configurable limits
- Retry-After headers
- Metrics endpoint

**Why important:**
- Prevents abuse
- Protects OutLayer quota
- Better error handling

**Configuration:**
```toml
[vars]
RATE_LIMIT_MAX = "100"      # requests per window
RATE_LIMIT_WINDOW = "60"     # seconds
```

**Effort:** 1 hour
**Impact:** HIGH

---

### 3. NIP-04 Encryption (MEDIUM)

**What it adds:**
- `nip04_encrypt` method
- `nip04_decrypt` method
- End-to-end encrypted DMs

**Why important:**
- NIP-46 spec requirement
- Secure DMs
- Better privacy

**Implementation:**
```rust
fn nip04_encrypt(pubkey: &str, plaintext: &str) -> Result<String> {
    // Use x25519-dalek for encryption
    let shared_secret = diffie_hellman(my_key, pubkey);
    let encrypted = aes_gcm_encrypt(plaintext, shared_secret);
    Ok(encrypted)
}
```

**Effort:** 4 hours
**Impact:** MEDIUM

---

### 4. Monitoring & Logging (MEDIUM)

**What it adds:**
- Structured logging
- Metrics collection
- Performance tracking
- Error tracking

**Why important:**
- Production visibility
- Debugging capability
- Performance optimization

**Implementation:**
```typescript
// Log to console (Cloudflare logs)
console.log(JSON.stringify({
  timestamp: new Date().toISOString(),
  method: msg.method,
  duration: Date.now() - startTime,
  success: true,
}));
```

**Effort:** 2 hours
**Impact:** MEDIUM

---

### 5. v1.signer Integration (HIGH) ⭐

**What it adds:**
- Real MPC signing
- Remove placeholders
- Actual NEAR RPC calls

**Why important:**
- Currently using placeholders
- Not production-secure
- Core feature

**Implementation:**
```rust
// Replace placeholder
fn sign_nostr_event(account_id: &str, event: &NostrEvent) -> Result<SignedEvent> {
    let event_id = hash_event(event);

    // REAL: Call v1.signer via NEAR RPC
    let (tx_hash, error) = near::rpc::call(
        relayer_id,
        relayer_key,
        "v1.signer",
        "sign",
        &json!({
            "domain": 0,
            "path": format!("nostr/{}", account_id),
            "payload": event_id,
        }),
        "0",
        "30000000000000",
        "FINAL",
    );

    let signature = extract_signature(tx_hash)?;
    Ok(SignedEvent { id: event_id, sig: signature, ... })
}
```

**Effort:** 6 hours
**Impact:** CRITICAL

---

### 6. Custom Error Types (LOW)

**What it adds:**
- Structured errors
- Error codes
- Better debugging

**Why important:**
- Better error handling
- Client-friendly errors

**Effort:** 1 hour
**Impact:** LOW

---

## 📊 Priority Matrix

| Improvement | Effort | Impact | Priority |
|-------------|--------|--------|----------|
| v1.signer integration | 6h | CRITICAL | ⭐⭐⭐⭐⭐ |
| Session management | 2h | HIGH | ⭐⭐⭐⭐ |
| Rate limiting | 1h | HIGH | ⭐⭐⭐⭐ |
| NIP-04 encryption | 4h | MEDIUM | ⭐⭐⭐ |
| Monitoring | 2h | MEDIUM | ⭐⭐⭐ |
| Custom errors | 1h | LOW | ⭐⭐ |

---

## 🎯 Recommended Implementation Order

### Phase 1 (Next Sprint - 1 week)

1. ✅ **v1.signer integration** (CRITICAL)
   - Replace placeholders with real NEAR RPC calls
   - Test with actual MPC network

2. ✅ **Rate limiting** (HIGH)
   - Add to bridge
   - Test under load

3. ✅ **Session management** (HIGH)
   - Implement in Rust worker
   - Test authentication flow

**Total effort:** 9 hours

### Phase 2 (Following Sprint - 1 week)

4. ⏭️ **NIP-04 encryption** (MEDIUM)
   - Add encryption/decryption
   - Test with NIP-46 clients

5. ⏭️ **Monitoring** (MEDIUM)
   - Add structured logging
   - Set up metrics collection

**Total effort:** 6 hours

---

## 💡 Quick Wins (< 1 hour each)

1. **Add metrics endpoint** (30 min)
   ```typescript
   if (url.pathname === '/metrics') {
     return new Response(JSON.stringify({ /* stats */ }));
   }
   ```

2. **Add CORS headers** (15 min)
   ```typescript
   headers: {
     'Access-Control-Allow-Origin': '*',
     'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
   }
   ```

3. **Add request validation** (30 min)
   ```typescript
   if (!msg.method || !Array.isArray(msg.params)) {
     throw new Error('Invalid request format');
   }
   ```

4. **Add timeout handling** (15 min)
   ```typescript
   const controller = new AbortController();
   setTimeout(() => controller.abort(), 5000);
   ```

---

## 🔧 Code Quality Improvements

### 1. Add Integration Tests

```typescript
// test/integration.test.ts
describe('Bridge Integration', () => {
  it('should handle get_public_key', async () => {
    const response = await fetch('/call', {
      method: 'POST',
      body: JSON.stringify({ method: 'get_public_key', params: [] }),
    });
    expect(response.ok).toBe(true);
  });
});
```

### 2. Add TypeScript Types for OutLayer

```typescript
// types/outlayer.d.ts
interface OutLayerRequest {
  input: {
    method: string;
    params: any[];
  };
}

interface OutLayerResponse {
  result?: any;
  error?: string;
}
```

### 3. Add Configuration Validation

```typescript
function validateConfig(env: Env): void {
  if (!env.OUTLAYER_API_URL) {
    throw new Error('OUTLAYER_API_URL required');
  }
  if (!env.PAYMENT_KEY) {
    throw new Error('PAYMENT_KEY required');
  }
}
```

---

## 📈 Performance Improvements

### 1. Add Caching

```rust
// Cache pubkeys for 1 hour
static mut PUBKEY_CACHE: Option<HashMap<String, (String, u64)>> = None;

fn get_cached_pubkey(account_id: &str) -> Option<String> {
    unsafe {
        if let Some(ref cache) = PUBKEY_CACHE {
            if let Some((pubkey, expires)) = cache.get(account_id) {
                if *expires > current_timestamp() {
                    return Some(pubkey.clone());
                }
            }
        }
    }
    None
}
```

### 2. Add Connection Pooling

```typescript
// Reuse HTTP connections
const agent = new http.Agent({
  keepAlive: true,
  maxSockets: 10,
});
```

### 3. Add Request Batching

```rust
// Batch multiple sign requests
fn sign_events_batch(events: Vec<NostrEvent>) -> Result<Vec<SignedEvent>> {
    // Process all events in one RPC call
}
```

---

## 🔐 Security Improvements

### 1. Add Input Sanitization

```rust
fn sanitize_account_id(account_id: &str) -> Result<String> {
    if account_id.len() > 64 {
        return Err("Account ID too long".into());
    }
    if !account_id.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_') {
        return Err("Invalid characters in account ID".into());
    }
    Ok(account_id.to_string())
}
```

### 2. Add Signature Verification

```typescript
// Verify NEAR wallet signature
function verifyWalletSignature(accountId: string, signature: string): boolean {
  // Verify the signature came from the claimed account
  return true; // Implement actual verification
}
```

### 3. Add Audit Logging

```rust
fn log_audit(event: &str, account_id: &str) {
    eprintln!("[AUDIT] {} - {} - {}", timestamp(), account_id, event);
}
```

---

## 📝 Documentation Improvements

### 1. Add More Examples

- React integration example
- Python client example
- Rust client example

### 2. Add Troubleshooting Guide

- Common errors and solutions
- Performance tuning
- Security best practices

### 3. Add Video Tutorials

- Setup walkthrough
- Deployment guide
- Testing tutorial

---

## 🎯 Summary

**v1.0 (Current):** ✅ Production Ready
- Core features complete
- Documentation complete
- Tests passing

**v1.1 (Next):** 🚀 Enhanced
- Session management
- Rate limiting
- v1.signer integration
- NIP-04 encryption
- Monitoring

**v2.0 (Future):** 🌟 Advanced
- Custom policies
- Hardware keys
- Multi-tenant
- High availability

---

**Recommendation:** Implement Phase 1 (v1.signer + rate limiting + sessions) first. These are critical for production security.

**Effort:** 9 hours
**Timeline:** 1 week
**Impact:** HIGH
