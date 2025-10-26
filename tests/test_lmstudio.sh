#!/bin/bash

set -e

echo "LCA - Test Script"
echo "=============================="
echo ""

BINARY="./target/release/lca"

if [ ! -f "$BINARY" ]; then
    echo "Error: Binary not found at $BINARY"
    echo "Please run: cargo build --release"
    exit 1
fi

echo "1. Testing binary execution..."
$BINARY --help > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "   ✓ Binary executes successfully"
else
    echo "   ✗ Binary execution failed"
    exit 1
fi

echo ""
echo "2. Testing help commands..."
$BINARY execute --help > /dev/null 2>&1
$BINARY agent --help > /dev/null 2>&1
$BINARY init --help > /dev/null 2>&1
echo "   ✓ All help commands work"

echo ""
echo "3. Checking LLM provider options..."
echo "   Available providers:"
echo "   - ollama (default, port 11434)"
echo "   - lmstudio (port 1234)"

echo ""
echo "4. Testing with LM Studio (requires LM Studio running)..."
echo "   To test with LM Studio:"
echo "   1. Start LM Studio"
echo "   2. Load a model"
echo "   3. Enable local server (port 1234)"
echo "   4. Run: $BINARY --provider lmstudio execute 'say hello'"

echo ""
echo "5. Testing with Ollama (requires Ollama running)..."
echo "   To test with Ollama:"
echo "   1. Install Ollama: curl -fsSL https://ollama.ai/install.sh | sh"
echo "   2. Pull a model: ollama pull llama2"
echo "   3. Run: $BINARY execute 'say hello'"

echo ""
echo "=============================="
echo "Basic tests completed successfully!"
echo "For live testing, ensure either LM Studio or Ollama is running."
