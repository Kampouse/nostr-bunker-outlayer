#!/bin/bash

# Test script for OutLayer Worker
# Tests the compiled WASM binary locally

set -e

echo "=== Testing OutLayer Worker ==="
echo ""

WASM_FILE="outlayer-worker/target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm"

if [ ! -f "$WASM_FILE" ]; then
    echo "Error: WASM binary not found"
    echo "Run: cargo build --target wasm32-wasip2 --release"
    exit 1
fi

echo "✓ Found WASM binary: $WASM_FILE"
echo ""

# Test 1: get_public_key
echo "Test 1: get_public_key"
echo '{"id":1,"method":"get_public_key","params":[]}' | \
  wasmtime "$WASM_FILE" | jq '.'

echo ""

# Test 2: sign_event
echo "Test 2: sign_event"
cat <<EOF | wasmtime "$WASM_FILE" | jq '.'
{
  "id": 2,
  "method": "sign_event",
  "params": [{
    "pubkey": "abc123def456",
    "created_at": 1234567890,
    "kind": 1,
    "tags": [],
    "content": "Hello Nostr from NEAR!"
  }]
}
EOF

echo ""

# Test 3: Unknown method (should error)
echo "Test 3: Unknown method (should return error)"
echo '{"id":3,"method":"unknown_method","params":[]}' | \
  wasmtime "$WASM_FILE" | jq '.'

echo ""
echo "=== All tests complete ==="
