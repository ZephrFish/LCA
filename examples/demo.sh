#!/bin/bash

# Demo script for LCA System
# This demonstrates various capabilities of the agent system

AGENT="./target/release/lca"
PROVIDER="${1:-ollama}"

echo "=========================================="
echo "LCA System Demo"
echo "Provider: $PROVIDER"
echo "=========================================="
echo ""

# Check if binary exists
if [ ! -f "$AGENT" ]; then
    echo "Building agent system..."
    cargo build --release
    echo ""
fi

echo "1. File Operations Demo"
echo "----------------------------------------"
echo "Task: List all Rust source files"
$AGENT --provider $PROVIDER agent file "list all files in src directory"
echo ""

echo "2. Code Generation Demo"
echo "----------------------------------------"
echo "Task: Generate a simple Rust function"
$AGENT --provider $PROVIDER agent code "write a function that checks if a number is prime"
echo ""

echo "3. Shell Execution Demo"
echo "----------------------------------------"
echo "Task: Check project structure"
$AGENT --provider $PROVIDER agent shell "show directory structure with tree or ls"
echo ""

echo "4. Analysis Demo"
echo "----------------------------------------"
echo "Task: Analyze the project"
$AGENT --provider $PROVIDER agent analysis "give me an overview of this project"
echo ""

echo "5. Multi-Agent Coordination Demo"
echo "----------------------------------------"
echo "Task: Complex task requiring multiple agents"
$AGENT --provider $PROVIDER execute "analyze the main.rs file and suggest improvements"
echo ""

echo "=========================================="
echo "Demo Complete!"
echo "=========================================="
echo ""
echo "Try interactive mode with:"
echo "$AGENT --provider $PROVIDER interactive"
