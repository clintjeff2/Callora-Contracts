#!/bin/bash
# Check that vault contract WASM binary stays under 64KB limit

set -e

# Build the vault contract in release mode
echo "Building vault contract..."
cargo build --target wasm32-unknown-unknown --release -p callora-vault

# Get the WASM file size
WASM_FILE="target/wasm32-unknown-unknown/release/callora_vault.wasm"
SIZE=$(wc -c < "$WASM_FILE")
SIZE_KB=$((SIZE / 1024))
MAX_SIZE=$((64 * 1024))  # 64KB in bytes

echo "Vault WASM size: $SIZE bytes (${SIZE_KB}KB)"
echo "Maximum allowed: $MAX_SIZE bytes (64KB)"

# Check if size exceeds limit
if [ "$SIZE" -gt "$MAX_SIZE" ]; then
    echo "❌ ERROR: WASM binary exceeds 64KB limit!"
    echo "   Current: ${SIZE_KB}KB"
    echo "   Limit: 64KB"
    exit 1
else
    REMAINING=$((MAX_SIZE - SIZE))
    REMAINING_KB=$((REMAINING / 1024))
    echo "✅ WASM size check passed!"
    echo "   Remaining headroom: ${REMAINING_KB}KB"
    exit 0
fi
