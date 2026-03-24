# OutLayer Integration Guide

Complete guide for deploying and using the NEAR + Nostr bunker with OutLayer TEE.

---

## 📋 Overview

This guide covers:

1. **OutLayer Worker** - Secure signing in TEE (Rust/WASM)
2. **Cloudflare Bridge** - WebSocket compatibility (TypeScript)
3. **Testing** - Local and production testing
4. **Deployment** - Step-by-step deployment
5. **Usage** - How to use with Nostr clients

---

## 🏗 Architecture

```
Nostr Client (Damus/Snort/Amethyst)
         ↓ WebSocket (NIP-46)
Cloudflare Worker (Bridge)
         ↓ HTTPS
OutLayer Worker (TEE)
         ↓ NEAR RPC
v1.signer (MPC)
```

---

## 📦 Prerequisites

### Required

- [x] OutLayer account (https://outlayer.fastnear.com)
- [x] Cloudflare account (https://dash.cloudflare.com)
- [x] NEAR account (~1 NEAR for gas)
- [x] Rust (latest stable)
- [x] Node.js 18+
- [x] wasmtime (for local testing)

### Install wasmtime

```bash
cargo install wasmtime
```

---

## 🧪 Local Testing

### 1. Build WASM

```bash
cd outlayer-worker
cargo build --target wasm32-wasip2 --release
```

**Output:** `target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm`

### 2. Run Tests

```bash
# Unit tests
cargo test

# Manual test
./test-wasm.sh
```

**Expected output:**
```json
{
  "id": 1,
  "result": "abc123...",
  "error": null
}
```

### 3. Test Individual Methods

**Get Public Key:**
```bash
echo '{"id":1,"method":"get_public_key","params":[]}' | \
  wasmtime target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm
```

**Sign Event:**
```bash
echo '{"id":2,"method":"sign_event","params":[{"pubkey":"abc","created_at":123,"kind":1,"tags":[],"content":"Hello!"}]}' | \
  wasmtime target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm
```

---

## 🚀 OutLayer Deployment

### Step 1: Create OutLayer Project

1. Go to https://outlayer.fastnear.com
2. Click "New Project"
3. Configure:
   - **Name:** `nostr-bunker`
   - **Upload:** `nostr_bunker_outlayer.wasm`

### Step 2: Configure Environment

Add environment variables:

```bash
# Required
NEAR_ACCOUNT_ID=your-relayer.near

# Optional (for future v1.signer integration)
NEAR_PRIVATE_KEY=ed25519:...
```

### Step 3: Get API URL

After deployment, copy your API URL:

```
https://api.outlayer.fastnear.com/call/YOUR_ACCOUNT/nostr-bunker
```

### Step 4: Create Payment Key

1. Go to "Payment Keys" section
2. Create new key with $10 USD balance
3. Copy the payment key: `alice.near:1:K7xR2mN9...`

---

## 🌉 Cloudflare Worker Deployment

### Step 1: Install Dependencies

```bash
cd cloudflare-bridge
npm install
```

### Step 2: Configure

Edit `wrangler.toml`:

```toml
[vars]
OUTLAYER_API_URL = "https://api.outlayer.fastnear.com/call/YOUR_ACCOUNT/nostr-bunker"
PAYMENT_KEY = "alice.near:1:K7xR2mN9..."
```

Or use secrets (recommended):

```bash
wrangler secret put OUTLAYER_API_URL
wrangler secret put PAYMENT_KEY
```

### Step 3: Test Locally

```bash
wrangler dev

# In another terminal
wscat -c ws://localhost:8787
> {"id":1,"method":"get_public_key","params":[]}
< {"id":1,"result":"abc123..."}
```

### Step 4: Deploy

```bash
wrangler deploy
```

**Output:**
```
✨ Success! Uploaded nostr-bunker-bridge
✨ Deployed to: https://nostr-bunker-bridge.your-subdomain.workers.dev
```

### Step 5: Verify

```bash
# Health check
curl https://nostr-bunker-bridge.your-subdomain.workers.dev/health

# WebSocket test
wscat -c wss://nostr-bunker-bridge.your-subdomain.workers.dev
```

---

## 🎯 Usage with Nostr Clients

### Damus (iOS)

1. Open Damus
2. Settings → Sign In → Remote Signer
3. Enter: `bunker://alice.near@nostr-bunker-bridge.your-subdomain.workers.dev`
4. Click "Connect"
5. First time: Auth prompt appears
6. Login with NEAR wallet
7. ✅ Connected

### Snort (Web)

1. Open https://snort.social
2. Settings → Add Remote Signer
3. Enter: `bunker://alice.near@nostr-bunker-bridge.your-subdomain.workers.dev`
4. First time: Auth prompt appears
5. Login with NEAR wallet
6. ✅ Connected

### Amethyst (Android)

Same as Damus

---

## 🧪 Integration Testing

### Test Suite

```bash
# Run full test suite
./test-wasm.sh

# Test bridge locally
cd cloudflare-bridge
wrangler dev
./test-bridge.sh
```

### Manual Tests

**1. Health Check**
```bash
curl https://your-bridge.workers.dev/health
# Expected: {"status":"ok",...}
```

**2. WebSocket Connection**
```bash
wscat -c wss://your-bridge.workers.dev
> {"id":1,"method":"get_public_key","params":[]}
< {"id":1,"result":"abc123..."}
```

**3. Sign Event**
```bash
wscat -c wss://your-bridge.workers.dev
> {"id":2,"method":"sign_event","params":[{"pubkey":"abc","created_at":123,"kind":1,"tags":[],"content":"Test"}]}
< {"id":2,"result":"{\"id\":\"...\",\"sig\":\"sig_...\"}"}
```

---

## 🔧 Configuration

### OutLayer Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `NEAR_ACCOUNT_ID` | Yes | NEAR account for signing |
| `NEAR_PRIVATE_KEY` | No | Private key (future use) |

### Cloudflare Worker Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `OUTLAYER_API_URL` | Yes | Your OutLayer endpoint |
| `PAYMENT_KEY` | Yes | OutLayer payment key |

---

## 🐛 Troubleshooting

### OutLayer: "WASM execution failed"

**Cause:** Binary too large or incompatible

**Solution:**
```bash
# Rebuild with size optimization
cargo build --target wasm32-wasip2 --release
# Binary should be <5MB
```

### OutLayer: "Payment key invalid"

**Cause:** Insufficient balance or invalid key

**Solution:**
1. Check payment key balance in OutLayer dashboard
2. Generate new payment key
3. Update Cloudflare secret

### Bridge: "WebSocket connection failed"

**Cause:** Worker not deployed or misconfigured

**Solution:**
```bash
# Check deployment
wrangler deployments list

# View logs
wrangler tail
```

### Client: "Authentication required"

**Cause:** Session not created

**Solution:**
1. Visit auth URL: `https://your-bridge.workers.dev/auth/alice.near`
2. Login with NEAR wallet
3. Return to client

---

## 📊 Monitoring

### Cloudflare Analytics

```bash
# View logs
wrangler tail

# View metrics
wrangler analytics
```

### OutLayer Dashboard

- Execution count
- Gas usage
- Error rate
- Payment balance

---

## 🔐 Security Checklist

- [ ] Payment key has limited balance
- [ ] Environment variables are secrets (not in code)
- [ ] HTTPS only (no HTTP)
- [ ] Authentication enabled
- [ ] Rate limiting configured (future)

---

## 💰 Cost Management

### OutLayer

- **Free tier:** 1000 calls/day
- **Paid:** $0.01/1000 calls
- **Monitor:** Dashboard shows usage

### Cloudflare

- **Free tier:** 100k requests/day
- **Monitor:** Workers dashboard

### NEAR Gas

- **Relayer:** ~1 NEAR/month
- **Monitor:** `near state relayer.near`

---

## 📈 Scaling

### Multiple Bridges

```bash
# Deploy backup bridge
wrangler deploy --name nostr-bunker-bridge-2

# Use load balancer or round-robin DNS
```

### High Availability

- Multiple Cloudflare Workers
- Multiple OutLayer instances
- Fallback relayer accounts

---

## 🗺 Roadmap

### v1.0 (Current)
- ✅ NIP-46 core methods
- ✅ WebSocket bridge
- ✅ Authentication page

### v1.1 (Next)
- [ ] NIP-04 encryption
- [ ] Session management
- [ ] Rate limiting
- [ ] v1.signer integration

### v2.0 (Future)
- [ ] Multiple key paths
- [ ] Custom policies
- [ ] Hardware keys

---

## 📞 Support

- **Issues:** https://github.com/Kampouse/nostr-bunker-outlayer/issues
- **OutLayer Docs:** https://outlayer.fastnear.com/docs
- **NIP-46 Spec:** https://github.com/nostr-protocol/nips/blob/master/46.md

---

## 📝 Checklist

Before going to production:

- [ ] OutLayer worker deployed
- [ ] Environment variables set
- [ ] Payment key created
- [ ] Cloudflare Worker deployed
- [ ] Secrets configured
- [ ] Health check passes
- [ ] WebSocket test passes
- [ ] Nostr client test passes
- [ ] Monitoring configured
- [ ] Backup plan ready

---

**Built with ❤️ by Kampouse**
