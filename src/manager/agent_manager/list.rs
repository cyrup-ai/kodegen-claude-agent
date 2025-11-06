//! Session listing functionality
//!
//! Provides methods for listing all active and completed sessions.

use crate::error::Result;
use crate::types::agent::{AgentInfo, ListSessionsResponse};

use super::super::helpers::extract_last_output_lines;
use super::core::{AgentManager, WORKING_THRESHOLD_MS};

impl AgentManager {
    /// List all agent sessions
    ///
    /// Returns information about active and (optionally) completed sessions, sorted by
    /// working status (working first) and then by runtime (most recent first).
    pub async fn list_sessions(
        &self,
        include_completed: bool,
        last_output_lines: usize,
    ) -> Result<ListSessionsResponse> {
        let mut agents = Vec::new();

        // Collect active sessions
        let active = self.active_sessions.lock().await;
        for (_, session) in active.iter() {
            let runtime_ms = session.created_at.elapsed().as_millis() as u64;
            let turn_count = *session.turn_count.lock().await;
            let is_complete = *session.is_complete.lock().await;
            let messages = session.messages.lock().await;
            let message_count = messages.len();
            let last_output = extract_last_output_lines(&messages, last_output_lines);

            let working = if is_complete {
                false
            } else {
                let last_msg_time = *session.last_message_at.lock().await;
                let elapsed_ms = last_msg_time.elapsed().as_millis() as u64;
                elapsed_ms < WORKING_THRESHOLD_MS
            };

            agents.push(AgentInfo {
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

        let total_active = agents.len();
        drop(active);

        // Collect completed sessions if requested
        let mut total_completed = 0;
        if include_completed {
            let completed = self.completed_sessions.lock().await;
            total_completed = completed.len();

            for (_, session) in completed.iter() {
                let last_output = extract_last_output_lines(&session.messages, last_output_lines);

                agents.push(AgentInfo {
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
        }

        agents.sort_by(|a, b| match (a.working, b.working) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.runtime_ms.cmp(&a.runtime_ms),
        });

        Ok(ListSessionsResponse {
            agents,
            total_active,
            total_completed,
        })
    }
}
