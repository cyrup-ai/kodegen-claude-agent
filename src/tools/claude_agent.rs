use crate::manager::{AgentManager, SpawnSessionRequest};
use crate::types::prompt_input;
use kodegen_mcp_schema::claude_agent::{AgentAction, ClaudeAgentArgs, ClaudeAgentPromptArgs, CLAUDE_AGENT};
use kodegen_mcp_tool::{Tool, ToolExecutionContext, error::McpError};
use rmcp::model::{Content, PromptMessage, PromptMessageContent, PromptMessageRole};
use serde_json::json;
use std::sync::Arc;
use tokio::time::Duration;

/// Unified MCP tool for Claude agent lifecycle management
#[derive(Clone)]
pub struct ClaudeAgentTool {
    agent_manager: Arc<AgentManager>,
    prompt_manager: Arc<kodegen_tools_prompt::PromptManager>,
}

impl ClaudeAgentTool {
    /// Create a new unified claude_agent tool with required dependencies
    #[must_use]
    pub fn new(
        agent_manager: Arc<AgentManager>,
        prompt_manager: Arc<kodegen_tools_prompt::PromptManager>,
    ) -> Self {
        Self {
            agent_manager,
            prompt_manager,
        }
    }

    /// Handle spawn action
    async fn handle_spawn(&self, args: ClaudeAgentArgs, ctx: &ToolExecutionContext) -> Result<Vec<Content>, McpError> {
        let prompt = args.prompt
            .ok_or_else(|| McpError::Other(anyhow::anyhow!("prompt is required for spawn action")))?;

        let resolved_prompt = prompt_input::resolve_schema_prompt(&prompt, &self.prompt_manager)
            .await
            .map_err(|e| McpError::Other(e.into()))?;

        let mut session_ids = Vec::new();
        let mut results = Vec::new();

        for i in 0..args.worker_count {
            let label = args.label.as_ref().map_or_else(
                || format!("Agent-{}", i + 1),
                |l| if args.worker_count > 1 {
                    format!("{}-{}", l, i + 1)
                } else {
                    l.clone()
                },
            );

            let request = SpawnSessionRequest {
                prompt: resolved_prompt.clone(),
                system_prompt: args.system_prompt.clone(),
                allowed_tools: args.allowed_tools.clone().unwrap_or_default(),
                disallowed_tools: args.disallowed_tools.clone().unwrap_or_default(),
                max_turns: args.max_turns.unwrap_or(10),
                model: args.model.clone(),
                cwd: args.cwd.clone(),
                add_dirs: args.add_dirs.clone().unwrap_or_default(),
                label,
            };

            let session_id = self.agent_manager
                .spawn_session(request)
                .await
                .map_err(|e| McpError::Other(e.into()))?;

            session_ids.push(session_id.clone());

            if args.blocking {
                self.wait_for_completion(&session_id, ctx).await?;
            }

            let info = self.agent_manager
                .get_session_info(&session_id)
                .await
                .map_err(|e| McpError::Other(e.into()))?;

            results.push((session_id, info));
        }

        // Build response
        let mut contents = Vec::new();

        // Human summary
        let summary = if args.worker_count == 1 {
            let (session_id, info) = &results[0];
            format!(
                "✨ Agent spawned: {}\nLabel: {}\nStatus: {:?}\nTurns: {}/{}",
                session_id,
                info.label,
                if info.working { "Working" } else { "Idle" },
                info.turn_count,
                info.max_turns
            )
        } else {
            let ids: Vec<&str> = session_ids.iter().map(|id| id.as_str()).collect();
            format!(
                "✨ {} agents spawned:\n{}",
                args.worker_count,
                ids.join("\n")
            )
        };
        contents.push(Content::text(summary));

        // Machine-readable JSON
        let metadata = json!({
            "action": "spawn",
            "worker_count": args.worker_count,
            "session_ids": session_ids.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
            "blocking": args.blocking,
            "sessions": results.iter().map(|(id, info)| json!({
                "session_id": id.as_str(),
                "label": info.label,
                "status": if info.working { "Working" } else { "Idle" },
                "turn_count": info.turn_count,
                "max_turns": info.max_turns,
            })).collect::<Vec<_>>(),
        });
        contents.push(Content::text(serde_json::to_string_pretty(&metadata)?));

        Ok(contents)
    }

    /// Handle send action
    async fn handle_send(&self, args: ClaudeAgentArgs, ctx: &ToolExecutionContext) -> Result<Vec<Content>, McpError> {
        let session_id = args.session_id
            .ok_or_else(|| McpError::Other(anyhow::anyhow!("session_id is required for send action")))?;

        let prompt = args.prompt
            .ok_or_else(|| McpError::Other(anyhow::anyhow!("prompt is required for send action")))?;

        let resolved_prompt = prompt_input::resolve_schema_prompt(&prompt, &self.prompt_manager)
            .await
            .map_err(|e| McpError::Other(e.into()))?;

        self.agent_manager
            .send_message(&session_id, &resolved_prompt)
            .await
            .map_err(|e| McpError::Other(e.into()))?;

        if args.blocking {
            self.wait_for_completion(&session_id, ctx).await?;
        }

        let working = self.agent_manager
            .is_working(&session_id)
            .await
            .map_err(|e| McpError::Other(e.into()))?;

        let info = self.agent_manager
            .get_session_info(&session_id)
            .await
            .map_err(|e| McpError::Other(e.into()))?;

        let mut contents = Vec::new();

        // Human summary
        let status = if info.turn_count >= info.max_turns {
            "⚠️  Max turns reached"
        } else if working {
            "⏳ Agent working"
        } else {
            "✓ Message sent"
        };

        let summary = format!(
            "{}\nSession: {}\nTurns: {}/{}",
            status,
            session_id,
            info.turn_count,
            info.max_turns
        );
        contents.push(Content::text(summary));

        // Machine-readable JSON
        let metadata = json!({
            "action": "send",
            "session_id": session_id,
            "working": working,
            "turn_count": info.turn_count,
            "max_turns": info.max_turns,
            "blocking": args.blocking,
        });
        contents.push(Content::text(serde_json::to_string_pretty(&metadata)?));

        Ok(contents)
    }

    /// Handle terminate action
    async fn handle_terminate(&self, args: ClaudeAgentArgs) -> Result<Vec<Content>, McpError> {
        let session_id = args.session_id
            .ok_or_else(|| McpError::Other(anyhow::anyhow!("session_id is required for terminate action")))?;

        let info = self.agent_manager
            .get_session_info(&session_id)
            .await
            .map_err(|e| McpError::Other(e.into()))?;

        self.agent_manager
            .terminate_session(&session_id)
            .await
            .map_err(|e| McpError::Other(e.into()))?;

        let mut contents = Vec::new();

        let summary = format!(
            "🛑 Session terminated: {}\nLabel: {}\nFinal turns: {}",
            session_id,
            info.label,
            info.turn_count
        );
        contents.push(Content::text(summary));

        let metadata = json!({
            "action": "terminate",
            "session_id": session_id,
            "final_turn_count": info.turn_count,
        });
        contents.push(Content::text(serde_json::to_string_pretty(&metadata)?));

        Ok(contents)
    }

    /// Wait for agent to finish working (blocking mode) with real-time event-driven streaming
    async fn wait_for_completion(&self, session_id: &str, ctx: &ToolExecutionContext) -> Result<(), McpError> {
        // Subscribe to the message broadcast channel
        let mut message_rx = self.agent_manager
            .subscribe_to_messages(session_id)
            .await
            .map_err(|e| McpError::Other(e.into()))?;

        let mut last_streamed_content = String::new();

        loop {
            tokio::select! {
                // Wait for new message event (event-driven, no polling!)
                Ok(msg) = message_rx.recv() => {
                    // Extract text content from assistant messages
                    if msg.message_type == "assistant"
                        && let Some(content) = msg.content.get("content")
                        && let Some(text) = content.as_str()
                    {
                        // Only stream if content is different from last
                        if text != last_streamed_content {
                            // Truncate long messages for streaming
                            let display = if text.len() > 500 {
                                format!("{}... [truncated]", &text[..500])
                            } else {
                                text.to_string()
                            };
                            
                            ctx.stream(&display).await.ok();
                            last_streamed_content = text.to_string();
                        }
                    }
                }
                // Check for completion every 100ms
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    let working = self.agent_manager
                        .is_working(session_id)
                        .await
                        .map_err(|e| McpError::Other(e.into()))?;
                    
                    if !working {
                        break; // Session complete or idle
                    }
                }
            }
        }
        Ok(())
    }
}

impl Tool for ClaudeAgentTool {
    type Args = ClaudeAgentArgs;
    type PromptArgs = ClaudeAgentPromptArgs;

    fn name() -> &'static str {
        CLAUDE_AGENT
    }

    fn description() -> &'static str {
        "Unified Claude agent interface with real-time streaming. Handles spawn, send, and terminate \
         operations in a single tool. MCP delivers output as it's produced - no polling needed.\n\n\
         **Actions:**\n\
         • spawn: Create new agent(s) with optional parallel workers\n\
         • send: Send prompt to existing session\n\
         • terminate: Clean up session\n\n\
         **Streaming:**\n\
         Output streams in real-time via MCP protocol. Use blocking=true to wait for completion.\n\n\
         **Example:**\n\
         {\"action\": \"spawn\", \"prompt\": {\"type\": \"string\", \"value\": \"Analyze codebase\"}, \"label\": \"analyzer\"}"
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
        match args.action {
            AgentAction::Spawn => self.handle_spawn(args, &ctx).await,
            AgentAction::Send => self.handle_send(args, &ctx).await,
            AgentAction::Terminate => self.handle_terminate(args).await,
        }
    }

    fn prompt_arguments() -> Vec<rmcp::model::PromptArgument> {
        vec![]
    }

    async fn prompt(
        &self,
        _args: Self::PromptArgs,
    ) -> Result<Vec<PromptMessage>, McpError> {
        Ok(vec![
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::Text {
                    text: "# claude_agent\n\nUnified Claude agent interface for spawning, interacting with, and managing agent sessions.\n\n## Actions\n\n### spawn\nCreate one or more new agent sessions:\n```json\n{\n  \"action\": \"spawn\",\n  \"prompt\": {\"type\": \"string\", \"value\": \"Analyze the codebase\"},\n  \"worker_count\": 1,\n  \"max_turns\": 10,\n  \"label\": \"Analyzer\"\n}\n```\n\n### send\nSend prompt to existing session:\n```json\n{\n  \"action\": \"send\",\n  \"session_id\": \"uuid-abc-123\",\n  \"prompt\": {\"type\": \"string\", \"value\": \"Now fix the issues\"}\n}\n```\n\n### terminate\nTerminate session:\n```json\n{\n  \"action\": \"terminate\",\n  \"session_id\": \"uuid-abc-123\"\n}\n```\n\n## Blocking Mode\n\nSet `blocking: true` to wait for agent to complete before returning.\n\n## Parallel Workers\n\nSet `worker_count > 1` to spawn multiple agents with identical configuration.".to_string(),
                },
            },
        ])
    }
}
