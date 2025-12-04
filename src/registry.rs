//! Agent session registry with connection isolation

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::manager::AgentManager;
use kodegen_mcp_schema::claude_agent::ClaudeAgentSummary;

// Maps (connection_id, agent_id) to session UUID
type AgentMap = HashMap<(String, u32), String>;

/// Agent registry for connection isolation and numeric ID mapping.
///
/// Provides a thin mapping layer between user-friendly agent IDs (0, 1, 2, ...)
/// and internal session UUIDs. Each MCP connection gets independent agent numbering.
#[derive(Clone)]
pub struct AgentRegistry {
    agents: Arc<Mutex<AgentMap>>,
    manager: Arc<AgentManager>,
}

impl AgentRegistry {
    /// Create a new agent registry with the given AgentManager.
    pub fn new(manager: Arc<AgentManager>) -> Self {
        Self {
            agents: Arc::new(Mutex::new(HashMap::new())),
            manager,
        }
    }

    /// Get session_id or error if not found
    pub async fn get_session_id(&self, connection_id: &str, agent_id: u32) -> Result<String> {
        let key = (connection_id.to_string(), agent_id);
        let agents = self.agents.lock().await;
        agents.get(&key)
            .cloned()
            .ok_or_else(|| anyhow!("Agent {} not found. Use SPAWN first.", agent_id))
    }

    /// Register new session
    pub async fn register_session(&self, connection_id: &str, agent_id: u32, session_id: String) {
        let key = (connection_id.to_string(), agent_id);
        self.agents.lock().await.insert(key, session_id);
    }

    /// Remove session
    pub async fn remove_session(&self, connection_id: &str, agent_id: u32) -> Option<String> {
        let key = (connection_id.to_string(), agent_id);
        self.agents.lock().await.remove(&key)
    }

    /// List all agents for connection
    pub async fn list_all(&self, connection_id: &str) -> Result<Vec<ClaudeAgentSummary>> {
        let agents = self.agents.lock().await;
        let mut snapshots = Vec::new();

        for ((conn_id, agent_id), session_id) in agents.iter() {
            if conn_id == connection_id
                && let Ok(info) = self.manager.get_session_info(session_id).await
            {
                snapshots.push(ClaudeAgentSummary {
                    agent: *agent_id,
                    session_id: Some(session_id.clone()),
                    message_count: info.message_count,
                    working: info.working,
                    completed: info.is_complete,
                });
            }
        }

        // Sort by agent number
        snapshots.sort_by_key(|s| s.agent);

        Ok(snapshots)
    }

    /// Get reference to AgentManager
    pub fn manager(&self) -> &Arc<AgentManager> {
        &self.manager
    }

    /// Cleanup all agents for a connection (called on connection drop)
    pub async fn cleanup_connection(&self, connection_id: &str) -> usize {
        let mut agents = self.agents.lock().await;
        let to_remove: Vec<(String, u32)> = agents
            .keys()
            .filter(|(conn_id, _)| conn_id == connection_id)
            .cloned()
            .collect();
        
        let count = to_remove.len();
        for key in to_remove {
            if let Some(session_id) = agents.remove(&key) {
                log::debug!(
                    "Cleaning up agent {} (session {}) for connection {}",
                    key.1,
                    session_id,
                    connection_id
                );
                // Terminate the agent session
                if let Err(e) = self.manager.terminate_session(&session_id).await {
                    log::warn!(
                        "Failed to terminate session {} during connection cleanup: {}",
                        session_id,
                        e
                    );
                }
            }
        }
        count
    }
}
