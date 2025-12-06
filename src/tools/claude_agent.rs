//! Unified Claude agent tool - Elite Registry Pattern

use crate::manager::SpawnSessionRequest;
use crate::registry::AgentRegistry;
use kodegen_mcp_schema::claude_agent::{
    ClaudeAgentAction, ClaudeAgentArgs, ClaudeAgentOutput, ClaudeAgentPrompts,
    CLAUDE_AGENT,
};
use kodegen_mcp_schema::{McpError, Tool, ToolExecutionContext, ToolResponse};
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
    type Prompts = ClaudeAgentPrompts;

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

    async fn execute(&self, args: Self::Args, ctx: ToolExecutionContext) -> Result<ToolResponse<<Self::Args as kodegen_mcp_schema::ToolArgs>::Output>, McpError> {
        let connection_id = ctx.connection_id().unwrap_or("default");

        let output = match args.action {
            ClaudeAgentAction::List => {
                // Get typed agent summaries directly (no JSON parsing!)
                let agents = self.registry.list_all(connection_id).await
                    .map_err(McpError::Other)?;
                
                let count = agents.len();
                
                ClaudeAgentOutput {
                    agent: 0,
                    action: "LIST".to_string(),
                    session_id: None,
                    output: format!("{} agent(s) active", count),
                    message_count: None,
                    working: None,
                    completed: true,
                    exit_code: Some(0),
                    agents: Some(agents),
                }
            }
            ClaudeAgentAction::Kill => {
                if let Some(session_id) = self.registry.remove_session(connection_id, args.agent).await {
                    self.registry.manager().terminate_session(&session_id).await
                        .map_err(|e| McpError::Other(e.into()))?;
                    ClaudeAgentOutput {
                        agent: args.agent,
                        action: "KILL".to_string(),
                        session_id: None,
                        output: format!("Agent {} terminated", args.agent),
                        message_count: None,
                        working: None,
                        completed: true,
                        exit_code: Some(0),
                        agents: None,
                    }
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
                
                // Convert Vec<SerializedMessage> to formatted string
                let output = serde_json::to_string_pretty(&output_response.output)
                    .unwrap_or_else(|_| "[]".to_string());
                
                ClaudeAgentOutput {
                    agent: args.agent,
                    action: "READ".to_string(),
                    session_id: Some(session_id),
                    output,
                    message_count: Some(info.message_count),
                    working: Some(info.working),
                    completed: info.is_complete,
                    exit_code: if info.is_complete { Some(0) } else { None },
                    agents: None,
                }
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

                ClaudeAgentOutput {
                    agent: args.agent,
                    action: "SPAWN".to_string(),
                    session_id: Some(session_id),
                    output: format!("[Agent {} spawned]\nUse action=READ to check progress.", args.agent),
                    message_count: None,
                    working: Some(true),
                    completed: false,
                    exit_code: None,
                    agents: None,
                }
            }
            ClaudeAgentAction::Send => {
                let prompt = args.prompt.as_ref()
                    .ok_or_else(|| McpError::invalid_arguments("prompt required for SEND"))?;

                let session_id = self.registry.get_session_id(connection_id, args.agent).await
                    .map_err(McpError::Other)?;

                // Send message to agent
                self.registry.manager().send_message(&session_id, prompt).await
                    .map_err(|e| McpError::Other(e.into()))?;

                ClaudeAgentOutput {
                    agent: args.agent,
                    action: "SEND".to_string(),
                    session_id: Some(session_id),
                    output: format!("[Prompt sent to agent {}]\nUse action=READ to check progress.", args.agent),
                    message_count: None,
                    working: Some(true),
                    completed: false,
                    exit_code: None,
                    agents: None,
                }
            }
        };

        let summary = format!(
            "\\x1b[35m󰚩 Claude Agent: {}\\x1b[0m\\n  Agent: {} · Status: {}",
            output.action,
            output.agent,
            if output.completed { "completed" } else { "in progress" }
        );

        Ok(ToolResponse::new(summary, output))
    }

}
