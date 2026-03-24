#!/bin/bash

# OutLayer Deployment Helper
# This script helps deploy the NEAR + Nostr bunker to OutLayer

set -e

echo "🚀 NEAR + Nostr Bunker - OutLayer Deployment Helper"
echo "===================================================="
echo ""

# Check prerequisites
echo "✓ Checking prerequisites..."

if ! command -v cargo &> /dev/null; then
    echo "✗ Rust not installed. Install from: https://rustup.rs"
    exit 1
fi

if ! command -v wasmtime &> /dev/null; then
    echo "⚠️  wasmtime not installed (optional for testing)"
    echo "   Install with: cargo install wasmtime"
fi

echo ""

# Step 1: Build
echo "📦 Step 1: Building WASM..."
cd outlayer-worker
cargo build --target wasm32-wasip2 --release
echo "✓ Build complete"
echo ""

# Step 2: Test
echo "🧪 Step 2: Running tests..."
cargo test --quiet
echo "✓ All tests passed"
echo ""

# Step 3: Show binary info
WASM_FILE="target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm"
if [ -f "$WASM_FILE" ]; then
    SIZE=$(wc -c < "$WASM_FILE" | awk '{print $1}')
    SIZE_MB=$(echo "scale=2; $SIZE / 1048576" | bc)
    echo "📊 Binary info:"
    echo "   File: $WASM_FILE"
    echo "   Size: ${SIZE_MB} MB"
    echo ""
else
    echo "✗ WASM binary not found at $WASM_FILE"
    exit 1
fi

# Step 4: Deployment instructions
echo "📝 Step 3: Deployment Instructions"
echo "=================================="
echo ""
echo "1. Go to OutLayer Dashboard:"
echo "   https://outlayer.fastnear.com"
echo ""
echo "2. Create new project:"
echo "   - Name: nostr-bunker"
echo "   - Upload: $WASM_FILE"
echo ""
echo "3. Configure environment variables:"
echo "   NEAR_ACCOUNT_ID=your-relayer.near"
echo "   (Optional) NEAR_PRIVATE_KEY=ed25519:..."
echo ""
echo "4. Get your API URL:"
echo "   https://api.outlayer.fastnear.com/call/YOUR_ACCOUNT/nostr-bunker"
echo ""
echo "5. Update Cloudflare Worker config:"
echo "   cd ../cloudflare-bridge"
echo "   # Edit wrangler.toml with your OutLayer URL"
echo "   wrangler secret put OUTLAYER_API_URL"
echo "   wrangler secret put PAYMENT_KEY"
echo ""
echo "6. Deploy Cloudflare Worker:"
echo "   npm install"
echo "   wrangler deploy"
echo ""
echo "7. Test with Nostr client:"
echo "   bunker://alice.near@your-bridge.workers.dev"
echo ""

# Optional: Test locally
if command -v wasmtime &> /dev/null; then
    read -p "Test WASM locally? (y/n): " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo ""
        echo "🧪 Testing WASM locally..."
        echo '{"id":1,"method":"get_public_key","params":[]}' | \
          wasmtime "$WASM_FILE" | jq '.'
        echo ""
        echo "✓ Local test complete"
    fi
fi

echo ""
echo "✅ Build and test complete!"
echo "📖 Next: Follow deployment instructions above"
