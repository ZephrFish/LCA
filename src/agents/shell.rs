use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, warn};

use super::base::{Agent, AgentCapability, AgentContext, AgentResult};
use crate::context::ContextManager;
use crate::llm::{LlmClient, Message};
use crate::tools::ToolExecutor;

pub struct ShellAgent {
    name: String,
}

impl ShellAgent {
    pub fn new() -> Self {
        Self {
            name: "shell".to_string(),
        }
    }
}

impl Default for ShellAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for ShellAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::ShellExecution]
    }

    fn can_handle(&self, task: &str) -> bool {
        let keywords = [
            "run", "execute", "command", "shell", "bash", "script", "install", "build", "test",
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
        debug!("Shell agent executing: {}", task);

        let system_prompt = r#"You are a shell command expert.

CRITICAL REQUIREMENT: Each COMMAND must be a SINGLE LINE. Use semicolons (;) or && to chain operations.

When asked to perform a task:
1. Determine the appropriate shell command(s)
2. Each COMMAND must be ONE LINE - use ; or && to combine multiple operations
3. For file content, use printf or echo with \n, NOT multi-line heredocs or quotes
4. Ensure commands are safe and non-destructive

Format:
COMMAND: <single-line command with ; or && for chaining>
EXPLANATION: <what it does>

Example GOOD commands:
COMMAND: printf '#!/bin/bash\necho hello\n' > script.sh && chmod +x script.sh
COMMAND: echo '#!/bin/bash' > script.sh && echo 'echo hello' >> script.sh && chmod +x script.sh

Example BAD (will FAIL - multi-line):
COMMAND: echo '#!/bin/bash
more lines...'

IMPORTANT: Use printf for newlines, NOT echo -e (the -e flag causes errors on some systems)

NEVER use rm -rf / or other destructive commands.
ALWAYS keep the entire command on ONE SINGLE LINE after "COMMAND:"."#;

        let messages = vec![
            Message::system(system_prompt),
            Message::user(format!(
                "Task: {}\nWorking directory: {}",
                task, context.working_directory
            )),
        ];

        let response = llm.chat_with_history(messages, "default").await?;

        let command = self.extract_command(&response);

        if self.is_dangerous_command(&command) {
            warn!("Dangerous command detected: {}", command);
            return Ok(AgentResult::failure(format!(
                "Refused to execute dangerous command: {}",
                command
            )));
        }

        debug!("Executing shell command: {}", command);
        let output = tools
            .execute_shell(&command, &context.working_directory)
            .await?;

        context.add_message(format!("Executed: {}", command));
        context.add_message(format!("Output: {}", output));

        // Check if we created a script and offer to test it
        let script_created = self.detect_script_creation(&command, &output);

        let result_output = if let Some(script_path) = script_created {
            let mut full_output = output.clone();

            // Prompt user if they want to test the script
            if self.prompt_test_script(&script_path) {
                full_output.push_str(&format!("\n\n>> Testing script: {}\n", script_path));

                match tools
                    .execute_shell(&format!("./{}", script_path), &context.working_directory)
                    .await
                {
                    Ok(test_output) => {
                        full_output.push_str(&format!("SUCCESS - Script output:\n{}", test_output));
                        context.add_message(format!("Tested script: {}", script_path));
                    }
                    Err(e) => {
                        full_output.push_str(&format!("FAILED - Script test failed: {}", e));
                    }
                }
            } else {
                full_output.push_str(&format!(
                    "\n\nScript created: {}\n   Run with: ./{}",
                    script_path, script_path
                ));
            }

            full_output
        } else {
            output
        };

        Ok(AgentResult::success(result_output).with_metadata("command", command))
    }
}

impl ShellAgent {
    fn extract_command(&self, response: &str) -> String {
        // Look for COMMAND: pattern anywhere in the response
        for line in response.lines() {
            if let Some(cmd_pos) = line.find("COMMAND:") {
                // Extract everything after "COMMAND:" on this line
                let command = line[cmd_pos + 8..].trim();
                if !command.is_empty() {
                    return command.to_string();
                }
            }
        }

        // Fallback: filter out EXPLANATION lines and join
        response
            .lines()
            .filter(|line| !line.contains("EXPLANATION:"))
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }

    fn is_dangerous_command(&self, command: &str) -> bool {
        let dangerous_patterns = [
            "rm -rf /",
            "rm -rf /*",
            ":(){ :|:& };:",
            "mkfs",
            "dd if=/dev/zero",
            "> /dev/sda",
        ];

        dangerous_patterns
            .iter()
            .any(|pattern| command.contains(pattern))
    }

    fn detect_script_creation(&self, command: &str, _output: &str) -> Option<String> {
        // Detect if a script file was created (common patterns: *.sh, *.py, *.rb, etc.)
        // Look for patterns like: > script.sh, >> script.py, etc.
        let script_extensions = [".sh", ".py", ".rb", ".pl", ".js", ".ts"];

        for ext in &script_extensions {
            // Check for redirection to file: > file.ext or >> file.ext
            if let Some(pos) = command.find(&format!(">{}", ext)) {
                // Look backwards to find the filename
                let before = &command[..pos + ext.len()];
                if let Some(filename_start) = before.rfind(|c: char| c.is_whitespace()) {
                    let filename = before[filename_start..]
                        .trim()
                        .trim_start_matches('>')
                        .trim();
                    if filename.ends_with(ext) {
                        return Some(filename.to_string());
                    }
                }
            }

            // Also check for explicit redirects like: echo ... > file.sh
            for token in command.split_whitespace() {
                if token.ends_with(ext) && !token.starts_with('-') {
                    // Found a potential script filename
                    return Some(token.to_string());
                }
            }
        }

        None
    }

    fn prompt_test_script(&self, script_path: &str) -> bool {
        use std::io::{self, Write};

        println!("\n┌─────────────────────────────────────────────────────────────┐");
        println!("│ SCRIPT TEST PROMPT                                         │");
        println!("└─────────────────────────────────────────────────────────────┘");
        println!("  Script created: {}", script_path);
        println!("\n  Would you like to test this script now?");
        println!("    [y] Yes, run the script");
        println!("    [n] No, skip testing");

        loop {
            print!("\n  Your choice [y/n]: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                return false;
            }

            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => {
                    println!("  >> Running script...\n");
                    return true;
                }
                "n" | "no" => {
                    println!("  >> Skipping test\n");
                    return false;
                }
                _ => {
                    println!("  Invalid choice. Please enter y or n.");
                }
            }
        }
    }
}
