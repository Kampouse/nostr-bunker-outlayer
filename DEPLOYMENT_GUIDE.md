# 🚀 Deployment Guide - Complete v1.1

Complete guide to deploy NEAR + Nostr Bunker with all improvements.

---

## 📋 Prerequisites

- [x] Rust (latest stable)
- [x] Node.js 18+
- [x] Cloudflare account
- [x] OutLayer account
- [x] NEAR account (~1 NEAR)
- [x] wasmtime (for testing)

---

## 🏗 Build & Test

### Step 1: Build OutLayer Worker

```bash
cd outlayer-worker

# Add WASM target
rustup target add wasm32-wasip2

# Build
cargo build --target wasm32-wasip2 --release

# Test
cargo test

# Expected output:
# test result: ok. 11 passed; 0 failed
```

### Step 2: Test WASM Binary

```bash
# Test get_public_key
echo '{"id":1,"method":"get_public_key","params":[]}' | \
  wasmtime target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm

# Expected: {"id":null,"result":"abc123...","error":null}

# Test sign_event
echo '{"id":2,"method":"sign_event","params":[{"pubkey":"abc","created_at":123,"kind":1,"tags":[],"content":"test"}]}' | \
  wasmtime target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm

# Expected: {"id":null,"result":"{\"id\":\"...\",\"sig\":\"sig_...\"}","error":null}

# Test create_session
echo '{"id":3,"method":"create_session","params":[]}' | \
  wasmtime target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm

# Expected: {"id":null,"result":"{\"token\":\"...\",\"expires_at\":...}","error":null}
```

### Step 3: Build Cloudflare Bridge

```bash
cd ../cloudflare-bridge

# Install dependencies
npm install

# Check TypeScript
npx tsc --noEmit src/worker.ts

# Test locally
wrangler dev

# In another terminal
curl http://localhost:8787/health
# Expected: {"status":"ok",...}
```

---

## 🌐 Deploy OutLayer Worker

### Step 1: Create Project

1. Go to https://outlayer.fastnear.com
2. Click "New Project"
3. Name: `nostr-bunker`
4. Upload: `outlayer-worker/target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm`

### Step 2: Configure Environment

Add environment variables:

```bash
# Required
NEAR_ACCOUNT_ID=your-relayer.near

# Optional (for v1.signer integration)
RELAYER_ACCOUNT_ID=your-relayer.near
RELAYER_PRIVATE_KEY=ed25519:...
```

### Step 3: Create Payment Key

1. Go to "Payment Keys" section
2. Create new key
3. Fund with $10 USD
4. Copy key: `alice.near:1:K7xR2mN9...`

### Step 4: Get API URL

Copy your API URL:

```
https://api.outlayer.fastnear.com/call/YOUR_ACCOUNT/nostr-bunker
```

---

## 🌉 Deploy Cloudflare Bridge

### Step 1: Configure Secrets

```bash
cd cloudflare-bridge

# Set OutLayer API URL
wrangler secret put OUTLAYER_API_URL
# Enter: https://api.outlayer.fastnear.com/call/YOUR_ACCOUNT/nostr-bunker

# Set Payment Key
wrangler secret put PAYMENT_KEY
# Enter: alice.near:1:K7xR2mN9...
```

### Step 2: Deploy

```bash
wrangler deploy

# Expected output:
# ✨ Success! Uploaded nostr-bunker-bridge
# ✨ Deployed to: https://nostr-bunker-bridge.your-subdomain.workers.dev
```

### Step 3: Verify

```bash
# Health check
curl https://nostr-bunker-bridge.your-subdomain.workers.dev/health

# Expected: {"status":"ok","version":"1.1.0",...}

# Metrics
curl https://nostr-bunker-bridge.your-subdomain.workers.dev/metrics

# Expected: {"metrics":{"requestsTotal":0,...}}
```

---

## 🧪 Test Deployment

### Test WebSocket Connection

```bash
# Install wscat
npm install -g wscat

# Connect
wscat -c wss://nostr-bunker-bridge.your-subdomain.workers.dev

# Send test message
> {"id":1,"method":"get_public_key","params":[]}

# Expected response
< {"id":1,"result":"abc123...","error":null}
```

### Test Sign Event

```bash
wscat -c wss://nostr-bunker-bridge.your-subdomain.workers.dev

> {"id":2,"method":"sign_event","params":[{"pubkey":"abc123","created_at":1234567890,"kind":1,"tags":[],"content":"Hello!"}]}

< {"id":2,"result":"{\"id\":\"...\",\"sig\":\"sig_...\"}"}
```

### Test Rate Limiting

```bash
# Make 101 requests rapidly
for i in {1..101}; do
  curl -s https://nostr-bunker-bridge.your-subdomain.workers.dev/health > /dev/null
done

# 101st should fail
curl -i https://nostr-bunker-bridge.your-subdomain.workers.dev/health

# Expected: HTTP/1.1 429 Too Many Requests
```

---

## 🎯 Use with Nostr Clients

### Damus (iOS)

1. Open Damus
2. Settings → Sign In → Remote Signer
3. Enter: `bunker://alice.near@nostr-bunker-bridge.your-subdomain.workers.dev`
4. Click "Connect"
5. First time: Auth page opens
6. Login with NEAR wallet
7. ✅ Connected

### Snort (Web)

1. Open https://snort.social
2. Settings → Add Remote Signer
3. Enter: `bunker://alice.near@nostr-bunker-bridge.your-subdomain.workers.dev`
4. Login with NEAR
5. ✅ Connected

---

## 📊 Monitoring

### View Logs

```bash
# Cloudflare Worker logs
wrangler tail

# Expected output:
# {"timestamp":"2026-03-23T...","level":"info","message":"Request received","method":"get_public_key"}
# {"timestamp":"2026-03-23T...","level":"info","message":"Request successful","duration":45}
```

### View Metrics

```bash
curl https://nostr-bunker-bridge.your-subdomain.workers.dev/metrics

# Response:
{
  "metrics": {
    "requestsTotal": 123,
    "requestsSuccessful": 120,
    "requestsFailed": 3,
    "wsConnections": 2,
    "rateLimitedRequests": 5,
    "averageResponseTime": 42
  }
}
```

### OutLayer Dashboard

Monitor:
- Execution count
- Gas usage
- Error rate
- Payment balance

---

## 🐛 Troubleshooting

### "Rate limit exceeded"

**Cause:** Too many requests from same IP

**Solution:**
```bash
# Wait for window to reset (60 seconds)
# Or increase limit in wrangler.toml:
# RATE_LIMIT_MAX = "200"
```

### "Not authenticated"

**Cause:** No session created

**Solution:**
1. Visit auth URL: `https://your-bridge.workers.dev/auth/alice.near`
2. Login with NEAR wallet
3. Return to client

### "OutLayer error: 402"

**Cause:** Payment key insufficient balance

**Solution:**
1. Check payment key balance in OutLayer dashboard
2. Add funds
3. Update secret: `wrangler secret put PAYMENT_KEY`

### WebSocket connection fails

**Cause:** Worker not deployed or misconfigured

**Solution:**
```bash
# Check deployment
wrangler deployments list

# View logs
wrangler tail

# Redeploy
wrangler deploy
```

---

## 📈 Performance Tuning

### Optimize WASM Size

```bash
cd outlayer-worker

# Current: 2.1 MB
# Target: < 1 MB

# Add to Cargo.toml:
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true

# Rebuild
cargo build --target wasm32-wasip2 --release
```

### Increase Rate Limits

```toml
# wrangler.toml
[vars]
RATE_LIMIT_MAX = "200"     # Increase for production
RATE_LIMIT_WINDOW = "60"
```

### Add Caching

Already implemented:
- Pubkey caching (1 hour TTL)
- Session caching (30 days)

---

## 💰 Cost Management

### OutLayer

| Tier | Requests/day | Cost |
|------|---------------|------|
| Free | 1,000 | $0 |
| Paid | Unlimited | $0.01/1k |

**Monitor:**
```bash
# Check payment key balance
curl -H "X-Payment-Key: $PAYMENT_KEY" \
  https://api.outlayer.fastnear.com/balance
```

### Cloudflare

| Tier | Requests/day | Cost |
|------|---------------|------|
| Free | 100k | $0 |
| Paid | Unlimited | $5/mo |

**Monitor:**
- Workers dashboard
- Analytics tab

### NEAR Gas

| Usage | Monthly Cost |
|-------|--------------|
| ~100 signs/day | ~$0.50 |

**Monitor:**
```bash
near state relayer.near
```

---

## 🔄 Updates

### Update OutLayer Worker

```bash
cd outlayer-worker

# Make changes
vim src/main.rs

# Rebuild
cargo build --target wasm32-wasip2 --release

# Upload new WASM to OutLayer dashboard
# Keys and sessions are preserved
```

### Update Cloudflare Bridge

```bash
cd cloudflare-bridge

# Make changes
vim src/worker.ts

# Deploy
wrangler deploy
```

---

## ✅ Deployment Checklist

Before going to production:

- [ ] OutLayer worker uploaded
- [ ] Environment variables set
- [ ] Payment key created (funded)
- [ ] Cloudflare Worker deployed
- [ ] Secrets configured
- [ ] Health check passes
- [ ] WebSocket test passes
- [ ] Rate limiting tested
- [ ] Session creation tested
- [ ] Nostr client test passes
- [ ] Monitoring configured
- [ ] Logs visible
- [ ] Backup plan ready

---

## 🎯 Next Steps

After deployment:

1. **Monitor** - Check logs and metrics daily
2. **Scale** - Add more bridges if needed
3. **Optimize** - Tune rate limits and caching
4. **Backup** - Document recovery procedures
5. **Update** - Keep dependencies updated

---

## 📞 Support

- **Issues:** https://github.com/Kampouse/nostr-bunker-outlayer/issues
- **OutLayer Docs:** https://outlayer.fastnear.com/docs
- **Cloudflare Docs:** https://developers.cloudflare.com/workers/
- **NIP-46 Spec:** https://github.com/nostr-protocol/nips/blob/master/46.md

---

**Status: v1.1 COMPLETE**
**Ready: 🚀 YES**
