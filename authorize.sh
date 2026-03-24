#!/bin/bash
# authorize-bunker.sh
# One-time setup: Authorize bunker-relayer to sign on your behalf

RELAYER="bunker-relayer.near"
ACCOUNT_ID=${1:-$(near account get-account-id 2>/dev/null)}

if [ -z "$ACCOUNT_ID" ]; then
    echo "Usage: $0 <your-account.near>"
    exit 1
fi

echo "=========================================="
echo "Nostr Bunker Authorization"
echo "=========================================="
echo ""
echo "Account: $ACCOUNT_ID"
echo "Relayer: $RELAYER"
echo ""
echo "This will authorize the bunker to sign Nostr events on your behalf."
echo "You only need to do this ONCE."
echo ""
read -p "Continue? (y/n) " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

echo ""
echo "Step 1: Authorizing relayer in v1.signer..."
echo ""

# Add allowance for relayer
near call v1.signer add_allowance '{
  "allowance": {
    "account_id": "'"$RELAYER"'",
    "allowance": "1000000000000000000000000000"
  }
}' --accountId "$ACCOUNT_ID" --networkId mainnet

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Authorization successful!"
    echo ""
    echo "Step 2: Getting your Nostr pubkey..."
    echo ""
    
    # Get derived pubkey
    PUBKEY=$(near view v1.signer derived_public_key '{
      "domain": 0,
      "path": "nostr/'"$ACCOUNT_ID"'"
    }' --networkId mainnet 2>/dev/null | grep -oP 'secp256k1:[^\)]+')
    
    echo "Your Nostr pubkey: $PUBKEY"
    echo ""
    echo "=========================================="
    echo "Setup Complete!"
    echo "=========================================="
    echo ""
    echo "You can now use your Nostr bunker:"
    echo ""
    echo "Bunker URL: bunker://${PUBKEY#secp256k1:}@${RELAYER}?relay=wss://nostr-bunker.kj95hgdgnn.workers.dev"
    echo ""
    echo "Use this URL in Damus, Primal, or any Nostr client that supports bunkers."
    echo ""
else
    echo ""
    echo "❌ Authorization failed. Please check your account and try again."
    exit 1
fi
