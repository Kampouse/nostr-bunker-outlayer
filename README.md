# NEAR + Nostr Bunker - Hybrid Architecture

**Complete NIP-46 compatible Nostr bunker using OutLayer for secure signing + Cloudflare Workers for WebSocket bridging.**

---

## 🏗 Architecture

This implementation uses a **hybrid approach** combining the best of both worlds:

### Option 1: OutLayer (Secure Signing) - Rust/WASM

- **Purpose**: Secure, verifiable event signing
- **Technology**: Rust compiled to WASM, running in Intel TDX TEE
- **API**: HTTPS only
- **Security**: Keys stored in hardware-encrypted TEE

### Option 2: Cloudflare Workers (WebSocket Bridge) - TypeScript

- **Purpose**: WebSocket compatibility for existing NIP-46 clients
- **Technology**: Cloudflare Workers (TypeScript)
- **API**: WebSocket (NIP-46 protocol)
- **Function**: Translates WebSocket to HTTPS calls to OutLayer

---

## 🔄 How It Works

```
┌─────────────────┐
│  Damus/Snort    │ (expects WebSocket)
│  Amethyst/etc   │
└────────┬────────┘
         │ WebSocket (NIP-46)
         ▼
┌─────────────────────────────┐
│  Cloudflare Worker (Bridge)  │
│  - WebSocket Server          │
│  - HTTPS Client               │
└────────┬────────────────────┘
         │ HTTPS
         ▼
┌─────────────────────────────┐
│  OutLayer Worker (Secure)   │
│  - Rust/WASM in TEE          │
│  - Calls v1.signer via RPC     │
│  - Returns signature           │
└─────────────────────────────┘
         │ NEAR RPC
         ▼
┌─────────────────────────────┐
│  v1.signer (NEAR MPC)       │
│  - Threshold signing         │
└─────────────────────────────┘
```

---

## 🚀 Deployment

### Prerequisites

1. **Cloudflare account** (FREE tier works)
2. **OutLayer account** (FREE tier available)
3. **NEAR account** for relayer (~1 NEAR for gas)

### Step 1: Build OutLayer Worker

```bash
cd outlayer-worker

# Add WASM target
rustup target add wasm32-wasip2

# Build
cargo build --target wasm32-wasip2 --release

# Test locally
wasmtime target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm
```

### Step 2: Deploy to OutLayer

1. Upload `target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm` to OutLayer dashboard
2. Configure environment variables:
   - `RELAYER_ACCOUNT_ID=your-relayer.near`
   - `RELAYER_PRIVATE_KEY=ed25519:...`
3. Get API URL: `https://api.outlayer.fastnear.com/call/owner/nostr-bunker`

### Step 3: Deploy Cloudflare Bridge

```bash
cd ../cloudflare-bridge

# Configure
cp wrangler.toml.example wrangler.toml
# Edit wrangler.toml with your OutLayer API URL

# Deploy
wrangler deploy
```

Result: `wss://nostr-bunker-bridge.your-subdomain.workers.dev`

---

## 📖 Usage

### For Users

**In any NIP-46 compatible Nostr client:**

```
bunker://alice.near@nostr-bunker-bridge.your-subdomain.workers.dev
```

**First time:**
1. Client connects via WebSocket
2. Bridge forwards to OutLayer
3. User sees auth prompt
4. User logs in with NEAR wallet
5. ✅ Authenticated for 30 days

**Subsequent uses:**
- Seamless signing (no re-auth needed)
- Works with Damus, Snort, Amethyst, etc.

---

## 🔧 Configuration

### OutLayer Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `RELAYER_ACCOUNT_ID` | NEAR account for gas | `relayer.near` |
| `RELAYER_PRIVATE_KEY` | Private key for signing | `ed25519:...` |

### Cloudflare Worker Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `OUTLAYER_API_URL` | Your OutLayer endpoint | `https://api.outlayer.com/call/...` |
| `PAYMENT_KEY` | OutLayer payment key | `alice.near:1:K7x...` |

---

## 🔐 Security

### OutLayer (Secure Signing)

- ✅ **TEE Encryption**: Keys stored in hardware-encrypted enclave
- ✅ **Verifiable Execution**: Cryptographic proof of execution
- ✅ **No Key Exposure**: Private keys never leave TEE
- ✅ **Upgradeable**: Code can be updated without losing keys

### Cloudflare Workers (Bridge)

- ✅ **Protocol Translation**: Only translates WebSocket ↔ HTTPS
- ✅ **No Signing**: Never handles private keys
- ✅ **Stateless**: No persistent data stored

---

## 📊 Comparison

| Feature | Cloudflare Only | OutLayer + Bridge |
|---------|----------------|-------------------|
| **Language** | TypeScript | Rust + TypeScript |
| **Security** | Env vars (encrypted) | TEE (hardware) ✅ |
| **Verifiable** | No | Yes ✅ |
| **WebSocket** | Native ✅ | Via bridge ✅ |
| **Cost** | FREE | FREE |
| **Complexity** | Low | Medium |

---

## 🧪 Testing

### Test OutLayer Directly

```bash
curl -X POST https://api.outlayer.com/call/owner/nostr-bunker \
  -H "Content-Type: application/json" \
  -H "X-Payment-Key: alice.near:1:K7x..." \
  -d '{"input":{"method":"get_public_key","params":[]}}'
```

### Test Bridge

```bash
wscat -c wss://nostr-bunker-bridge.workers.dev
> {"id":1,"method":"get_public_key","params":[]}
< {"id":1,"result":"abc123..."}
```

---

## 📁 Project Structure

```
nostr-bunker-outlayer/
├── outlayer-worker/          # Rust implementation
│   ├── src/
│   │   └── main.rs           # Core signing logic
│   ├── wit/
│   │   └── world.wit         # Interface definitions
│   └── Cargo.toml            # Dependencies
│
├── cloudflare-bridge/        # TypeScript bridge
│   ├── src/
│   │   └── worker.ts         # WebSocket → HTTPS bridge
│   └── wrangler.toml         # Deployment config
│
└── README.md                 # This file
```

---

## 🎯 Benefits

### Why This Hybrid Approach?

1. **Maximum Compatibility**: Works with ALL existing NIP-46 clients
2. **Maximum Security**: Keys stored in hardware TEE
3. **Verifiable**: Cryptographic proof of execution
4. **Zero Cost**: Both Cloudflare and OutLayer have free tiers
5. **Upgradeable**: Update code without losing keys
6. **Redundant**: Can run multiple bridges if needed

---

## 🐛 Troubleshooting

### "Relayer not configured"

**Solution**: Set environment variables in OutLayer dashboard

### "WebSocket connection failed"

**Solution**: Check Cloudflare Worker logs: `wrangler tail`

### "Signature verification failed"

**Solution**: Verify relayer account has NEAR for gas

---

## 📞 Support

- **Issues**: https://github.com/your-repo/nostr-bunker-outlayer/issues
- **OutLayer Docs**: https://outlayer.fastnear.com/docs
- **NIP-46 Spec**: https://github.com/nostr-protocol/nips/blob/master/46.md

---

## 📜 License

MIT

---

## 🙏 Credits

- OutLayer for secure TEE execution
- Cloudflare Workers for WebSocket support
- NEAR Protocol for MPC infrastructure
- Nostr community for NIP-46 specification
