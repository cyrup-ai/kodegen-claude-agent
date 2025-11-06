//! Session information and status queries
//!
//! Provides methods for querying session info and working status.

use crate::error::{ClaudeError, Result};
use crate::types::agent::AgentInfo;

use super::super::helpers::extract_last_output_lines;
use super::core::{AgentManager, WORKING_THRESHOLD_MS};

impl AgentManager {
    /// Get information about a specific agent session
    ///
    /// Returns session details including working status, turn count, runtime, and
    /// a preview of recent output. Checks active sessions first, then completed sessions.
    pub async fn get_session_info(&self, session_id: &str) -> Result<AgentInfo> {
        // Check active sessions first
        let active = self.active_sessions.lock().await;
        if let Some(session) = active.get(session_id) {
            let runtime_ms = session.created_at.elapsed().as_millis() as u64;
            let turn_count = *session.turn_count.lock().await;
            let is_complete = *session.is_complete.lock().await;
            let messages = session.messages.lock().await;
            let message_count = messages.len();
            let last_output = extract_last_output_lines(&messages, 3);

            // Calculate working status
            let working = if is_complete {
                false
            } else {
                let last_msg_time = *session.last_message_at.lock().await;
                let elapsed_ms = last_msg_time.elapsed().as_millis() as u64;
                elapsed_ms < WORKING_THRESHOLD_MS
            };

            return Ok(AgentInfo {
                session_id: session.session_id.clone(),
                label: session.label.clone(),
                working,
                turn_count,
                max_turns: session.max_turns,
                runtime_ms,
                message_count,
                is_complete,
                last_output,
                completion_time: None,
            });
        }
        drop(active);

        // Check completed sessions
        let completed = self.completed_sessions.lock().await;
        if let Some(session) = completed.get(session_id) {
            let last_output = extract_last_output_lines(&session.messages, 3);

            return Ok(AgentInfo {
                session_id: session.session_id.clone(),
                label: session.label.clone(),
                working: false,
                turn_count: session.final_turn_count,
                max_turns: 0,
                runtime_ms: session.runtime_ms,
                message_count: session.messages.len(),
                is_complete: true,
                last_output,
                completion_time: Some(session.completed_at),
            });
        }

        Err(ClaudeError::SessionNotFound(session_id.to_string()))
    }

    /// Check if an agent session is actively working
    ///
    /// Returns true if the agent has received a message within the working threshold
    /// and the session is not complete.
    pub async fn is_working(&self, session_id: &str) -> Result<bool> {
        let active = self.active_sessions.lock().await;
        if let Some(session) = active.get(session_id) {
            if *session.is_complete.lock().await {
                return Ok(false);
            }

            let last_msg_time = *session.last_message_at.lock().await;
            let elapsed_ms = last_msg_time.elapsed().as_millis() as u64;

            Ok(elapsed_ms < WORKING_THRESHOLD_MS)
        } else {
            Ok(false)
        }
    }
}
