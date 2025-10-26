use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

use super::base::{Agent, AgentCapability, AgentContext, AgentResult};
use crate::context::ContextManager;
use crate::llm::{LlmClient, Message};
use crate::mcp::McpClient;
use crate::tools::ToolExecutor;

#[allow(dead_code)]
pub struct McpAgent {
    name: String,
    mcp_client: Arc<McpClient>,
}

#[allow(dead_code)]
impl McpAgent {
    pub fn new(mcp_client: Arc<McpClient>) -> Self {
        Self {
            name: "mcp".to_string(),
            mcp_client,
        }
    }
}

#[async_trait]
impl Agent for McpAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::TaskOrchestration]
    }

    fn can_handle(&self, task: &str) -> bool {
        let keywords = ["mcp", "tool", "external", "server", "plugin"];
        keywords.iter().any(|kw| task.to_lowercase().contains(kw))
    }

    async fn execute(
        &self,
        task: &str,
        context: &mut AgentContext,
        llm: Arc<dyn LlmClient>,
        _tools: Arc<ToolExecutor>,
        _context_mgr: Arc<ContextManager>,
    ) -> Result<AgentResult> {
        debug!("MCP agent executing: {}", task);

        let all_tools = self.mcp_client.list_all_tools().await?;

        let mut tools_description = String::new();
        for (server_name, tools) in &all_tools {
            tools_description.push_str(&format!("\nServer '{}':\n", server_name));
            for tool in tools {
                tools_description.push_str(&format!("  - {}: {}\n", tool.name, tool.description));
            }
        }

        let system_prompt = format!(
            r#"You are an MCP tool orchestration agent.
You have access to the following MCP tools:
{}

When asked to perform a task:
1. Determine which MCP tool(s) to use
2. Provide the tool name and arguments in JSON format

Response format:
TOOL: <tool_name>
ARGUMENTS: <json_arguments>

If multiple tools are needed, provide them on separate lines."#,
            tools_description
        );

        let messages = vec![
            Message::system(system_prompt),
            Message::user(format!("Task: {}", task)),
        ];

        let response = llm.chat_with_history(messages, "default").await?;

        let tool_calls = self.parse_tool_calls(&response);

        let mut results = Vec::new();
        for (tool_name, args) in tool_calls {
            info!("Calling MCP tool: {} with args: {:?}", tool_name, args);

            match self.mcp_client.call_tool(&tool_name, args).await {
                Ok(result) => {
                    let result_str = serde_json::to_string_pretty(&result)?;
                    results.push(format!("Tool '{}' result:\n{}", tool_name, result_str));
                }
                Err(e) => {
                    results.push(format!("Tool '{}' failed: {}", tool_name, e));
                }
            }
        }

        context.add_message(format!("MCP task: {}", task));
        context.add_message(format!("Results: {}", results.join("\n\n")));

        if results.is_empty() {
            Ok(AgentResult::failure("No MCP tools were called"))
        } else {
            Ok(AgentResult::success(results.join("\n\n")))
        }
    }
}

#[allow(dead_code)]
impl McpAgent {
    fn parse_tool_calls(
        &self,
        response: &str,
    ) -> Vec<(String, HashMap<String, serde_json::Value>)> {
        let mut calls = Vec::new();
        let lines: Vec<&str> = response.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            if lines[i].starts_with("TOOL:") {
                let tool_name = lines[i]
                    .strip_prefix("TOOL:")
                    .unwrap_or("")
                    .trim()
                    .to_string();

                if i + 1 < lines.len() && lines[i + 1].starts_with("ARGUMENTS:") {
                    let args_str = lines[i + 1].strip_prefix("ARGUMENTS:").unwrap_or("{}");

                    if let Ok(args) =
                        serde_json::from_str::<HashMap<String, serde_json::Value>>(args_str)
                    {
                        calls.push((tool_name, args));
                    } else {
                        calls.push((tool_name, HashMap::new()));
                    }

                    i += 2;
                } else {
                    calls.push((tool_name, HashMap::new()));
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        calls
    }
}
