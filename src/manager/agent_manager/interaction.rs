//! Session interaction methods
//!
//! Handles sending messages to sessions and terminating sessions.

use chrono::Utc;
use tokio::sync::{oneshot, broadcast};

use crate::error::{ClaudeError, Result};
use crate::types::agent::{TerminateResponse, SerializedMessage};

use super::super::commands::SessionCommand;
use super::super::session::CompletedAgentSession;
use super::core::AgentManager;

impl AgentManager {
    /// Send a follow-up message to an active agent session
    ///
    /// Only works for active, non-completed sessions that haven't reached `max_turns`.
    pub async fn send_message(&self, session_id: &str, prompt: &str) -> Result<()> {
        let active = self.active_sessions.lock().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| ClaudeError::SessionNotFound(session_id.to_string()))?;

        if *session.is_complete.lock().await {
            return Err(ClaudeError::SessionComplete(session_id.to_string()));
        }

        if *session.turn_count.lock().await >= session.max_turns {
            return Err(ClaudeError::SessionComplete(session_id.to_string()));
        }

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = SessionCommand::SendMessage {
            prompt: prompt.to_string(),
            response_tx,
        };

        session
            .command_tx
            .send(cmd)
            .map_err(|_| ClaudeError::SessionComplete(session_id.to_string()))?;

        response_rx
            .await
            .map_err(|_| ClaudeError::SessionComplete(session_id.to_string()))??;

        Ok(())
    }

    /// Terminate an agent session gracefully
    ///
    /// Closes the client connection, moves the session to completed state, and returns
    /// final statistics. The session will be retained for `COMPLETED_RETENTION_MS` before cleanup.
    pub async fn terminate_session(&self, session_id: &str) -> Result<TerminateResponse> {
        let mut active = self.active_sessions.lock().await;
        let session = active
            .remove(session_id)
            .ok_or_else(|| ClaudeError::SessionNotFound(session_id.to_string()))?;
        drop(active);

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = SessionCommand::Shutdown { response_tx };

        if session.command_tx.send(cmd).is_ok() {
            let _ = response_rx.await;
        }

        let runtime_ms = session.created_at.elapsed().as_millis() as u64;
        let messages = session.messages.lock().await;
        let total_messages = messages.len();
        let final_turn_count = *session.turn_count.lock().await;

        let completed = CompletedAgentSession {
            session_id: session.session_id.clone(),
            label: session.label.clone(),
            messages: messages.clone(),
            final_turn_count,
            runtime_ms,
            completed_at: Utc::now(),
        };

        self.completed_sessions
            .lock()
            .await
            .insert(session_id.to_string(), completed);

        Ok(TerminateResponse {
            session_id: session_id.to_string(),
            success: true,
            final_turn_count,
            total_messages,
            runtime_ms,
        })
    }

    /// Subscribe to real-time message events for a session
    ///
    /// Returns a broadcast receiver that will receive all new messages
    /// as they arrive from the agent. Used for event-driven streaming.
    pub async fn subscribe_to_messages(&self, session_id: &str) -> Result<broadcast::Receiver<SerializedMessage>> {
        let active = self.active_sessions.lock().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| ClaudeError::SessionNotFound(session_id.to_string()))?;
        
        Ok(session.message_tx.subscribe())
    }
}
