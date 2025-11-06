//! Session output retrieval with pagination
//!
//! Handles paginated output queries for agent sessions.

use crate::error::{ClaudeError, Result};
use crate::types::agent::GetOutputResponse;

use super::core::{AgentManager, WORKING_THRESHOLD_MS};
use super::pagination::{calculate_has_more, paginate_messages};

impl AgentManager {
    /// Get paginated output from an agent session
    ///
    /// Supports offset/length pagination:
    /// - offset >= 0: Start from line N, take `length` messages
    /// - offset < 0: Tail mode - take last |offset| messages
    ///
    /// Returns messages, working status, and pagination metadata.
    pub async fn get_output(
        &self,
        session_id: &str,
        offset: i64,
        length: usize,
    ) -> Result<GetOutputResponse> {
        // Try active sessions first
        let active = self.active_sessions.lock().await;
        if let Some(session) = active.get(session_id) {
            let messages = session.messages.lock().await;
            let turn_count = *session.turn_count.lock().await;
            let is_complete = *session.is_complete.lock().await;
            let max_turns = session.max_turns;

            // Calculate working status
            let working = if is_complete {
                false
            } else {
                let last_msg_time = *session.last_message_at.lock().await;
                let elapsed_ms = last_msg_time.elapsed().as_millis() as u64;
                elapsed_ms < WORKING_THRESHOLD_MS
            };

            // Handle pagination
            let output = paginate_messages(&messages, offset, length);
            let total_messages = messages.len();
            let messages_returned = output.len();
            let has_more = calculate_has_more(offset, messages_returned, total_messages);

            return Ok(GetOutputResponse {
                session_id: session_id.to_string(),
                working,
                output,
                total_messages,
                messages_returned,
                is_complete,
                turn_count,
                max_turns,
                has_more,
            });
        }
        drop(active);

        // Try completed sessions
        let completed = self.completed_sessions.lock().await;
        if let Some(session) = completed.get(session_id) {
            let output = paginate_messages(&session.messages, offset, length);
            let total_messages = session.messages.len();
            let messages_returned = output.len();
            let has_more = calculate_has_more(offset, messages_returned, total_messages);

            return Ok(GetOutputResponse {
                session_id: session_id.to_string(),
                working: false,
                output,
                total_messages,
                messages_returned,
                is_complete: true,
                turn_count: session.final_turn_count,
                max_turns: 0,
                has_more,
            });
        }

        Err(ClaudeError::SessionNotFound(session_id.to_string()))
    }
}
