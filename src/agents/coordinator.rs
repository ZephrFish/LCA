use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info};

use super::base::{Agent, AgentCapability, AgentContext, AgentRegistry, AgentResult};
use crate::context::ContextManager;
use crate::llm::{LlmClient, Message};
use crate::tools::ToolExecutor;

pub struct CoordinatorAgent {
    name: String,
    registry: Arc<AgentRegistry>,
}

impl CoordinatorAgent {
    pub fn new(registry: Arc<AgentRegistry>) -> Self {
        Self {
            name: "coordinator".to_string(),
            registry,
        }
    }

    async fn decompose_task(&self, task: &str, llm: Arc<dyn LlmClient>) -> Result<Vec<SubTask>> {
        let system_prompt = r#"You are a task decomposition expert. Analyze the user's task and break it down into subtasks.

Available agent types:
- code: Generate or edit code
- shell: Execute shell commands
- file: Read, write, search files
- analysis: Analyze code or provide insights
- mcp: Use external MCP tools

Guidelines:
1. Keep subtasks atomic and focused
2. Identify dependencies between subtasks
3. Choose the most appropriate agent for each subtask
4. Consider parallel execution when possible

Return ONLY a valid JSON array of subtasks in this exact format:
[
  {
    "description": "what needs to be done",
    "agent_type": "code|shell|file|analysis|mcp",
    "dependencies": [0, 1]
  }
]

Example:
[
  {"description": "Read the configuration file", "agent_type": "file", "dependencies": []},
  {"description": "Analyze the configuration structure", "agent_type": "analysis", "dependencies": [0]},
  {"description": "Generate updated configuration", "agent_type": "code", "dependencies": [1]}
]"#;

        let messages = vec![
            Message::system(system_prompt),
            Message::user(format!("Task: {}\n\nBreak this down into subtasks:", task)),
        ];

        let response = llm.chat_with_history(messages, "default").await?;

        debug!("Task decomposition response: {}", response);

        let subtasks: Vec<SubTask> = self.parse_subtasks(&response, task);

        if subtasks.is_empty() {
            return Ok(vec![SubTask {
                description: task.to_string(),
                agent_type: self.infer_agent_type(task).to_string(),
                dependencies: vec![],
            }]);
        }

        Ok(subtasks)
    }

    fn parse_subtasks(&self, response: &str, fallback_task: &str) -> Vec<SubTask> {
        let json_start = response.find('[');
        let json_end = response.rfind(']');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &response[start..=end];

            if let Ok(subtasks) = serde_json::from_str::<Vec<SubTask>>(json_str) {
                if !subtasks.is_empty() {
                    return subtasks;
                }
            }
        }

        vec![SubTask {
            description: fallback_task.to_string(),
            agent_type: self.infer_agent_type(fallback_task).to_string(),
            dependencies: vec![],
        }]
    }

    fn infer_agent_type(&self, task: &str) -> &str {
        let task_lower = task.to_lowercase();

        if task_lower.contains("code")
            || task_lower.contains("implement")
            || task_lower.contains("write")
        {
            "code"
        } else if task_lower.contains("run")
            || task_lower.contains("execute")
            || task_lower.contains("command")
        {
            "shell"
        } else if task_lower.contains("read")
            || task_lower.contains("file")
            || task_lower.contains("search")
        {
            "file"
        } else if task_lower.contains("mcp")
            || task_lower.contains("tool")
            || task_lower.contains("external")
        {
            "mcp"
        } else {
            "analysis"
        }
    }

    async fn execute_subtask(
        &self,
        subtask: &SubTask,
        context: &mut AgentContext,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolExecutor>,
        context_mgr: Arc<ContextManager>,
    ) -> Result<AgentResult> {
        info!(
            "Executing subtask: {} with {} agent",
            subtask.description, subtask.agent_type
        );

        let agent = self
            .registry
            .get(&subtask.agent_type)
            .ok_or_else(|| anyhow::anyhow!("Agent type '{}' not found", subtask.agent_type))?;

        agent
            .execute(&subtask.description, context, llm, tools, context_mgr)
            .await
    }
}

#[async_trait]
impl Agent for CoordinatorAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::TaskOrchestration]
    }

    fn can_handle(&self, _task: &str) -> bool {
        true
    }

    async fn execute(
        &self,
        task: &str,
        context: &mut AgentContext,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolExecutor>,
        context_mgr: Arc<ContextManager>,
    ) -> Result<AgentResult> {
        debug!("Coordinator analyzing task: {}", task);

        let subtasks = self.decompose_task(task, llm.clone()).await?;

        let mut results: Vec<AgentResult> = Vec::new();
        let mut all_success = true;

        for (idx, subtask) in subtasks.iter().enumerate() {
            for dep_idx in &subtask.dependencies {
                if *dep_idx >= idx {
                    return Ok(AgentResult::failure(
                        "Invalid dependency: forward reference",
                    ));
                }
                if !results[*dep_idx].success {
                    return Ok(AgentResult::failure(format!(
                        "Dependency {} failed, skipping subtask {}",
                        dep_idx, idx
                    )));
                }
            }

            let result = self
                .execute_subtask(
                    subtask,
                    context,
                    llm.clone(),
                    tools.clone(),
                    context_mgr.clone(),
                )
                .await?;

            all_success &= result.success;
            results.push(result);
        }

        let summary = results
            .iter()
            .enumerate()
            .map(|(idx, r)| {
                format!(
                    "Subtask {}: {}",
                    idx,
                    if r.success { "SUCCESS" } else { "FAILED" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(AgentResult {
            success: all_success,
            output: summary,
            metadata: std::collections::HashMap::new(),
        })
    }
}

#[derive(Debug, serde::Deserialize)]
struct SubTask {
    description: String,
    agent_type: String,
    dependencies: Vec<usize>,
}
