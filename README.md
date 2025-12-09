<div align="center">
  <img src="assets/img/banner.png" alt="Kodegen AI Banner" width="100%" />
</div>

# KODEGEN Claude Agent Tools 

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](LICENSE.md)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)
[![Edition](https://img.shields.io/badge/edition-2024-green.svg)](https://doc.rust-lang.org/edition-guide/)

**Memory-efficient, blazing-fast MCP tools for Claude Code agent delegation.**

Build powerful multi-agent systems by spawning and orchestrating Claude agent sub-sessions. This Rust SDK and MCP server enables sophisticated delegation patterns where Claude instances can spawn, manage, and communicate with other Claude agents.

## Features

- 🚀 **Agent Delegation** - Spawn Claude sub-sessions with full control over configuration
- 💬 **Bidirectional Communication** - Send prompts and read responses from agent sessions
- 🔄 **Session Management** - Track active/completed sessions with automatic cleanup
- 📊 **Circular Message Buffering** - Efficient memory usage with 1000-message circular buffers
- 🎯 **Type-Safe API** - Strongly-typed Rust SDK with zero-cost abstractions
- ⚡ **Lock-Free Concurrency** - Non-blocking reader/writer architecture
- 🔐 **Security First** - Multiple layers of protection (env filtering, timeouts, buffer limits)
- 🪝 **Hook System** - Intercept and modify tool execution
- 🛡️ **Permission Control** - Fine-grained control over tool access

## Architecture 

This project provides both:
1. **Rust SDK** - Direct programmatic access to Claude agent functionality
2. **MCP Server** - HTTP/HTTPS server exposing 5 MCP tools for agent delegation

### MCP Tools

| Tool | Description |
|------|-------------|
| `claude_spawn_agent` | Create a new Claude agent sub-session with configurable options |
| `claude_send_agent_prompt` | Send a prompt to an existing agent session |
| `claude_read_agent_output` | Read messages and responses from an agent session |
| `claude_list_agents` | List all active and completed agent sessions |
| `claude_terminate_agent_session` | Gracefully terminate an agent session |

## Installation

### Prerequisites

- Rust nightly toolchain (specified in `rust-toolchain.toml`)
- Claude Code CLI: `npm install -g @anthropic-ai/claude-code`
- Node.js (for Claude Code CLI)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/cyrup-ai/kodegen-claude-agent.git
cd kodegen-claude-agent

# Build the project
cargo build --release

# Run tests
cargo test

# Build the HTTP server
cargo build --release --bin kodegen-claude-agent
```

## Quick Start

### Using the MCP Tools (via HTTP Server)

The HTTP server is typically managed by the `kodegend` daemon and runs on port 30460:

```bash
# Start the server
cargo run --bin kodegen-claude-agent
```

### Using the Rust SDK

Add to your `Cargo.toml`:

```toml
[dependencies]
kodegen_claude_agent = "0.1"
```

#### Simple Query

```rust
use kodegen_claude_agent::query;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream = query("What is 2 + 2?", None).await?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        match message? {
            kodegen_claude_agent::Message::Assistant { message, .. } => {
                println!("Claude: {:?}", message);
            }
            _ => {}
        }
    }
    Ok(())
}
```

#### Interactive Client

```rust
use kodegen_claude_agent::{ClaudeSDKClient, ClaudeAgentOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = ClaudeAgentOptions::builder()
        .max_turns(10)
        .build();

    let mut client = ClaudeSDKClient::new(options, None).await?;
    client.send_message("Hello, Claude!").await?;

    while let Some(message) = client.next_message().await {
        // Process messages...
    }

    client.close().await?;
    Ok(())
}
```

#### Agent Session Management

```rust
use kodegen_claude_agent::{AgentManager, PromptInput};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = Arc::new(AgentManager::new());

    // Spawn an agent
    let request = SpawnSessionRequest {
        prompt: PromptInput::String("What is the capital of France?".to_string()),
        options: ClaudeAgentOptions::default(),
    };
    let response = manager.spawn_session(request).await?;
    let session_id = &response.session_ids[0];

    // Read output
    let output = manager.get_session_output(session_id, 0, 100).await?;
    println!("Messages: {:?}", output.output);

    // Send follow-up
    manager.send_prompt(
        session_id,
        PromptInput::String("What is its population?".to_string())
    ).await?;

    // List all sessions
    let sessions = manager.list_sessions().await?;
    println!("Active: {}, Completed: {}",
        sessions.total_active, sessions.total_completed);

    // Cleanup
    manager.terminate_session(session_id).await?;
    Ok(())
}
```

## Examples

Run the comprehensive demo:

```bash
cargo run --example claude_agent_demo
```

The demo showcases:
- Spawning Claude agent sub-sessions
- Multi-turn conversations with context persistence
- Multiple concurrent agents
- Session management and cleanup
- Message parsing and display

## Documentation

- **CLAUDE.md** - Technical guidance for AI assistants working on this codebase
- **API Docs** - Run `cargo doc --open` for full API documentation
- **Examples** - See `examples/` directory for working code samples
- **Tests** - See `tests/` directory for integration test patterns

## Development

### Running Tests

```bash
# All tests
cargo test

# Specific test suite
cargo test --test client_tests

# With debug output
RUST_LOG=debug cargo test -- --nocapture
```

### Code Quality

```bash
# Lint with clippy
cargo clippy

# Format code
cargo fmt

# Check formatting
cargo fmt -- --check
```

## Project Structure

```
kodegen-claude-agent/
├── src/
│   ├── client/          # Interactive bidirectional client
│   ├── manager/         # Agent session management
│   ├── tools/           # MCP tool implementations
│   ├── transport/       # Communication layer (subprocess)
│   ├── control/         # Control protocol handler
│   ├── types/           # Type-safe abstractions
│   ├── hooks/           # Hook system
│   ├── permissions/     # Permission control
│   ├── message/         # Message parsing
│   ├── query.rs         # Simple query function
│   ├── error.rs         # Error types
│   ├── lib.rs           # SDK library
│   └── main.rs          # HTTP server binary
├── tests/               # Integration tests
├── examples/            # Usage examples
└── Cargo.toml
```

## Security

This SDK includes multiple security layers:

- **Environment Filtering** - Blocks dangerous variables (`LD_PRELOAD`, `PATH`, `NODE_OPTIONS`)
- **Argument Validation** - CLI flags validated against allowlist
- **Timeout Protection** - 30-second timeouts on all I/O operations
- **Buffer Limits** - Default 1MB max buffer size prevents memory exhaustion
- **Bounds Checking** - Limits on configurable values (e.g., `max_turns` ≤ 1000)

For complete security details, see `SECURITY_FIXES_APPLIED.md` in the repository.

## Performance

- **Zero-copy message streaming** with circular buffers
- **Lock-free concurrency** - No reader/writer contention
- **Efficient memory usage** - Fixed-size buffers per session
- **Async I/O** - Built on tokio for high concurrency

## Requirements

- **Rust Edition**: 2024
- **Toolchain**: nightly (see `rust-toolchain.toml`)
- **Runtime**: tokio async runtime
- **Claude Code**: Must be installed globally via npm

## Contributing

Contributions are welcome! Please ensure:

1. Code passes `cargo clippy`
2. Code is formatted with `cargo fmt`
3. All tests pass with `cargo test`
4. New features include tests and documentation

## License

Dual-licensed under Apache 2.0 OR MIT.

See [LICENSE.md](LICENSE.md) for details.

## Acknowledgments

Part of the [KODEGEN.ᴀɪ](https://kodegen.ai) ecosystem for AI-powered development tools.

**Authors**: David Maple / KODEGEN.ᴀɪ

---

Built with ❤️ using Rust 🦀
