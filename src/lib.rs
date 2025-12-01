#![recursion_limit = "256"]
#![feature(impl_trait_in_fn_trait_return)]
#![feature(impl_trait_in_assoc_type)]
#![feature(negative_impls)]
#![feature(auto_traits)]
#![feature(fn_traits)]

//! # Claude Agent SDK for Rust
//!
//! A comprehensive Rust SDK for building AI agents powered by Claude Code. This library
//! provides idiomatic Rust bindings with full support for async/await, strong typing,
//! and zero-cost abstractions.
//!
//! ## Quick Start
//!
//! The simplest way to use this SDK is with the [`query()`] function:
//!
//! ```no_run
//! use kodegen_claude_agent::query;
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let stream = query("What is 2 + 2?", None).await?;
//!     let mut stream = Box::pin(stream);
//!
//!     while let Some(message) = stream.next().await {
//!         match message? {
//!             kodegen_claude_agent::Message::Assistant { message, .. } => {
//!                 log::info!("Claude: {:?}", message);
//!             }
//!             _ => {}
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Core Features
//!
//! ### 1. Simple Queries with [`query()`]
//!
//! For one-shot interactions where you don't need bidirectional communication:
//!
//! ```no_run
//! # use kodegen_claude_agent::{query, ClaudeAgentOptions};
//! # use futures::StreamExt;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let options = ClaudeAgentOptions::builder()
//!     .system_prompt("You are a helpful coding assistant")
//!     .max_turns(5)
//!     .build();
//!
//! let stream = query("Explain async/await in Rust", Some(options)).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### 2. Interactive Client with [`ClaudeSDKClient`]
//!
//! For stateful conversations with bidirectional communication:
//!
//! ```no_run
//! # use kodegen_claude_agent::{ClaudeSDKClient, ClaudeAgentOptions};
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let options = ClaudeAgentOptions::builder()
//!     .max_turns(10)
//!     .build();
//!
//! let mut client = ClaudeSDKClient::new(options, None).await?;
//! client.send_message("Hello, Claude!").await?;
//!
//! while let Some(message) = client.next_message().await {
//!     // Process messages...
//! }
//!
//! client.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### 3. Custom Tools with SDK MCP Server
//!
//! Create in-process tools that Claude can invoke directly:
//!
//! ```ignore
//! # use kodegen_claude_agent::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
//! # use serde_json::json;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let calculator = SdkMcpServer::new("calculator")
//!     .version("1.0.0")
//!     .tool(SdkMcpTool::new(
//!         "add",
//!         "Add two numbers",
//!         json!({"type": "object", "properties": {
//!             "a": {"type": "number"},
//!             "b": {"type": "number"}
//!         }}),
//!         |input| Box::pin(async move {
//!             let sum = input["a"].as_f64().unwrap_or(0.0)
//!                     + input["b"].as_f64().unwrap_or(0.0);
//!             Ok(ToolResult::text(format!("Sum: {}", sum)))
//!         }),
//!     ));
//! # Ok(())
//! # }
//! ```
//!
//! See the [`mcp`] module for more details.
//!
//! ### 4. Hooks for Custom Behavior
//!
//! Intercept and modify tool execution:
//!
//! ```no_run
//! # use kodegen_claude_agent::{ClaudeAgentOptions, HookManager, HookEvent, HookOutput};
//! # use kodegen_claude_agent::hooks::HookMatcherBuilder;
//! # use std::collections::HashMap;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let hook = HookManager::callback(|event_data, tool_name, _context| async move {
//!     log::info!("Tool used: {:?}", tool_name);
//!     Ok(HookOutput::default())
//! });
//!
//! let matcher = HookMatcherBuilder::new(Some("*"))
//!     .add_hook(hook)
//!     .build();
//!
//! let mut hooks = HashMap::new();
//! hooks.insert(HookEvent::PreToolUse, vec![matcher]);
//!
//! let options = ClaudeAgentOptions::builder()
//!     .hooks(hooks)
//!     .build();
//! # Ok(())
//! # }
//! ```
//!
//! See the [`hooks`] module for more details.
//!
//! ### 5. Permission Control
//!
//! Control which tools Claude can use and how:
//!
//! ```no_run
//! # use kodegen_claude_agent::{ClaudeAgentOptions, PermissionManager};
//! # use kodegen_claude_agent::types::{PermissionResult, PermissionResultAllow, PermissionResultDeny};
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let permission_callback = PermissionManager::callback(
//!     |tool_name, _tool_input, _context| async move {
//!         match tool_name.as_str() {
//!             "Read" | "Glob" => Ok(PermissionResult::Allow(PermissionResultAllow {
//!                 updated_input: None,
//!                 updated_permissions: None,
//!             })),
//!             _ => Ok(PermissionResult::Deny(PermissionResultDeny {
//!                 message: "Tool not allowed".to_string(),
//!                 interrupt: false,
//!             }))
//!         }
//!     }
//! );
//!
//! let options = ClaudeAgentOptions::builder()
//!     .can_use_tool(permission_callback)
//!     .build();
//! # Ok(())
//! # }
//! ```
//!
//! See the [`permissions`] module for more details.
//!
//! ## Architecture
//!
//! The SDK is organized into several key modules:
//!
//! - [`types`]: Core type definitions, newtypes, and builders
//! - [`query()`]: Simple one-shot query function
//! - [`client`]: Interactive bidirectional client
//! - [`mcp`]: SDK MCP server for custom tools
//! - [`hooks`]: Hook system for intercepting events
//! - [`permissions`]: Permission control for tool usage
//! - [`transport`]: Communication layer with Claude Code CLI
//! - [`control`]: Control protocol handler
//! - [`message`]: Message parsing and types
//! - [`error`]: Error types and handling
//!
//! ## Feature Flags
//!
//! This crate supports the following feature flags:
//!
//! - `http` - Enables HTTP transport support (requires `reqwest`)
//! - `tracing-support` - Enables structured logging with `tracing`
//!
//! ## Examples
//!
//! The SDK comes with comprehensive examples:
//!
//! - `simple_query.rs` - Basic query usage
//! - `interactive_client.rs` - Interactive conversation
//! - `bidirectional_demo.rs` - Concurrent operations
//! - `hooks_demo.rs` - Hook system with 3 examples
//! - `permissions_demo.rs` - Permission control with 3 examples
//! - `mcp_demo.rs` - Custom tools with MCP server
//!
//! Run examples with:
//! ```bash
//! cargo run --example simple_query
//! ```
//!
//! ## Requirements
//!
//! - Rust 1.75.0 or later
//! - Node.js (for Claude Code CLI)
//! - Claude Code: `npm install -g @anthropic-ai/claude-code`
//!
//! ## Error Handling
//!
//! All fallible operations return [`Result<T, ClaudeError>`](Result). The SDK uses
//! `thiserror` for ergonomic error types with full context:
//!
//! ```no_run
//! # use kodegen_claude_agent::{query, ClaudeError};
//! # async fn example() {
//! match query("Hello", None).await {
//!     Ok(stream) => { /* ... */ }
//!     Err(ClaudeError::CliNotFound(msg)) => {
//!         log::error!("Claude Code not installed: {}", msg);
//!     }
//!     Err(e) => {
//!         log::error!("Error: {}", e);
//!     }
//! }
//! # }
//! ```
//!
//! ## Safety and Best Practices
//!
//! - **No unsafe code** - The SDK is 100% safe Rust
//! - **Type safety** - Newtypes prevent mixing incompatible values
//! - **Async/await** - Built on tokio for efficient concurrency
//! - **Resource management** - Proper cleanup via RAII and Drop
//! - **Error handling** - Comprehensive error types with context
//!
//! ## Security
//!
//! This SDK includes multiple layers of security protection:
//!
//! - **Environment variable filtering** - Dangerous variables like `LD_PRELOAD`, `PATH`, `NODE_OPTIONS` are blocked
//! - **Argument validation** - CLI flags are validated against an allowlist
//! - **Timeout protection** - All I/O operations have 30-second timeouts
//! - **Buffer limits** - Configurable max buffer size (default 1MB) prevents memory exhaustion
//! - **Bounds checking** - Limits on configurable values (e.g., `max_turns` ≤ 1000)
//! - **Secure logging** - Sensitive data only logged in debug builds with proper feature flags
//!
//! For complete security details, see `SECURITY_FIXES_APPLIED.md` in the repository.
//!
//! ## Version History
//!
//! - **0.1.0** (Current) - Initial release with full feature parity
//!   - ✅ `query()` function for simple queries
//!   - ✅ `ClaudeSDKClient` for bidirectional communication
//!   - ✅ SDK MCP server for custom tools
//!   - ✅ Hook system for event interception
//!   - ✅ Permission control for tool usage
//!   - ✅ Comprehensive test suite (55+ tests)
//!   - ✅ Full documentation and examples

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod client;
pub mod control;
pub mod error;
pub mod hooks;
pub mod manager;
pub mod message;
pub mod permissions;
pub mod query;
pub mod registry;
pub mod transport;
pub mod types;

// Re-export commonly used types for external API
pub use client::ClaudeSDKClient;
pub use error::{ClaudeError, Result};
pub use hooks::{HookManager, HookMatcherBuilder};
pub use message::parse_message;
pub use permissions::{PermissionManager, PermissionManagerBuilder};
pub use query::query;
pub use transport::{PromptInput as TransportPromptInput, SubprocessTransport, Transport};

// Re-export type submodules for flat public API
pub use types::agent::{AgentDefinition, SystemPrompt, SystemPromptPreset};
pub use types::hooks::{
    HookCallback, HookContext, HookDecision, HookEvent, HookMatcher, HookOutput,
};
pub use types::identifiers::{RequestId, SessionId, ToolName};
pub use types::mcp::{
    McpHttpServerConfig, McpServerConfig, McpServers, McpStreamableHttpConfig, McpStdioServerConfig,
};
pub use types::messages::{ContentBlock, ContentValue, Message, UserContent};
pub use types::options::{ClaudeAgentOptions, ClaudeAgentOptionsBuilder};
pub use types::permissions::{
    CanUseToolCallback, PermissionBehavior, PermissionMode, PermissionRequest, PermissionResult,
    PermissionResultAllow, PermissionResultDeny, PermissionRuleValue, PermissionUpdate,
    PermissionUpdateDestination, SettingSource, ToolPermissionContext,
};

/// Version of the SDK
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Agent Tool trait implementations
///
/// Provides MCP tools for spawning, managing, and interacting with Claude agent sessions.
pub mod tools;
pub use tools::ClaudeAgentTool;

// Agent session management
pub use manager::AgentManager;
pub use registry::AgentRegistry;

// Prompt input types
pub use types::{PromptInput, PromptTemplateInput};

// ============================================================================
// EMBEDDED SERVER FUNCTION
// ============================================================================

use std::pin::Pin;
use std::future::Future;
use std::sync::Arc;

// Wrapper to implement ShutdownHook for AgentManager
struct AgentManagerWrapper(Arc<crate::AgentManager>);

impl kodegen_server_http::ShutdownHook for AgentManagerWrapper {
    fn shutdown(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let manager = self.0.clone();
        Box::pin(async move {
            manager.shutdown().await.map_err(|e| anyhow::anyhow!("{}", e))
        })
    }
}

/// Start the claude-agent HTTP server programmatically for embedded mode
pub async fn start_server(
    addr: std::net::SocketAddr,
    tls_cert: Option<std::path::PathBuf>,
    tls_key: Option<std::path::PathBuf>,
) -> anyhow::Result<kodegen_server_http::ServerHandle> {
    use kodegen_server_http::{Managers, RouterSet, register_tool};
    use kodegen_config_manager::ConfigManager;
    use rmcp::handler::server::router::{prompt::PromptRouter, tool::ToolRouter};

    let _ = env_logger::try_init();

    if rustls::crypto::ring::default_provider().install_default().is_err() {
        log::debug!("rustls crypto provider already installed");
    }

    let config = ConfigManager::new();
    config.init().await?;

    let timestamp = chrono::Utc::now();
    let pid = std::process::id();
    let instance_id = format!("{}-{}", timestamp.format("%Y%m%d-%H%M%S-%9f"), pid);
    let usage_tracker = kodegen_utils::usage_tracker::UsageTracker::new(
        format!("claude-agent-{}", instance_id)
    );

    kodegen_mcp_tool::tool_history::init_global_history(instance_id.clone()).await;

    let mut tool_router = ToolRouter::new();
    let mut prompt_router = PromptRouter::new();
    let managers = Managers::new();

    // Initialize agent manager
    let agent_manager = Arc::new(crate::AgentManager::new());
    managers.register(AgentManagerWrapper(agent_manager.clone())).await;

    // Initialize agent registry
    let agent_registry = Arc::new(crate::AgentRegistry::new(agent_manager.clone()));

    // Register unified Claude agent tool
    (tool_router, prompt_router) = register_tool(
        tool_router,
        prompt_router,
        crate::ClaudeAgentTool::new(agent_registry.clone()),
    );

    // Create connection cleanup callback for agent registry
    // This ensures all agent sessions are properly terminated when a connection drops
    let registry_clone = agent_registry.clone();
    let connection_cleanup: kodegen_server_http::ConnectionCleanupFn = Arc::new(
        move |connection_id: String| {
            let registry = registry_clone.clone();
            Box::pin(async move {
                let count = registry.cleanup_connection(&connection_id).await;
                log::info!(
                    "Connection {} dropped: cleaned up {} agent session(s)",
                    connection_id,
                    count
                );
            })
        }
    );

    let router_set = RouterSet::new(tool_router, prompt_router, managers);

    // Create session manager
    let session_config = rmcp::transport::streamable_http_server::session::local::SessionConfig {
        channel_capacity: 16,
        keep_alive: Some(std::time::Duration::from_secs(3600)),
    };
    let session_manager = Arc::new(
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager {
            sessions: Default::default(),
            session_config,
        }
    );

    // Create HTTP server
    let server = kodegen_server_http::HttpServer::new(
        router_set.tool_router,
        router_set.prompt_router,
        usage_tracker,
        config,
        router_set.managers,
        session_manager,
        Some(connection_cleanup),
    );

    // Start server with TLS
    let tls_config = tls_cert.zip(tls_key);
    let shutdown_timeout = std::time::Duration::from_secs(30);
    let handle = server.serve_with_tls(addr, tls_config, shutdown_timeout).await?;

    // Return handle for kodegend to control shutdown
    Ok(handle)
}

/// Start claude-agent HTTP server using pre-bound listener (TOCTOU-safe)
///
/// This variant is used by kodegend to eliminate TOCTOU race conditions
/// during port cleanup. The listener is already bound to a port.
///
/// # Arguments
/// * `listener` - Pre-bound TcpListener (port already reserved)
/// * `tls_config` - Optional (cert_path, key_path) for HTTPS
///
/// # Returns
/// ServerHandle for graceful shutdown, or error if startup fails
pub async fn start_server_with_listener(
    listener: tokio::net::TcpListener,
    tls_config: Option<(std::path::PathBuf, std::path::PathBuf)>,
) -> anyhow::Result<kodegen_server_http::ServerHandle> {
    use kodegen_server_http::{Managers, RouterSet, register_tool};
    use kodegen_config_manager::ConfigManager;
    use rmcp::handler::server::router::{prompt::PromptRouter, tool::ToolRouter};

    let _ = env_logger::try_init();

    if rustls::crypto::ring::default_provider().install_default().is_err() {
        log::debug!("rustls crypto provider already installed");
    }

    let config = ConfigManager::new();
    config.init().await?;

    let timestamp = chrono::Utc::now();
    let pid = std::process::id();
    let instance_id = format!("{}-{}", timestamp.format("%Y%m%d-%H%M%S-%9f"), pid);
    let usage_tracker = kodegen_utils::usage_tracker::UsageTracker::new(
        format!("claude-agent-{}", instance_id)
    );

    kodegen_mcp_tool::tool_history::init_global_history(instance_id.clone()).await;

    let mut tool_router = ToolRouter::new();
    let mut prompt_router = PromptRouter::new();
    let managers = Managers::new();

    // Initialize agent manager
    let agent_manager = Arc::new(crate::AgentManager::new());
    managers.register(AgentManagerWrapper(agent_manager.clone())).await;

    // Initialize agent registry
    let agent_registry = Arc::new(crate::AgentRegistry::new(agent_manager.clone()));

    // Register unified Claude agent tool
    (tool_router, prompt_router) = register_tool(
        tool_router,
        prompt_router,
        crate::ClaudeAgentTool::new(agent_registry.clone()),
    );

    // Create connection cleanup callback for agent registry
    let registry_clone = agent_registry.clone();
    let connection_cleanup: kodegen_server_http::ConnectionCleanupFn = Arc::new(
        move |connection_id: String| {
            let registry = registry_clone.clone();
            Box::pin(async move {
                let count = registry.cleanup_connection(&connection_id).await;
                log::info!(
                    "Connection {} dropped: cleaned up {} agent session(s)",
                    connection_id,
                    count
                );
            })
        }
    );

    let router_set = RouterSet::new(tool_router, prompt_router, managers);

    // Create session manager
    let session_config = rmcp::transport::streamable_http_server::session::local::SessionConfig {
        channel_capacity: 16,
        keep_alive: Some(std::time::Duration::from_secs(3600)),
    };
    let session_manager = Arc::new(
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager {
            sessions: Default::default(),
            session_config,
        }
    );

    // Create HTTP server
    let server = kodegen_server_http::HttpServer::new(
        router_set.tool_router,
        router_set.prompt_router,
        usage_tracker,
        config,
        router_set.managers,
        session_manager,
        Some(connection_cleanup),
    );

    // Start server with pre-bound listener
    let shutdown_timeout = std::time::Duration::from_secs(30);
    let handle = server.serve_with_listener(listener, tls_config, shutdown_timeout).await?;

    // Return handle for kodegend to control shutdown
    Ok(handle)
}
