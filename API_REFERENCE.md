# OutLayer Worker API Reference

Complete API documentation for the NEAR + Nostr bunker OutLayer worker.

---

## Endpoints

### Base URL

```
https://api.outlayer.fastnear.com/call/{owner}/nostr-bunker
```

### Headers

| Header | Required | Description |
|--------|----------|-------------|
| `Content-Type` | Yes | `application/json` |
| `X-Payment-Key` | Yes | OutLayer payment key |

---

## Methods

### 1. get_public_key

Get Nostr public key for a NEAR account.

**Request:**
```json
{
  "input": {
    "id": 1,
    "method": "get_public_key",
    "params": []
  }
}
```

**Response:**
```json
{
  "id": 1,
  "result": "abc123def456...",
  "error": null
}
```

**Parameters:**
- `id` (optional): Request ID for response matching

**Result:** 64-character hex string (Nostr pubkey)

---

### 2. connect

Alias for `get_public_key` (NIP-46 compatibility).

**Request:**
```json
{
  "input": {
    "id": 1,
    "method": "connect",
    "params": []
  }
}
```

**Response:** Same as `get_public_key`

---

### 3. sign_event

Sign a Nostr event.

**Request:**
```json
{
  "input": {
    "id": 2,
    "method": "sign_event",
    "params": [
      {
        "pubkey": "abc123...",
        "created_at": 1234567890,
        "kind": 1,
        "tags": [],
        "content": "Hello Nostr!"
      }
    ]
  }
}
```

**Response:**
```json
{
  "id": 2,
  "result": {
    "id": "def456...",
    "pubkey": "abc123...",
    "created_at": 1234567890,
    "kind": 1,
    "tags": [],
    "content": "Hello Nostr!",
    "sig": "sig_def456..._alice.near"
  },
  "error": null
}
```

**Event Parameters:**
- `pubkey` (required): 64-char hex pubkey
- `created_at` (required): Unix timestamp
- `kind` (required): Event kind (1 = text note)
- `tags` (required): Array of tag arrays
- `content` (required): Event content

**Result:** Signed event object with `id` and `sig`

---

### 4. nip04_encrypt

Encrypt a message (NIP-04).

**Status:** Not implemented

**Request:**
```json
{
  "input": {
    "id": 3,
    "method": "nip04_encrypt",
    "params": ["pubkey", "plaintext"]
  }
}
```

**Response:**
```json
{
  "id": 3,
  "result": null,
  "error": "NIP-04 encryption not implemented"
}
```

---

### 5. nip04_decrypt

Decrypt a message (NIP-04).

**Status:** Not implemented

**Request:**
```json
{
  "input": {
    "id": 4,
    "method": "nip04_decrypt",
    "params": ["pubkey", "ciphertext"]
  }
}
```

**Response:**
```json
{
  "id": 4,
  "result": null,
  "error": "NIP-04 decryption not implemented"
}
```

---

## Error Responses

All errors follow this format:

```json
{
  "id": <request_id>,
  "result": null,
  "error": "Error message"
}
```

**Common Errors:**

| Error | Cause | Solution |
|-------|-------|----------|
| `Missing event parameter` | No event in params | Include event object |
| `Invalid event` | Malformed event | Check JSON format |
| `Unknown method` | Invalid method name | Use: get_public_key, sign_event, etc. |
| `NIP-04 encryption not implemented` | NIP-04 called | Wait for v1.1 |

---

## Rate Limits

| Tier | Requests/day | Rate |
|------|---------------|------|
| **Free** | 1,000 | $0 |
| **Basic** | 100,000 | $10/month |
| **Pro** | Unlimited | $50/month |

---

## Examples

### cURL

```bash
# Get public key
curl -X POST https://api.outlayer.fastnear.com/call/alice/nostr-bunker \
  -H "Content-Type: application/json" \
  -H "X-Payment-Key: alice.near:1:K7x..." \
  -d '{"input":{"id":1,"method":"get_public_key","params":[]}}'
```

### JavaScript

```javascript
const response = await fetch('https://api.outlayer.fastnear.com/call/alice/nostr-bunker', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'X-Payment-Key': 'alice.near:1:K7x...',
  },
  body: JSON.stringify({
    input: {
      id: 1,
      method: 'get_public_key',
      params: [],
    },
  }),
});

const { result } = await response.json();
console.log('Pubkey:', result);
```

### Python

```python
import requests

url = 'https://api.outlayer.fastnear.com/call/alice/nostr-bunker'
headers = {
    'Content-Type': 'application/json',
    'X-Payment-Key': 'alice.near:1:K7x...',
}
data = {
    'input': {
        'id': 1,
        'method': 'get_public_key',
        'params': [],
    }
}

response = requests.post(url, headers=headers, json=data)
result = response.json()['result']
print(f'Pubkey: {result}')
```

---

## Event Kinds

| Kind | Description | Usage |
|------|-------------|-------|
| 0 | Set metadata | Profile data |
| 1 | Text note | Regular posts |
| 3 | Recommend | Share events |
| 4 | Encrypted DM | Direct messages |
| 5 | Event deletion | Delete events |
| 6 | Repost | Share with comment |
| 7 | Reaction | React to events |

---

## Testing

### Test Get Public Key

```bash
./test-wasm.sh
# Or manually:
echo '{"id":1,"method":"get_public_key","params":[]}' | \
  wasmtime target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm
```

### Test Sign Event

```bash
echo '{"id":2,"method":"sign_event","params":[{"pubkey":"abc","created_at":123,"kind":1,"tags":[],"content":"test"}]}' | \
  wasmtime target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm
```

---

## Best Practices

1. **Always include `id`** for request tracking
2. **Handle errors** gracefully in your client
3. **Cache pubkeys** locally (they don't change)
4. **Use HTTPS** only (no HTTP)
5. **Keep payment key** secure (environment variable)

---

## Version

- **API Version:** 1.0.0
- **NIP-46 Version:** Supported
- **Last Updated:** March 23, 2026

---

## Support

- **Issues:** https://github.com/Kampouse/nostr-bunker-outlayer/issues
- **Docs:** This file
- **OutLayer:** https://outlayer.fastnear.com/docs
