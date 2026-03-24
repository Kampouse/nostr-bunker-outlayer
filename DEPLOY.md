# Deployment Guide - NEAR + Nostr Bunker

Complete step-by-step guide to deploying the hybrid OutLayer + Cloudflare architecture.

---

## 📋 Prerequisites

### Required

- [ ] Cloudflare account (FREE)
- [ ] OutLayer account (FREE tier available)
- [ ] NEAR account with ~1 NEAR (for gas)
- [ ] Node.js 18+
- [ ] Rust (latest stable)

### Optional

- [ ] Custom domain (for better UX)

---

## 🚀 Step 1: Build OutLayer Worker

### 1.1 Install Rust Target

```bash
rustup target add wasm32-wasip2
```

### 1.2 Build

```bash
cd outlayer-worker
cargo build --target wasm32-wasip2 --release
```

Output: `target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm`

### 1.3 Test Locally (Optional)

```bash
# Install wasmtime
cargo install wasmtime

# Test
echo '{"method":"get_public_key","params":[]}' | \
  wasmtime target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm
```

---

## 🌐 Step 2: Deploy to OutLayer

### 2.1 Create OutLayer Account

1. Go to https://outlayer.fastnear.com
2. Sign in with NEAR wallet
3. Note your account ID (e.g., `alice.near`)

### 2.2 Create Project

1. Click "New Project"
2. Name: `nostr-bunker`
3. Upload WASM file from Step 1

### 2.3 Configure Environment Variables

In project settings, add:

```
RELAYER_ACCOUNT_ID=your-relayer.near
RELAYER_PRIVATE_KEY=ed25519:...
```

**To get relayer credentials:**

```bash
# Create new account
near create-account relayer.near --masterAccount your-account.near

# Fund with NEAR
near send your-account.near relayer.near 1

# Export key
near account export-account relayer.near
```

### 2.4 Get API URL

After deployment, copy your API URL:

```
https://api.outlayer.fastnear.com/call/alice.near/nostr-bunker
```

### 2.5 Test OutLayer

```bash
curl -X POST https://api.outlayer.fastnear.com/call/alice.near/nostr-bunker \
  -H "Content-Type: application/json" \
  -H "X-Payment-Key: alice.near:1:K7xR2..." \
  -d '{"input":{"method":"get_public_key","params":[]}}'
```

Expected: `{"id":null,"result":"abc123...","error":null}`

---

## 🌉 Step 3: Deploy Cloudflare Bridge

### 3.1 Install Dependencies

```bash
cd ../cloudflare-bridge
npm install
```

### 3.2 Configure

```bash
cp wrangler.toml.example wrangler.toml
```

Edit `wrangler.toml`:

```toml
name = "nostr-bunker-bridge"
main = "src/worker.ts"
compatibility_date = "2024-01-01"

[vars]
OUTLAYER_API_URL = "https://api.outlayer.fastnear.com/call/alice.near/nostr-bunker"
PAYMENT_KEY = "alice.near:1:K7xR2..."
```

### 3.3 Set Secrets (Production)

```bash
wrangler secret put OUTLAYER_API_URL
# Enter your OutLayer URL

wrangler secret put PAYMENT_KEY
# Enter your payment key
```

### 3.4 Deploy

```bash
wrangler deploy
```

Output:

```
✨ Success! Uploaded nostr-bunker-bridge
✨ Deployed to: https://nostr-bunker-bridge.your-subdomain.workers.dev
```

### 3.5 Test Bridge

```bash
# Install wscat
npm install -g wscat

# Test WebSocket
wscat -c wss://nostr-bunker-bridge.your-subdomain.workers.dev
```

Send message:

```json
{"id":1,"method":"get_public_key","params":[]}
```

Expected response:

```json
{"id":1,"result":"abc123...","error":null}
```

---

## 🎯 Step 4: Use with Nostr Clients

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

## 🔧 Step 5: Custom Domain (Optional)

### 5.1 Add Custom Domain

In Cloudflare dashboard:

1. Workers → nostr-bunker-bridge
2. Settings → Triggers → Custom Domains
3. Add domain: `bunker.yourdomain.com`

### 5.2 Update DNS

Add CNAME record:

```
bunker → your-subdomain.workers.dev
```

### 5.3 Update Bunker URL

```
bunker://alice.near@bunker.yourdomain.com
```

---

## 📊 Monitoring

### View Logs

```bash
# Cloudflare Worker logs
wrangler tail

# OutLayer logs
# Check OutLayer dashboard
```

### Health Check

```bash
# Bridge health
curl https://nostr-bunker-bridge.your-subdomain.workers.dev

# Expected: "NEAR + Nostr Bunker (OutLayer + Bridge)"
```

---

## 🐛 Troubleshooting

### "Relayer not configured"

**Problem**: OutLayer environment variables missing

**Solution**:
```bash
# In OutLayer dashboard, verify:
RELAYER_ACCOUNT_ID=relayer.near
RELAYER_PRIVATE_KEY=ed25519:...
```

### "WebSocket connection failed"

**Problem**: Cloudflare Worker not deployed or misconfigured

**Solution**:
```bash
# Check deployment
wrangler deployments list

# View logs
wrangler tail
```

### "OutLayer API error"

**Problem**: Payment key invalid or insufficient balance

**Solution**:
1. Generate new payment key in OutLayer dashboard
2. Fund payment key with USD
3. Update `PAYMENT_KEY` in Cloudflare secrets

### "Signature verification failed"

**Problem**: Relayer account lacks NEAR for gas

**Solution**:
```bash
# Check balance
near state relayer.near

# Fund if needed
near send your-account.near relayer.near 1
```

---

## 💰 Cost Breakdown

| Component | Free Tier | Paid |
|-----------|-----------|------|
| **Cloudflare Workers** | 100k requests/day | $5/mo for more |
| **OutLayer** | FREE tier | $0.01/1k calls |
| **NEAR Gas** | ~$0.50/month | - |
| **Total** | **$0.50/mo** | **$5.50/mo** |

---

## 🔄 Updates

### Update OutLayer Worker

```bash
cd outlayer-worker
cargo build --target wasm32-wasip2 --release

# Upload new WASM to OutLayer dashboard
# Keys are preserved automatically
```

### Update Cloudflare Bridge

```bash
cd cloudflare-bridge
wrangler deploy
```

---

## 📈 Scaling

### Multiple Bridges

Deploy multiple Cloudflare Workers for redundancy:

```bash
wrangler deploy --name nostr-bunker-bridge-2
```

Users can choose any:

```
bunker://alice.near@bridge-1.workers.dev
bunker://alice.near@bridge-2.workers.dev
```

### Load Balancing

Use Cloudflare Load Balancer to distribute traffic across multiple bridges.

---

## ✅ Checklist

After deployment, verify:

- [ ] OutLayer worker responds to HTTPS requests
- [ ] Cloudflare bridge accepts WebSocket connections
- [ ] NIP-46 clients can connect (Damus/Snort/Amethyst)
- [ ] Authentication works (first time)
- [ ] Event signing works (subsequent times)
- [ ] Custom domain configured (optional)

---

## 🎉 Done!

Your NEAR + Nostr bunker is now live!

**Bunker URL**: `bunker://alice.near@nostr-bunker-bridge.your-subdomain.workers.dev`

**Users can now**:
- Connect with any NIP-46 client
- Sign events gaslessly
- Use same NEAR account across devices
- Enjoy TEE-grade security

---

## 📞 Support

- **Issues**: https://github.com/your-repo/nostr-bunker-outlayer/issues
- **OutLayer Docs**: https://outlayer.fastnear.com/docs
- **Cloudflare Docs**: https://developers.cloudflare.com/workers/
