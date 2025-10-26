# LCA (Local Code Agent)

Local AI agent system for task automation using LM Studio or Ollama. Similar to how Claude Code operates but for local execution and written in Rust for cross-OS compatibility.

## Quick Start

### 1. Setup LM Studio

1. Download and install [LM Studio](https://lmstudio.ai)
2. Load a model (recommended: Llama 3, Mistral, or Code Llama)
3. Start the local server (default port: 1234)

### 2. Build

```bash
cargo build --release
```

### 3. Run

```bash
# Interactive mode
./target/release/lca --provider lmstudio interactive

# Execute a task
./target/release/lca --provider lmstudio execute "create a hello world script"

# Allow all operations without prompting (use with caution)
./target/release/lca --provider lmstudio --allow-all interactive
```

## Commands

```bash
# Interactive mode - best for exploration
lca --provider lmstudio interactive

# Execute a specific task
lca --provider lmstudio execute "your task here"

# Use a specific agent
lca --provider lmstudio agent shell "list files"

# Enable verbose logging
lca --provider lmstudio --verbose interactive
```

## Permission System

By default, the agent will prompt you before:
- Writing files (shows content preview)
- Executing shell commands

Options when prompted:
- `y` - Allow this operation
- `n` - Deny this operation
- `a` - Allow ALL operations for this session
- `q` - Quit/cancel task

Use `--allow-all` flag to skip all prompts (automated mode).

## Interactive Mode Features

- Arrow keys to navigate command history
- Ctrl+C or Ctrl+D to exit
- Type `exit` or `quit` to quit
- History saved to `~/.lca/history.txt`

## Using Ollama Instead

```bash
# Install Ollama
curl -fsSL https://ollama.ai/install.sh | sh

# Pull a model
ollama pull llama2

# Run with Ollama (default)
./target/release/lca interactive
```

## Troubleshooting

**"Connection refused"**
- Ensure LM Studio local server is running
- Check that a model is loaded (not just available)
- Default port is 1234
