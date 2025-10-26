use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;

use super::base::{Agent, AgentCapability, AgentContext, AgentResult};
use crate::context::ContextManager;
use crate::llm::{LlmClient, Message};
use crate::tools::ToolExecutor;

pub struct CodeAgent {
    name: String,
}

impl CodeAgent {
    pub fn new() -> Self {
        Self {
            name: "code".to_string(),
        }
    }
}

impl Default for CodeAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for CodeAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::CodeGeneration,
            AgentCapability::CodeEditing,
        ]
    }

    fn can_handle(&self, task: &str) -> bool {
        let keywords = [
            "code",
            "implement",
            "function",
            "class",
            "write",
            "generate",
            "refactor",
        ];
        keywords.iter().any(|kw| task.to_lowercase().contains(kw))
    }

    async fn execute(
        &self,
        task: &str,
        context: &mut AgentContext,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolExecutor>,
        _context_mgr: Arc<ContextManager>,
    ) -> Result<AgentResult> {
        debug!("Code agent executing: {}", task);

        let system_prompt = r#"You are an expert code generation agent.
When asked to write code:
1. Analyze the requirements carefully
2. Generate clean, well-documented code
3. Follow best practices for the language
4. Include error handling where appropriate
5. Return the code with explanations

Format your response as:
```language
<code here>
```
Explanation: <your explanation>"#;

        let history_context = context.conversation_history.join("\n");
        let full_task = if history_context.is_empty() {
            task.to_string()
        } else {
            format!(
                "Previous context:\n{}\n\nCurrent task: {}",
                history_context, task
            )
        };

        let messages = vec![Message::system(system_prompt), Message::user(full_task)];

        let response = llm.chat_with_history(messages, "default").await?;

        context.add_message(format!("Code task: {}", task));
        context.add_message(format!("Response: {}", response));

        if response.contains("```") {
            if let Some(file_path) = self.extract_file_path(&response) {
                let code = self.extract_code_block(&response);
                tools.write_file(&file_path, &code).await?;

                return Ok(AgentResult::success(response).with_metadata("file_written", file_path));
            }
        }

        Ok(AgentResult::success(response))
    }
}

impl CodeAgent {
    fn extract_code_block(&self, response: &str) -> String {
        let start = response
            .find("```")
            .map(|i| response[i..].find('\n').map(|j| i + j + 1).unwrap_or(i + 3));

        let end = response.rfind("```");

        match (start, end) {
            (Some(s), Some(e)) if s < e => response[s..e].trim().to_string(),
            _ => response.to_string(),
        }
    }

    fn extract_file_path(&self, response: &str) -> Option<String> {
        let lines: Vec<&str> = response.lines().collect();
        for line in lines {
            if line.starts_with("File:") || line.starts_with("file:") {
                return Some(line.split(':').nth(1)?.trim().to_string());
            }
        }
        None
    }
}
