use crate::manager::AgentManager;
use kodegen_mcp_schema::claude_agent::{ReadClaudeAgentOutputArgs, ReadClaudeAgentOutputPromptArgs};
use kodegen_mcp_tool::{Tool, ToolExecutionContext};
use rmcp::model::{Content, PromptMessage, PromptMessageContent, PromptMessageRole};
use std::sync::Arc;

// ============================================================================
// ARGS STRUCTS - Imported from kodegen_mcp_schema::claude_agent
// ============================================================================

// ============================================================================
// TOOL STRUCT
// ============================================================================

/// MCP tool for reading paginated output from Claude agent sessions
#[derive(Clone)]
pub struct ReadClaudeAgentOutputTool {
    agent_manager: Arc<AgentManager>,
}

impl ReadClaudeAgentOutputTool {
    /// Create a new read output tool with required dependencies
    #[must_use]
    pub fn new(agent_manager: Arc<AgentManager>) -> Self {
        Self { agent_manager }
    }
}

// ============================================================================
// TOOL TRAIT IMPLEMENTATION
// ============================================================================

impl Tool for ReadClaudeAgentOutputTool {
    type Args = ReadClaudeAgentOutputArgs;
    type PromptArgs = ReadClaudeAgentOutputPromptArgs;

    fn name() -> &'static str {
        kodegen_mcp_schema::claude_agent::CLAUDE_READ_AGENT_OUTPUT
    }

    fn description() -> &'static str {
        "Read paginated output from an agent session. Returns messages with working indicator. \
         Use offset/length for pagination (offset=0 for start, negative for tail). \
         Includes working status (true if actively processing, false if idle/complete). \
         Non-destructive read - messages persist in buffer."
    }

    fn read_only() -> bool {
        true
    }

    fn destructive() -> bool {
        false
    }

    fn idempotent() -> bool {
        true
    }

    fn open_world() -> bool {
        false
    }

    async fn execute(&self, args: Self::Args, _ctx: ToolExecutionContext) -> Result<Vec<Content>, kodegen_mcp_tool::error::McpError> {
        let response = self
            .agent_manager
            .get_output(&args.session_id, args.offset, args.length)
            .await
            .map_err(|e| kodegen_mcp_tool::error::McpError::Other(e.into()))?;

        let mut contents = Vec::new();

        // Human summary
        let status_icon = if response.is_complete {
            "✓"
        } else if response.working {
            "⏳"
        } else {
            "⏸️"
        };

        let status_text = if response.is_complete {
            "Agent completed"
        } else if response.working {
            "Agent is working"
        } else {
            "Agent is idle"
        };

        // Extract last assistant message content (if any)
        let last_assistant_msg = response.output.iter()
            .rev()
            .find(|m| m.message_type == "assistant")
            .and_then(|m| {
                // Extract text from content blocks if present
                m.content.get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|block| block.get("text"))
                    .and_then(|t| t.as_str())
            });

        let msg_preview = if let Some(text) = last_assistant_msg {
            // Truncate to first 100 chars
            let truncated = if text.len() > 100 {
                format!("{}...", &text[..100])
            } else {
                text.to_string()
            };
            format!("\n\nLast assistant message:\n{}", truncated)
        } else {
            String::new()
        };

        let summary = format!(
            "{} {}\n\n\
             Session: {}\n\
             Turn: {}/{}\n\
             Messages: {} (showing {})\
             {}{}",
            status_icon,
            status_text,
            response.session_id,
            response.turn_count,
            response.max_turns,
            response.total_messages,
            response.messages_returned,
            if response.is_complete { "\nStatus: Complete" } else { "" },
            msg_preview
        );
        contents.push(Content::text(summary));

        // JSON metadata (preserve exact structure)
        let metadata = serde_json::to_value(&response)
            .map_err(|e| kodegen_mcp_tool::error::McpError::Other(e.into()))?;
        let json_str = serde_json::to_string_pretty(&metadata)
            .unwrap_or_else(|_| "{}".to_string());
        contents.push(Content::text(json_str));

        Ok(contents)
    }

    fn prompt_arguments() -> Vec<rmcp::model::PromptArgument> {
        vec![]
    }

    async fn prompt(
        &self,
        _args: Self::PromptArgs,
    ) -> Result<Vec<PromptMessage>, kodegen_mcp_tool::error::McpError> {
        Ok(vec![PromptMessage {
            role: PromptMessageRole::User,
            content: PromptMessageContent::Text {
                text: r#"# claude_read_agent_output

Read paginated output from an agent session. Returns messages with working indicator (true = actively processing).

## Example: Read latest messages
```json
{
  "session_id": "uuid-abc-123"
}
```

## Example: Read first 100 messages
```json
{
  "session_id": "uuid-abc-123",
  "offset": 0,
  "length": 100
}
```

## Example: Read last 20 messages (tail)
```json
{
  "session_id": "uuid-abc-123",
  "offset": -20
}
```

## Response
Returns messages array with full content, working status, turn count, completion status, and pagination info (has_more).

## Key Fields
- `working`: true if agent actively processing, false if idle/complete
- `is_complete`: true if conversation finished (max_turns or Result message received)
- `has_more`: true if more messages available for pagination"#.to_string(),
            },
        }])
    }
}
