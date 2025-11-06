//! Message pagination utilities
//!
//! Helper functions for paginating message collections.

use std::collections::VecDeque;

use crate::types::agent::SerializedMessage;

/// Paginate messages based on offset and length
///
/// # Arguments
/// * `messages` - Message collection to paginate
/// * `offset` - Starting position (>=0) or tail count (<0)
/// * `length` - Maximum number of messages to return
///
/// # Pagination Modes
/// - offset >= 0: Start from position N, take `length` messages
/// - offset < 0: Tail mode - take last |offset| messages
pub(crate) fn paginate_messages(
    messages: &VecDeque<SerializedMessage>,
    offset: i64,
    length: usize,
) -> Vec<SerializedMessage> {
    if offset >= 0 {
        let start = offset as usize;
        messages.iter().skip(start).take(length).cloned().collect()
    } else {
        let tail_count = (-offset) as usize;
        messages
            .iter()
            .rev()
            .take(tail_count)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

/// Calculate if there are more messages available for pagination
///
/// # Arguments
/// * `offset` - The offset used in the query
/// * `messages_returned` - Number of messages returned in this page
/// * `total_messages` - Total number of messages available
///
/// # Returns
/// `true` if more messages are available beyond the current page
pub(crate) fn calculate_has_more(
    offset: i64,
    messages_returned: usize,
    total_messages: usize,
) -> bool {
    if offset >= 0 {
        (offset as usize + messages_returned) < total_messages
    } else {
        false
    }
}
