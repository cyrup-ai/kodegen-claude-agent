//! Agent session spawning logic
//!
//! Handles creation of new agent sessions with background message collection.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

use crate::client::ClaudeSDKClient;
use crate::error::Result;
use crate::types::agent::SystemPrompt;
use crate::types::identifiers::ToolName;
use crate::types::options::ClaudeAgentOptions;

use super::super::background::{CollectorContext, spawn_message_collector};
use super::super::session::AgentSessionInfo;
use super::core::AgentManager;

// ============================================================================
// REQUEST TYPES
// ============================================================================

/// Request parameters for spawning a new agent session
#[derive(Debug, Clone)]
pub struct SpawnSessionRequest {
    /// Initial prompt to send to the agent
    pub prompt: String,
    /// System prompt configuration
    pub system_prompt: Option<String>,
    /// List of tools that Claude is allowed to use
    pub allowed_tools: Vec<String>,
    /// List of tools that Claude is not allowed to use
    pub disallowed_tools: Vec<String>,
    /// Maximum number of turns before stopping
    pub max_turns: u32,
    /// AI model to use
    pub model: Option<String>,
    /// Working directory for the CLI process
    pub cwd: Option<String>,
    /// Additional directories to add to the context
    pub add_dirs: Vec<String>,
    /// Label for identifying the session
    pub label: String,
}

// ============================================================================
// SPAWN IMPLEMENTATION
// ============================================================================

impl AgentManager {
    /// Spawn a new Claude agent session
    ///
    /// Creates a new `ClaudeSDKClient` with the specified options, sends the initial prompt,
    /// and spawns a background task to collect messages into a circular buffer.
    ///
    /// Returns the session ID for subsequent operations.
    pub async fn spawn_session(&self, request: SpawnSessionRequest) -> Result<String> {
        // Generate unique session ID
        let session_id = Uuid::new_v4().to_string();

        // Build ClaudeAgentOptions
        let options = ClaudeAgentOptions {
            allowed_tools: request
                .allowed_tools
                .into_iter()
                .map(ToolName::from)
                .collect(),
            disallowed_tools: request
                .disallowed_tools
                .into_iter()
                .map(ToolName::from)
                .collect(),
            system_prompt: request.system_prompt.map(SystemPrompt::from),
            max_turns: Some(request.max_turns),
            model: request.model,
            cwd: request.cwd.map(PathBuf::from),
            add_dirs: request.add_dirs.into_iter().map(PathBuf::from).collect(),
            ..Default::default()
        };

        // Create client
        let mut client = ClaudeSDKClient::new(options, None).await?;

        // Send initial prompt
        client.send_message(&request.prompt).await?;

        // Create command channel
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        // Create shared state for background task
        let messages_arc = Arc::new(Mutex::new(VecDeque::with_capacity(1000)));
        let last_message_arc = Arc::new(Mutex::new(Instant::now()));
        let turn_count_arc = Arc::new(Mutex::new(0));
        let is_complete_arc = Arc::new(Mutex::new(false));

        // Create session info
        let session_info = AgentSessionInfo {
            session_id: session_id.clone(),
            label: request.label,
            command_tx: command_tx.clone(),
            messages: Arc::clone(&messages_arc),
            created_at: Instant::now(),
            last_message_at: Arc::clone(&last_message_arc),
            turn_count: Arc::clone(&turn_count_arc),
            max_turns: request.max_turns,
            is_complete: Arc::clone(&is_complete_arc),
        };

        // Store in active sessions
        self.active_sessions
            .lock()
            .await
            .insert(session_id.clone(), session_info);

        // Spawn background message collector
        let ctx = CollectorContext {
            messages: messages_arc,
            last_message: last_message_arc,
            turn_count: turn_count_arc,
            is_complete: is_complete_arc,
            max_turns: request.max_turns,
            session_id: session_id.clone(),
        };
        spawn_message_collector(client, command_rx, ctx);

        Ok(session_id)
    }
}
