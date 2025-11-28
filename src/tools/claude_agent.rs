//! Unified Claude agent tool - Elite Registry Pattern

use crate::manager::SpawnSessionRequest;
use crate::registry::AgentRegistry;
use kodegen_mcp_schema::claude_agent::{ClaudeAgentAction, ClaudeAgentArgs, ClaudeAgentPromptArgs, CLAUDE_AGENT};
use kodegen_mcp_tool::{Tool, ToolExecutionContext, error::McpError};
use rmcp::model::{Content, PromptMessage, PromptMessageContent, PromptMessageRole};
use serde_json::json;
use std::sync::Arc;

/// Unified MCP tool for Claude agent lifecycle management
#[derive(Clone)]
pub struct ClaudeAgentTool {
    registry: Arc<AgentRegistry>,
}

impl ClaudeAgentTool {
    /// Create a new unified claude_agent tool with agent registry
    #[must_use]
    pub fn new(registry: Arc<AgentRegistry>) -> Self {
        Self { registry }
    }
}

impl Tool for ClaudeAgentTool {
    type Args = ClaudeAgentArgs;
    type PromptArgs = ClaudeAgentPromptArgs;

    fn name() -> &'static str {
        CLAUDE_AGENT
    }

    fn description() -> &'static str {
        "Unified Claude agent interface with action-based dispatch (SPAWN/SEND/READ/LIST/KILL). \
         Each connection gets independent agent numbering (agent:0, agent:1, agent:2). \
         Supports timeout with background continuation.\n\n\
         **Actions:**\n\
         • SPAWN: Create new agent session with initial prompt\n\
         • SEND: Send additional prompt to existing agent\n\
         • READ: Read current agent output\n\
         • LIST: List all agents for this connection\n\
         • KILL: Terminate agent and cleanup"
    }

    fn read_only() -> bool {
        false
    }

    fn destructive() -> bool {
        false
    }

    fn idempotent() -> bool {
        false
    }

    fn open_world() -> bool {
        true
    }

    async fn execute(&self, args: Self::Args, ctx: ToolExecutionContext) -> Result<Vec<Content>, McpError> {
        let connection_id = ctx.connection_id().unwrap_or("default");

        let result = match args.action {
            ClaudeAgentAction::List => {
                self.registry.list_all(connection_id).await
                    .map_err(McpError::Other)?
            }
            ClaudeAgentAction::Kill => {
                if let Some(session_id) = self.registry.remove_session(connection_id, args.agent).await {
                    self.registry.manager().terminate_session(&session_id).await
                        .map_err(|e| McpError::Other(e.into()))?;
                    json!({
                        "agent": args.agent,
                        "output": format!("Agent {} terminated", args.agent),
                        "completed": true,
                        "exit_code": 0,
                    })
                } else {
                    return Err(McpError::invalid_arguments(format!("Agent {} not found", args.agent)));
                }
            }
            ClaudeAgentAction::Read => {
                let session_id = self.registry.get_session_id(connection_id, args.agent).await
                    .map_err(McpError::Other)?;
                
                let info = self.registry.manager().get_session_info(&session_id).await
                    .map_err(|e| McpError::Other(e.into()))?;
                
                let output_response = self.registry.manager().get_output(&session_id, 0, 50).await
                    .map_err(|e| McpError::Other(e.into()))?;
                
                json!({
                    "agent": args.agent,
                    "session_id": session_id,
                    "output": output_response.output,
                    "message_count": info.message_count,
                    "working": info.working,
                    "completed": info.is_complete,
                    "exit_code": if info.is_complete { Some(0) } else { None },
                })
            }
            ClaudeAgentAction::Spawn => {
                let prompt = args.prompt.as_ref()
                    .ok_or_else(|| McpError::invalid_arguments("prompt required for SPAWN"))?;

                // Build spawn request
                let request = SpawnSessionRequest {
                    prompt: prompt.clone(),
                    system_prompt: args.system_prompt.clone(),
                    allowed_tools: args.allowed_tools.clone(),
                    disallowed_tools: args.disallowed_tools.clone(),
                    max_turns: args.max_turns.unwrap_or(10),
                    model: args.model.clone(),
                    cwd: args.cwd.clone(),
                    add_dirs: args.add_dirs.clone(),
                    label: format!("agent:{}", args.agent),
                };

                // Spawn the agent
                let session_id = self.registry.manager().spawn_session(request).await
                    .map_err(|e| McpError::Other(e.into()))?;

                // Register in the registry
                self.registry.register_session(connection_id, args.agent, session_id.clone()).await;

                json!({
                    "agent": args.agent,
                    "session_id": session_id,
                    "output": format!("[Agent {} spawned]\nUse action=READ to check progress.", args.agent),
                    "completed": false,
                    "exit_code": null,
                })
            }
            ClaudeAgentAction::Send => {
                let prompt = args.prompt.as_ref()
                    .ok_or_else(|| McpError::invalid_arguments("prompt required for SEND"))?;

                let session_id = self.registry.get_session_id(connection_id, args.agent).await
                    .map_err(McpError::Other)?;

                // Send message to agent
                self.registry.manager().send_message(&session_id, prompt).await
                    .map_err(|e| McpError::Other(e.into()))?;

                json!({
                    "agent": args.agent,
                    "output": format!("[Prompt sent to agent {}]\nUse action=READ to check progress.", args.agent),
                    "completed": false,
                    "exit_code": null,
                })
            }
        };

        Ok(vec![Content::text(serde_json::to_string_pretty(&result)?)])
    }

    fn prompt_arguments() -> Vec<rmcp::model::PromptArgument> {
        vec![
            rmcp::model::PromptArgument {
                name: "focus_area".to_string(),
                title: Some("Action Focus".to_string()),
                description: Some(
                    "Which agent action(s) to focus on: 'spawn', 'send', 'read', 'list', 'kill', or 'all' (default: 'all')".to_string()
                ),
                required: Some(false),
            },
            rmcp::model::PromptArgument {
                name: "detail_level".to_string(),
                title: Some("Detail Level".to_string()),
                description: Some(
                    "Depth of explanation: 'basic' for core usage, 'advanced' for edge cases (default: 'basic')".to_string()
                ),
                required: Some(false),
            },
        ]
    }

    async fn prompt(
        &self,
        args: Self::PromptArgs,
    ) -> Result<Vec<PromptMessage>, McpError> {
        let focus = args.focus_area.to_lowercase();
        let detail = args.detail_level.to_lowercase();
        let is_advanced = detail == "advanced";

        let mut messages = vec![
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::Text {
                    text: if focus == "all" {
                        format!(
                            "How do I use the claude_agent tool with the elite registry pattern?{}",
                            if is_advanced { " Include best practices and edge cases." } else { "" }
                        )
                    } else {
                        format!(
                            "How do I use the claude_agent '{}' action?{}",
                            focus,
                            if is_advanced { " Include best practices and edge cases." } else { "" }
                        )
                    },
                },
            },
        ];

        let mut assistant_response = String::new();

        assistant_response.push_str(
            "The claude_agent tool uses the elite registry pattern with connection isolation and numeric instance IDs:\n\n"
        );

        if focus == "all" || focus == "spawn" {
            assistant_response.push_str(
                "## SPAWN: Create Agent Session\n\
                 Creates a new autonomous agent with an initial prompt.\n\n\
                 Example:\n\
                 ```json\n\
                 {\"action\": \"SPAWN\", \"agent\": 0, \"prompt\": \"Analyze the codebase\", \"max_turns\": 10, \"await_completion_ms\": 300000}\n\
                 ```\n\n\
                 Parameters:\n\
                 • agent: Instance number (0, 1, 2...) - automatically isolated per connection\n\
                 • prompt: Initial task description\n\
                 • await_completion_ms: Timeout in milliseconds (default: 300000, 0=fire-and-forget)\n\
                 • max_turns: Conversation limit (default: 10)\n\
                 • system_prompt: Custom behavior definition\n\
                 • allowed_tools/disallowed_tools: Tool access control\n\n"
            );
        }

        if focus == "all" || focus == "send" {
            assistant_response.push_str(
                "## SEND: Continue Agent Conversation\n\
                 Sends additional prompt to existing agent session.\n\n\
                 Example:\n\
                 ```json\n\
                 {\"action\": \"SEND\", \"agent\": 0, \"prompt\": \"Now fix the bugs\", \"await_completion_ms\": 60000}\n\
                 ```\n\n\
                 Parameters:\n\
                 • agent: Instance number to send to\n\
                 • prompt: Follow-up instruction\n\
                 • await_completion_ms: Timeout (0=fire-and-forget)\n\n"
            );
        }

        if focus == "all" || focus == "read" {
            assistant_response.push_str(
                "## READ: Check Agent Status\n\
                 Reads current agent output and state.\n\n\
                 Example:\n\
                 ```json\n\
                 {\"action\": \"READ\", \"agent\": 0}\n\
                 ```\n\n\
                 Returns:\n\
                 • output: Current agent messages\n\
                 • message_count: Number of messages\n\
                 • working: Is agent currently active\n\
                 • completed: Has session finished\n\n"
            );
        }

        if focus == "all" || focus == "list" {
            assistant_response.push_str(
                "## LIST: Show All Agents\n\
                 Lists all agent sessions for this connection.\n\n\
                 Example:\n\
                 ```json\n\
                 {\"action\": \"LIST\"}\n\
                 ```\n\n\
                 Shows:\n\
                 • agent: Instance number\n\
                 • session_id: Internal UUID\n\
                 • message_count: Total messages\n\
                 • working: Current status\n\n"
            );
        }

        if focus == "all" || focus == "kill" {
            assistant_response.push_str(
                "## KILL: Terminate Agent\n\
                 Gracefully terminates an agent session.\n\n\
                 Example:\n\
                 ```json\n\
                 {\"action\": \"KILL\", \"agent\": 0}\n\
                 ```\n\n\
                 Cleanup:\n\
                 • Terminates subprocess\n\
                 • Releases resources\n\
                 • Removes from registry\n\n"
            );
        }

        assistant_response.push_str(
            "\n## Connection Isolation\n\
             Each MCP connection gets independent agent numbering. agent:0 for connection A is \
             completely separate from agent:0 for connection B.\n\n\
             ## Timeout Behavior\n\
             • await_completion_ms > 0: Wait up to N milliseconds, return current state on timeout, agent continues in background\n\
             • await_completion_ms = 0: Fire-and-forget mode, agent runs in background\n\
             • Use READ action to check progress of backgrounded agents\n"
        );

        messages.push(PromptMessage {
            role: PromptMessageRole::Assistant,
            content: PromptMessageContent::Text {
                text: assistant_response,
            },
        });

        Ok(messages)
    }
}
