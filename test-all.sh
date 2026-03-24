#!/bin/bash

# Comprehensive Test Suite for NEAR + Nostr Bunker
# Tests both OutLayer Worker and Cloudflare Bridge

set -e

echo "🧪 NEAR + Nostr Bunker - Test Suite"
echo "====================================="
echo ""

PASS=0
FAIL=0

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test helper
test_case() {
    local name="$1"
    local result="$2"

    if [ "$result" -eq 0 ]; then
        echo -e "${GREEN}✓${NC} $name"
        PASS=$((PASS + 1))
    else
        echo -e "${RED}✗${NC} $name"
        FAIL=$((FAIL + 1))
    fi
}

# ============================================
# RUST TESTS
# ============================================

echo "📦 Testing OutLayer Worker (Rust)"
echo "---------------------------------"
echo ""

cd outlayer-worker

# Build WASM
echo "Building WASM..."
cargo build --target wasm32-wasip2 --release > /dev/null 2>&1
test_case "WASM build" $?

# Run unit tests
echo "Running unit tests..."
cargo test --quiet 2>&1 | grep -q "test result: ok"
test_case "Unit tests pass" $?

# Test individual methods
WASM_FILE="target/wasm32-wasip2/release/nostr_bunker_outlayer.wasm"

if command -v wasmtime &> /dev/null; then
    echo "Testing methods with wasmtime..."

    # Test get_public_key
    RESULT=$(echo '{"id":1,"method":"get_public_key","params":[]}' | wasmtime "$WASM_FILE" 2>/dev/null)
    echo "$RESULT" | grep -q '"result"'
    test_case "get_public_key returns result" $?

    # Test sign_event
    RESULT=$(echo '{"id":2,"method":"sign_event","params":[{"pubkey":"abc","created_at":123,"kind":1,"tags":[],"content":"test"}]}' | wasmtime "$WASM_FILE" 2>/dev/null)
    echo "$RESULT" | grep -q '"sig"'
    test_case "sign_event returns signature" $?

    # Test create_session
    RESULT=$(echo '{"id":3,"method":"create_session","params":[]}' | wasmtime "$WASM_FILE" 2>/dev/null)
    echo "$RESULT" | grep -q '"token"'
    test_case "create_session returns token" $?

    # Test unknown method (should error)
    RESULT=$(echo '{"id":4,"method":"unknown","params":[]}' | wasmtime "$WASM_FILE" 2>/dev/null)
    echo "$RESULT" | grep -q '"error"'
    test_case "Unknown method returns error" $?
else
    echo -e "${YELLOW}⚠️  wasmtime not installed, skipping WASM tests${NC}"
fi

cd ..

echo ""

# ============================================
# TYPESCRIPT BRIDGE TESTS
# ============================================

echo "🌐 Testing Cloudflare Bridge (TypeScript)"
echo "------------------------------------------"
echo ""

cd cloudflare-bridge

# Check dependencies
if [ -d "node_modules" ]; then
    echo "Checking TypeScript compilation..."
    npx tsc --noEmit src/worker.ts > /dev/null 2>&1
    test_case "TypeScript compiles" $?
else
    echo -e "${YELLOW}⚠️  node_modules not found, run npm install${NC}"
fi

# Check wrangler config
if [ -f "wrangler.toml" ]; then
    test_case "wrangler.toml exists" 0
else
    test_case "wrangler.toml exists" 1
fi

cd ..

echo ""

# ============================================
# INTEGRATION TESTS (requires running services)
# ============================================

echo "🔗 Integration Tests"
echo "--------------------"
echo ""

# Test if services are running
if command -v curl &> /dev/null; then
    # Test health endpoint (if bridge is running)
    if curl -s http://localhost:8787/health > /dev/null 2>&1; then
        echo "Testing bridge health endpoint..."
        curl -s http://localhost:8787/health | grep -q '"status":"ok"'
        test_case "Bridge health check" $?

        # Test metrics endpoint
        curl -s http://localhost:8787/metrics | grep -q '"metrics"'
        test_case "Bridge metrics endpoint" $?
    else
        echo -e "${YELLOW}⚠️  Bridge not running, start with: cd cloudflare-bridge && wrangler dev${NC}"
    fi
else
    echo -e "${YELLOW}⚠️  curl not installed, skipping integration tests${NC}"
fi

echo ""

# ============================================
# SUMMARY
# ============================================

echo "📊 Test Summary"
echo "---------------"
echo ""
echo -e "Passed: ${GREEN}$PASS${NC}"
echo -e "Failed: ${RED}$FAIL${NC}"
echo ""

if [ $FAIL -eq 0 ]; then
    echo -e "${GREEN}✅ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}❌ Some tests failed${NC}"
    exit 1
fi
