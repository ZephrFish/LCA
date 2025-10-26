use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::context::ContextManager;
use crate::llm::LlmClient;
use crate::tools::ToolExecutor;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentCapability {
    CodeGeneration,
    CodeEditing,
    ShellExecution,
    FileOperations,
    Analysis,
    TaskOrchestration,
    ContextManagement,
}

#[derive(Debug, Clone)]
pub struct AgentContext {
    pub working_directory: String,
    pub conversation_history: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl AgentContext {
    pub fn new(working_directory: impl Into<String>) -> Self {
        Self {
            working_directory: working_directory.into(),
            conversation_history: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_message(&mut self, message: impl Into<String>) {
        self.conversation_history.push(message.into());
    }

    #[allow(dead_code)]
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentResult {
    pub success: bool,
    pub output: String,
    pub metadata: HashMap<String, String>,
}

impl AgentResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            metadata: HashMap::new(),
        }
    }

    pub fn failure(output: impl Into<String>) -> Self {
        Self {
            success: false,
            output: output.into(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;

    #[allow(dead_code)]
    fn capabilities(&self) -> Vec<AgentCapability>;

    fn can_handle(&self, task: &str) -> bool;

    async fn execute(
        &self,
        task: &str,
        context: &mut AgentContext,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolExecutor>,
        context_mgr: Arc<ContextManager>,
    ) -> Result<AgentResult>;

    #[allow(dead_code)]
    async fn plan(&self, task: &str, llm: Arc<dyn LlmClient>) -> Result<Vec<String>> {
        use crate::llm::Message;

        let system_prompt = format!(
            "You are a {} agent. Break down the following task into actionable steps. \
             Return a JSON array of step descriptions.",
            self.name()
        );

        let messages = vec![Message::system(system_prompt), Message::user(task)];

        let response = llm.chat_with_history(messages, "default").await?;

        let steps: Vec<String> =
            serde_json::from_str(&response).unwrap_or_else(|_| vec![task.to_string()]);

        Ok(steps)
    }
}

pub struct AgentRegistry {
    agents: HashMap<String, Arc<dyn Agent>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    pub fn register(&mut self, agent: Arc<dyn Agent>) {
        self.agents.insert(agent.name().to_string(), agent);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Agent>> {
        self.agents.get(name).cloned()
    }

    pub fn find_capable(&self, task: &str) -> Vec<Arc<dyn Agent>> {
        self.agents
            .values()
            .filter(|agent| agent.can_handle(task))
            .cloned()
            .collect()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
