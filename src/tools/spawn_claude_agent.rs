use crate::manager::AgentManager;
use kodegen_mcp_schema::claude_agent::{SpawnClaudeAgentArgs, SpawnClaudeAgentPromptArgs};
use kodegen_mcp_tool::Tool;
use rmcp::model::{Content, PromptMessage, PromptMessageContent, PromptMessageRole};
use serde_json::json;
use std::sync::Arc;
use tokio::time::Duration;

// Import resolve method extension for schema's PromptInput type
use crate::types::prompt_input;

// ============================================================================
// ARGS STRUCTS - Imported from kodegen_mcp_schema::claude_agent
// ============================================================================

// ============================================================================
// TOOL STRUCT
// ============================================================================

/// MCP tool for spawning new Claude agent sessions
#[derive(Clone)]
pub struct SpawnClaudeAgentTool {
    agent_manager: Arc<AgentManager>,
    prompt_manager: Arc<kodegen_tools_prompt::PromptManager>,
}

impl SpawnClaudeAgentTool {
    /// Create a new spawn agent tool with required dependencies
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
}

// ============================================================================
// TOOL TRAIT IMPLEMENTATION
// ============================================================================

impl Tool for SpawnClaudeAgentTool {
    type Args = SpawnClaudeAgentArgs;
    type PromptArgs = SpawnClaudeAgentPromptArgs;

    fn name() -> &'static str {
        "claude_spawn_agent"
    }

    fn description() -> &'static str {
        "Spawn one or more Claude agent sessions for parallel task delegation. \
         Each agent gets identical configuration and can work independently. \
         Use worker_count for parallel processing of the same task. \
         Returns session IDs and initial status for each agent."
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

    async fn execute(&self, args: Self::Args) -> Result<Vec<Content>, kodegen_mcp_tool::error::McpError> {
        // Resolve prompt (render template if needed)
        let resolved_prompt = prompt_input::resolve_schema_prompt(&args.prompt, &self.prompt_manager)
            .await
            .map_err(|e| kodegen_mcp_tool::error::McpError::Other(e.into()))?;

        let mut results = Vec::new();
        let mut session_ids = Vec::new();

        for i in 0..args.worker_count {
            let label = args.label.as_ref().map_or_else(
                || format!("Agent-{}", i + 1),
                |l| format!("{}-{}", l, i + 1),
            );

            let request = crate::manager::SpawnSessionRequest {
                prompt: resolved_prompt.clone(),
                system_prompt: args.system_prompt.clone(),
                allowed_tools: args.allowed_tools.clone(),
                disallowed_tools: args.disallowed_tools.clone(),
                max_turns: args.max_turns,
                model: args.model.clone(),
                cwd: args.cwd.clone(),
                add_dirs: args.add_dirs.clone(),
                label,
            };

            let session_id = self
                .agent_manager
                .spawn_session(request)
                .await
                .map_err(|e| kodegen_mcp_tool::error::McpError::Other(e.into()))?;

            session_ids.push(session_id.clone());

            tokio::time::sleep(Duration::from_millis(args.initial_delay_ms)).await;

            let info = self
                .agent_manager
                .get_session_info(&session_id)
                .await
                .map_err(|e| kodegen_mcp_tool::error::McpError::Other(e.into()))?;

            results.push(info);
        }

        let mut contents = Vec::new();

        // Human summary
        let summary = if results.len() == 1 {
            // Single agent format
            let agent = &results[0];
            format!(
                "✓ Spawned Claude agent\n\n\
                 Session: {}\n\
                 Label: {}\n\
                 Status: {} (turn {}/{})\n\
                 Runtime: {:.1}s\n\n\
                 Use read_claude_agent_output to monitor progress",
                agent.session_id,
                agent.label,
                if agent.working { "Working" } else { "Idle" },
                agent.turn_count,
                agent.max_turns,
                agent.runtime_ms as f64 / 1000.0
            )
        } else {
            // Multiple agents format
            let agent_list = results.iter()
                .map(|a| format!(
                    "  • {} [{}] - {} (turn {}/{})",
                    a.session_id,
                    a.label,
                    if a.working { "Working" } else { "Idle" },
                    a.turn_count,
                    a.max_turns
                ))
                .collect::<Vec<_>>()
                .join("\n");
            
            format!(
                "✓ Spawned {} Claude agent(s)\n\n\
                 Sessions:\n{}\n\n\
                 Use read_claude_agent_output to monitor progress",
                results.len(),
                agent_list
            )
        };
        contents.push(Content::text(summary));

        // JSON metadata
        let metadata = json!({
            "session_ids": session_ids,
            "worker_count": args.worker_count,
            "agents": results
        });
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
                text: r#"# claude_spawn_agent

Spawn one or more Claude agent sessions for parallel task delegation. Each agent runs independently with identical configuration.

## Example: Spawn single agent with plain string
```json
{
  "prompt": {
    "type": "string",
    "value": "Analyze the codebase and identify all TODO comments"
  },
  "max_turns": 5,
  "system_prompt": "You are a code analysis expert"
}
```

## Example: Spawn with template
```json
{
  "prompt": {
    "type": "template",
    "value": {
      "name": "code_review",
      "parameters": {
        "file_path": "src/main.rs",
        "focus_areas": ["security", "performance"]
      }
    }
  },
  "worker_count": 3,
  "label": "SecurityReview",
  "allowed_tools": ["fs_read_file", "fs_list_directory", "grep_search"],
  "max_turns": 10
}
```

## Response
Returns session IDs and initial status for each spawned agent, including working state, turn count, and message preview.

## Workflow
1. Spawn agent(s) with this tool
2. Poll with `claude_read_agent_output` to get responses
3. Send follow-ups with `claude_send_agent_prompt`
4. Terminate with `claude_terminate_agent_session` when done"#.to_string(),
            },
        }])
    }
}
