// Category HTTP Server: Claude Agent Tools
//
// This binary serves Claude agent delegation tools over HTTP/HTTPS transport.
// Managed by kodegend daemon, typically running on port 30460.

use anyhow::Result;
use kodegen_server_http::{run_http_server, Managers, RouterSet, ShutdownHook, register_tool};
use rmcp::handler::server::router::{prompt::PromptRouter, tool::ToolRouter};
use std::sync::Arc;

// Wrapper to impl ShutdownHook for Arc<AgentManager>
struct AgentManagerWrapper(Arc<kodegen_claude_agent::AgentManager>);

impl ShutdownHook for AgentManagerWrapper {
    fn shutdown(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let manager = self.0.clone();
        Box::pin(async move {
            manager.shutdown().await.map_err(|e| anyhow::anyhow!("{}", e))
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    run_http_server("claude-agent", |_config, _tracker| {
        Box::pin(async move {
        let mut tool_router = ToolRouter::new();
        let mut prompt_router = PromptRouter::new();
        let managers = Managers::new();

        // Initialize agent manager
        let agent_manager = Arc::new(kodegen_claude_agent::AgentManager::new());
        managers.register(AgentManagerWrapper(agent_manager.clone())).await;

        // Initialize agent registry
        let agent_registry = Arc::new(kodegen_claude_agent::AgentRegistry::new(agent_manager.clone()));

        // Register unified Claude agent tool
        use kodegen_claude_agent::tools::ClaudeAgentTool;

        (tool_router, prompt_router) = register_tool(
            tool_router,
            prompt_router,
            ClaudeAgentTool::new(agent_registry.clone()),
        );

        Ok(RouterSet::new(tool_router, prompt_router, managers))
        })
    })
    .await
}
