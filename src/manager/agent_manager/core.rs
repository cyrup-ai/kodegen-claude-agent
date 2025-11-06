//! Core agent manager structure and lifecycle management
//!
//! Provides the main `AgentManager` struct with initialization, cleanup, and shutdown.

use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::error::Result;

use super::super::session::{AgentSessionInfo, CompletedAgentSession};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Time threshold for considering an agent "working" (2 seconds)
pub(crate) const WORKING_THRESHOLD_MS: u64 = 2000;

/// Retention time for completed sessions before cleanup (1 minute)
const COMPLETED_RETENTION_MS: u64 = 60000;

/// Interval for cleanup task execution (1 minute)
const CLEANUP_INTERVAL_SECS: u64 = 60;

// ============================================================================
// AGENT MANAGER CORE
// ============================================================================

/// Manager for multiple concurrent Claude agent sessions
///
/// The `AgentManager` coordinates multiple Claude agent sessions, handling:
/// - Session lifecycle (spawn, monitor, terminate)
/// - Message buffering with circular buffers
/// - Working status detection
/// - Automatic cleanup of completed sessions
pub struct AgentManager {
    pub(crate) active_sessions: Arc<Mutex<HashMap<String, AgentSessionInfo>>>,
    pub(crate) completed_sessions: Arc<Mutex<HashMap<String, CompletedAgentSession>>>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

impl AgentManager {
    /// Create a new `AgentManager` with background cleanup task
    #[must_use]
    pub fn new() -> Self {
        let active: Arc<Mutex<HashMap<String, AgentSessionInfo>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let completed: Arc<Mutex<HashMap<String, CompletedAgentSession>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Spawn cleanup background task
        let completed_clone = Arc::clone(&completed);
        let cleanup_handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(CLEANUP_INTERVAL_SECS)).await;

                let mut sessions = completed_clone.lock().await;
                let now = Utc::now();

                // Remove sessions older than retention period
                sessions.retain(|_id, session| {
                    let age_ms = now
                        .signed_duration_since(session.completed_at)
                        .num_milliseconds() as u64;
                    age_ms < COMPLETED_RETENTION_MS
                });
            }
        });

        Self {
            active_sessions: active,
            completed_sessions: completed,
            cleanup_handle: Some(cleanup_handle),
        }
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AgentManager {
    fn drop(&mut self) {
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
    }
}

impl AgentManager {
    /// Gracefully shutdown the AgentManager
    ///
    /// Terminates all active sessions and cancels the cleanup task.
    /// Should be called before dropping to ensure clean shutdown.
    pub async fn shutdown(&self) -> Result<()> {
        log::info!("Shutting down AgentManager...");

        // Get all active session IDs
        let session_ids: Vec<String> = {
            let sessions = self.active_sessions.lock().await;
            sessions.keys().cloned().collect()
        };

        // Terminate all active sessions
        for session_id in session_ids {
            log::debug!("Terminating session: {}", session_id);
            if let Err(e) = self.terminate_session(&session_id).await {
                log::warn!("Failed to terminate session {}: {}", session_id, e);
            }
        }

        log::info!("AgentManager shutdown complete");
        Ok(())
    }
}

// ShutdownHook implementation for MCP server integration
#[cfg(feature = "server")]
use kodegen_server_http::ShutdownHook;

#[cfg(feature = "server")]
impl ShutdownHook for AgentManager {
    fn shutdown(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(async move {
            AgentManager::shutdown(self).await.map_err(|e| anyhow::anyhow!("{}", e))
        })
    }
}
