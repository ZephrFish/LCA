use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;

use super::base::{Agent, AgentCapability, AgentContext, AgentResult};
use crate::context::ContextManager;
use crate::llm::{LlmClient, Message};
use crate::tools::ToolExecutor;

pub struct FileAgent {
    name: String,
}

impl FileAgent {
    pub fn new() -> Self {
        Self {
            name: "file".to_string(),
        }
    }
}

impl Default for FileAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for FileAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::FileOperations]
    }

    fn can_handle(&self, task: &str) -> bool {
        let keywords = [
            "read", "write", "file", "create", "delete", "copy", "move", "search",
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
        debug!("File agent executing: {}", task);

        let system_prompt = r#"You are a file operations expert.
When asked to perform file operations:
1. Determine what file operation is needed
2. Identify the file path(s) involved
3. Provide the operation details

Respond in this format:
OPERATION: <read|write|search|list>
PATH: <file or directory path>
CONTENT: <for write operations only>
PATTERN: <for search operations only>"#;

        let messages = vec![
            Message::system(system_prompt),
            Message::user(format!(
                "Task: {}\nWorking directory: {}",
                task, context.working_directory
            )),
        ];

        let response = llm.chat_with_history(messages, "default").await?;

        let operation = self.extract_field(&response, "OPERATION");
        let path = self.extract_field(&response, "PATH");

        match operation.to_lowercase().as_str() {
            "read" => {
                let content = tools.read_file(&path).await?;
                context.add_message(format!("Read file: {}", path));
                Ok(AgentResult::success(content).with_metadata("path", path))
            }
            "write" => {
                let content = self.extract_field(&response, "CONTENT");
                tools.write_file(&path, &content).await?;
                context.add_message(format!("Wrote file: {}", path));
                Ok(AgentResult::success(format!("File written to {}", path))
                    .with_metadata("path", path))
            }
            "search" => {
                let pattern = self.extract_field(&response, "PATTERN");
                let results = tools.search_files(&path, &pattern).await?;
                context.add_message(format!("Searched in: {}", path));
                Ok(AgentResult::success(results.join("\n")).with_metadata("pattern", pattern))
            }
            "list" => {
                let files = tools.list_files(&path).await?;
                context.add_message(format!("Listed directory: {}", path));
                Ok(AgentResult::success(files.join("\n")).with_metadata("path", path))
            }
            _ => Ok(AgentResult::failure(format!(
                "Unknown operation: {}",
                operation
            ))),
        }
    }
}

impl FileAgent {
    fn extract_field(&self, response: &str, field: &str) -> String {
        let prefix = format!("{}:", field);
        for line in response.lines() {
            if line.to_uppercase().starts_with(&prefix) {
                return line[prefix.len()..].trim().to_string();
            }
        }
        String::new()
    }
}
