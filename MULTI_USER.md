# Nostr Bunker with MPC - Multi-User

A multi-user Nostr bunker that uses NEAR's v1.signer MPC for key management.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              SETUP (one-time per user)                  │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  1. User calls bunker: "connect alice.near"            │
│     → Returns authorization instructions               │
│                                                         │
│  2. User authorizes via NEAR wallet:                   │
│     v1.signer.add_allowance(bunker-relayer.near)       │
│                                                         │
│  3. User can now sign Nostr events automatically       │
│                                                         │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│              DAILY USAGE (automated)                    │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  Damus/Primal → Bunker URL                             │
│                    ↓                                    │
│              TEE (bunker-relayer.near)                 │
│                    ↓                                    │
│         Signs tx as relayer with stored key            │
│                    ↓                                    │
│         v1.signer.sign("nostr/alice.near")             │
│                    ↓                                    │
│              MPC Network signs                         │
│                    ↓                                    │
│         Returns Schnorr signature                      │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

## Features

- ✅ **Multi-user** - Anyone can connect and authorize
- ✅ **MPC security** - Keys distributed across MPC network
- ✅ **Automated signing** - Apps can sign without user present
- ✅ **Works with all Nostr clients** - Damus, Primal, etc.
- ✅ **User control** - Can revoke authorization anytime

## Setup

### For Users

```bash
# 1. Authorize the bunker (one-time)
./authorize.sh your-account.near

# 2. Get your bunker URL
# It will look like:
# bunker://pubkey@bunker-relayer.near?relay=wss://nostr-bunker.kj95hgdgnn.workers.dev

# 3. Use in any Nostr client that supports bunkers
```

### For Operators

```bash
# 1. Create relayer account
near create-account bunker-relayer.near --useFaucet

# 2. Fund with NEAR for gas
near send your-account.near bunker-relayer.near 10

# 3. Store private key in TEE
export RELAYER_PRIVATE_KEY="ed25519:..."

# 4. Deploy to OutLayer TEE
# (OutLayer deployment instructions)
```

## API

### connect
Initialize connection and get authorization instructions.

```json
{
  "method": "connect",
  "params": ["alice.near"]
}
```

Response:
```json
{
  "status": "pending_authorization",
  "nostr_pubkey": "...",
  "authorization_instructions": {
    "method": "v1.signer.add_allowance",
    "args": {...}
  }
}
```

### check_authorization
Check if user has authorized the bunker.

```json
{
  "method": "check_authorization",
  "params": ["alice.near"]
}
```

### get_identity
Get user's full Nostr identity.

```json
{
  "method": "get_identity",
  "params": ["alice.near"]
}
```

### sign_event
Sign a Nostr event (requires prior authorization).

```json
{
  "method": "sign_event",
  "params": [
    "alice.near",
    {
      "pubkey": "...",
      "created_at": 1234567890,
      "kind": 1,
      "tags": [],
      "content": "Hello from MPC!"
    }
  ]
}
```

## Security Model

| Layer | What | Security |
|-------|------|----------|
| **MPC** | Key generation & signing | Distributed threshold (no single point of failure) |
| **TEE** | Stores relayer key | Hardware isolation |
| **Allowance** | User authorization | User controls access, can revoke |
| **Audit** | All operations logged | Verifiable in TEE |

## Costs

- **One-time setup**: ~0.001 NEAR (add_allowance transaction)
- **Per signature**: ~0.0003 NEAR (gas for sign transaction)
- **Monthly**: ~1 NEAR for typical usage (1000 signatures)

## Revocation

Users can revoke bunker access anytime:

```bash
near call v1.signer remove_allowance '{
  "account_id": "bunker-relayer.near"
}' --accountId your-account.near --networkId mainnet
```

## Development Status

- ✅ Multi-user architecture
- ✅ Authorization flow
- ✅ Session management
- ⚠️ Transaction signing (needs ed25519 implementation)
- ⚠️ TEE deployment

## Next Steps

1. Implement ed25519 transaction signing in Rust
2. Deploy to OutLayer TEE
3. Test with real Nostr clients (Damus, Primal)
4. Add rate limiting and usage tracking
