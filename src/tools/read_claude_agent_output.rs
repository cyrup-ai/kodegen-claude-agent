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
        vec![rmcp::model::PromptArgument {
            name: "scenario".to_string(),
            title: None,
            description: Some(
                "Optional scenario to focus examples on: 'pagination' (navigating output history), \
                 'monitoring' (tracking progress and status), or 'integration' (full workflow with agent spawning)".to_string(),
            ),
            required: Some(false),
        }]
    }

    async fn prompt(
        &self,
        _args: Self::PromptArgs,
    ) -> Result<Vec<PromptMessage>, kodegen_mcp_tool::error::McpError> {
        Ok(vec![
            // =====================================================================
            // USER QUESTION: How to use read_claude_agent_output
            // =====================================================================
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::Text {
                    text: "How do I use the read_claude_agent_output tool to monitor and interact with agent sessions?".to_string(),
                },
            },
            
            // =====================================================================
            // ASSISTANT RESPONSE: Comprehensive Teaching
            // =====================================================================
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::Text {
                    text: r#"# read_claude_agent_output - Agent Session Output Monitoring

The `read_claude_agent_output` tool enables you to read paginated messages from Claude agent sessions. \
It's essential for monitoring agent progress, debugging execution, and determining when agents have completed their tasks.

## Core Concepts

### Session IDs
Every spawned agent has a unique session ID. You need this ID to read its output:
- Returned by the `claude_agent` tool when you spawn agents
- Format: UUID string (e.g., "550e8400-e29b-41d4-a716-446655440000")

### Pagination Parameters
The tool supports flexible pagination:
- **offset = 0 (default)**: Reads from the beginning up to `length` messages
- **offset = N (positive)**: Reads starting at message N for `length` messages  
- **offset = -N (negative)**: Reads the last N messages (tail), `length` is ignored
- **length = M**: How many messages to read (default: 50)

### Status Indicators
The response includes critical status fields:
- **working**: `true` = agent actively processing, `false` = idle/idle/complete
- **is_complete**: `true` = agent finished (hit max_turns or received Result), conversation closed
- **has_more**: `true` = more messages available for pagination

## Usage Patterns

### Pattern 1: Monitor Latest Output (Most Common)
Check the most recent messages to see what the agent is doing right now:

```json
{
  "session_id": "uuid-of-running-agent"
}
```

Response includes:
- Latest `length` messages (up to 50 by default)
- Current `working` status
- `turn_count` and `max_turns` progress
- `has_more: true` if agent produced more than 50 messages

Use when: Polling for real-time updates on agent progress

### Pattern 2: Read Last N Messages (Tail)
Get only the final messages from a large conversation:

```json
{
  "session_id": "uuid-of-completed-agent",
  "offset": -20
}
```

This reads the last 20 messages regardless of how many total messages exist. \
Efficient for: Reviewing final output without loading entire history

### Pattern 3: Paginate Through Full Output
Read output in chunks (e.g., 100 messages per page):

```json
{
  "session_id": "uuid-of-agent",
  "offset": 0,
  "length": 100
}
```

Then read next page:

```json
{
  "session_id": "uuid-of-agent",
  "offset": 100,
  "length": 100
}
```

Check `has_more` in response to know if more pages exist. \
When: Processing large output archives or complete audit logs

### Pattern 4: Poll Until Completion
Monitor a spawned agent until it finishes:

```rust
loop {
    let result = read_claude_agent_output(session_id).await?;
    
    if result.is_complete {
        // Agent finished, read final output
        let final = read_claude_agent_output_with_tail(&session_id, -50).await?;
        break;
    }
    
    if result.working {
        println!("Agent turn {}/{}", result.turn_count, result.max_turns);
    }
    
    // Wait before next poll
    sleep(Duration::from_secs(1)).await;
}
```

This pattern is essential for: Long-running agents where you need to know when work is done

### Pattern 5: Combine Status Check with Output Read
Get status AND latest output in one call:

```json
{
  "session_id": "uuid-of-agent"
}
```

Response contains:
- `working: true/false` - What's happening right now?
- `is_complete: true/false` - Did it finish?
- Last 50 messages - What did it output?
- `turn_count` / `max_turns` - Progress percentage
- `total_messages` - How many messages total?

This is the most common pattern for: Status monitoring and debugging

## Response Structure

Every response includes:

```json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "output": [
    {
      "message_type": "user",
      "content": { ... },
      "timestamp": "..."
    },
    {
      "message_type": "assistant",
      "content": { ... },
      "timestamp": "..."
    }
  ],
  "working": false,
  "is_complete": true,
  "has_more": false,
  "turn_count": 5,
  "max_turns": 10,
  "total_messages": 15,
  "messages_returned": 15
}
```

Key fields to check:
- `is_complete` first - has agent finished?
- `working` second - is it still processing?
- `has_more` - are there more messages to read?
- `messages_returned` - how many did we get in this request?

## Key Behaviors

1. **Non-destructive Read**: Reading output never modifies or affects the agent session
2. **Large Output Handling**: Use pagination (`offset`/`length`) for agents with 1000+ messages
3. **Negative Offsets**: Only `offset` (positive or negative) and `length` for tails - length is ignored for tails
4. **Real-time Status**: `working` field is live - tells you what the agent is doing NOW
5. **Completion Detection**: `is_complete: true` means the agent conversation is closed

## Common Patterns by Use Case

### Use Case: Spawn Agent and Wait for Completion
```json
{
  "action": "spawn",
  "prompt": "Write a Python script that...",
  "max_turns": 20
}
```
Then periodically:
```json
{
  "session_id": "from-spawn-response"
}
```
Until `is_complete: true`

### Use Case: Debug Failed Agent
```json
{
  "session_id": "uuid-of-failed-agent",
  "offset": -50
}
```
Reads last 50 messages to see where it went wrong

### Use Case: Extract Final Results
```json
{
  "session_id": "uuid-of-completed-agent",
  "offset": -10
}
```
Gets final 10 messages which usually contain the conclusion

## Important Notes

- **Session Must Exist**: Reading a non-existent session_id returns an error
- **Offset Constraints**: offset >= 0 for forward read, negative for tail
- **Default Length**: If you don't specify length, defaults to 50 messages
- **Pagination Info**: Always check `has_more` to know if more pages exist
- **Working vs Complete**: An agent can be `working=false` but `is_complete=false` (idle, not done)
- **Message Ordering**: Messages in response are in chronological order (oldest first)"#.to_string(),
                },
            },
        ])
    }
}
