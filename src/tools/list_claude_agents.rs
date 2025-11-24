use crate::manager::AgentManager;
use kodegen_mcp_schema::claude_agent::{ListClaudeAgentsArgs, ListClaudeAgentsPromptArgs};
use kodegen_mcp_tool::{Tool, ToolExecutionContext};
use rmcp::model::{Content, PromptMessage, PromptMessageContent, PromptMessageRole};
use std::sync::Arc;

// ============================================================================
// ARGS STRUCTS - Imported from kodegen_mcp_schema::claude_agent
// ============================================================================

// ============================================================================
// TOOL STRUCT
// ============================================================================

/// MCP tool for listing all active and completed Claude agent sessions
#[derive(Clone)]
pub struct ListClaudeAgentsTool {
    agent_manager: Arc<AgentManager>,
}

impl ListClaudeAgentsTool {
    /// Create a new list agents tool with required dependencies
    #[must_use]
    pub fn new(agent_manager: Arc<AgentManager>) -> Self {
        Self { agent_manager }
    }
}

// ============================================================================
// TOOL TRAIT IMPLEMENTATION
// ============================================================================

impl Tool for ListClaudeAgentsTool {
    type Args = ListClaudeAgentsArgs;
    type PromptArgs = ListClaudeAgentsPromptArgs;

    fn name() -> &'static str {
        kodegen_mcp_schema::claude_agent::CLAUDE_LIST_AGENTS
    }

    fn description() -> &'static str {
        "List all active and completed agent sessions with status and output preview. \
         Shows working indicator (true if actively processing), turn count, runtime, \
         message count, and last N lines of output for quick overview. \
         Agents are sorted with working agents first, then by most recent activity."
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
            .list_sessions(args.include_completed, args.last_output_lines)
            .await
            .map_err(|e| kodegen_mcp_tool::error::McpError::Other(e.into()))?;

        let mut contents = Vec::new();

        // Human summary
        let summary = if response.agents.is_empty() {
            "🤖 No active agents\n\n\
             Spawn an agent with claude_spawn_agent to start a task".to_string()
        } else {
            let working_agents: Vec<_> = response.agents.iter()
                .filter(|a| a.working && !a.is_complete)
                .collect();
            
            let idle_agents: Vec<_> = response.agents.iter()
                .filter(|a| !a.working && !a.is_complete)
                .collect();
            
            let completed_agents: Vec<_> = response.agents.iter()
                .filter(|a| a.is_complete)
                .collect();

            let mut sections = Vec::new();
            
            if !working_agents.is_empty() {
                let list = working_agents.iter()
                    .map(|a| format!(
                        "  • {} [{}] - Turn {}/{}, {:.1}s, {} messages",
                        a.session_id, a.label, a.turn_count, a.max_turns,
                        a.runtime_ms as f64 / 1000.0, a.message_count
                    ))
                    .collect::<Vec<_>>()
                    .join("\n");
                sections.push(format!("Working:\n{}", list));
            }
            
            if !idle_agents.is_empty() {
                let list = idle_agents.iter()
                    .map(|a| format!(
                        "  • {} [{}] - Turn {}/{}, {:.1}s, {} messages",
                        a.session_id, a.label, a.turn_count, a.max_turns,
                        a.runtime_ms as f64 / 1000.0, a.message_count
                    ))
                    .collect::<Vec<_>>()
                    .join("\n");
                sections.push(format!("Idle:\n{}", list));
            }
            
            if !completed_agents.is_empty() {
                let list = completed_agents.iter()
                    .map(|a| format!(
                        "  • {} [{}] - Turn {}/{}, {:.1}s, {} messages ✓",
                        a.session_id, a.label, a.turn_count, a.max_turns,
                        a.runtime_ms as f64 / 1000.0, a.message_count
                    ))
                    .collect::<Vec<_>>()
                    .join("\n");
                sections.push(format!("Completed:\n{}", list));
            }
            
            format!(
                "🤖 Active agents ({} working, {} completed)\n\n{}",
                response.total_active,
                response.total_completed,
                sections.join("\n\n")
            )
        };
        contents.push(Content::text(summary));

        // JSON metadata
        let metadata = serde_json::to_value(&response)
            .map_err(|e| kodegen_mcp_tool::error::McpError::Other(e.into()))?;
        let json_str = serde_json::to_string_pretty(&metadata)
            .unwrap_or_else(|_| "{}".to_string());
        contents.push(Content::text(json_str));

        Ok(contents)
    }

    fn prompt_arguments() -> Vec<rmcp::model::PromptArgument> {
        vec![rmcp::model::PromptArgument {
            name: "use_case".to_string(),
            title: None,
            description: Some(
                "Primary use case for listing agents (e.g., 'monitoring', 'debugging', 'filtering') \
                 to customize examples and focus".to_string(),
            ),
            required: Some(false),
        }]
    }

    async fn prompt(
        &self,
        _args: Self::PromptArgs,
    ) -> Result<Vec<PromptMessage>, kodegen_mcp_tool::error::McpError> {
        Ok(vec![PromptMessage {
            role: PromptMessageRole::User,
            content: PromptMessageContent::Text {
                text: r#"# claude_list_agents

List all active and completed agent sessions with status overview and output preview.

## Example: List all agents
```json
{
  "include_completed": true,
  "last_output_lines": 3
}
```

## Example: List only active agents
```json
{
  "include_completed": false
}
```

## Response
Returns array of agents sorted by:
1. Working agents first (working=true)
2. Then by most recent activity

Each agent includes:
- Session ID and label
- Working status (true = actively processing)
- Turn count and max_turns
- Runtime and message count
- Completion status and time
- Last N lines of output for preview

## Use Cases
- Monitor multiple parallel agents
- Check which agents are still working
- View progress across all tasks
- Identify stuck or completed agents
- Get quick output preview without full read"#
                    .to_string(),
            },
        }])
    }
}
