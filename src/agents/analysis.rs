use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;

use super::base::{Agent, AgentCapability, AgentContext, AgentResult};
use crate::context::ContextManager;
use crate::llm::{LlmClient, Message};
use crate::tools::ToolExecutor;

pub struct AnalysisAgent {
    name: String,
}

impl AnalysisAgent {
    pub fn new() -> Self {
        Self {
            name: "analysis".to_string(),
        }
    }
}

impl Default for AnalysisAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for AnalysisAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Analysis]
    }

    fn can_handle(&self, task: &str) -> bool {
        let keywords = [
            "analyze",
            "explain",
            "review",
            "understand",
            "investigate",
            "examine",
        ];
        keywords.iter().any(|kw| task.to_lowercase().contains(kw))
    }

    async fn execute(
        &self,
        task: &str,
        context: &mut AgentContext,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolExecutor>,
        context_mgr: Arc<ContextManager>,
    ) -> Result<AgentResult> {
        debug!("Analysis agent executing: {}", task);

        let mut analysis_context = String::new();

        if let Some(file_path) = self.extract_file_reference(task) {
            match tools.read_file(&file_path).await {
                Ok(content) => {
                    analysis_context = format!("File: {}\n\n{}", file_path, content);
                    context.add_message(format!("Analyzing file: {}", file_path));
                }
                Err(e) => {
                    debug!("Could not read file {}: {}", file_path, e);
                }
            }
        }

        let project_context = context_mgr.get_project_summary().await?;

        let system_prompt = r#"You are a code analysis expert.
When analyzing code or projects:
1. Examine the structure and organization
2. Identify patterns, issues, and improvements
3. Provide clear, actionable insights
4. Consider best practices and common pitfalls
5. Be thorough but concise"#;

        let user_message = if analysis_context.is_empty() {
            format!("Task: {}\n\nProject context:\n{}", task, project_context)
        } else {
            format!(
                "Task: {}\n\n{}\n\nProject context:\n{}",
                task, analysis_context, project_context
            )
        };

        let messages = vec![Message::system(system_prompt), Message::user(user_message)];

        let response = llm.chat_with_history(messages, "default").await?;

        context.add_message(format!("Analysis task: {}", task));
        context.add_message(format!("Analysis result: {}", response));

        Ok(AgentResult::success(response))
    }
}

impl AnalysisAgent {
    fn extract_file_reference(&self, task: &str) -> Option<String> {
        let words: Vec<&str> = task.split_whitespace().collect();

        for word in words {
            if word.contains('.') && !word.starts_with('.') {
                let extensions = ["rs", "py", "js", "ts", "java", "go", "cpp", "c", "h"];
                if extensions.iter().any(|ext| word.ends_with(ext)) {
                    return Some(word.to_string());
                }
            }
        }
        None
    }
}
