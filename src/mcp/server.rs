use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, info};

use super::protocol::{McpRequest, McpResponse, Tool};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[allow(dead_code)]
pub struct McpServer {
    config: McpServerConfig,
    process: Option<Child>,
    tools: Vec<Tool>,
}

#[allow(dead_code)]
impl McpServer {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            process: None,
            tools: Vec::new(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting MCP server: {}", self.config.name);

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        let child = cmd.spawn()?;
        self.process = Some(child);

        self.discover_tools().await?;

        info!(
            "MCP server {} started with {} tools",
            self.config.name,
            self.tools.len()
        );

        Ok(())
    }

    async fn discover_tools(&mut self) -> Result<()> {
        let request = McpRequest::ListTools {};
        let response = self.send_request(&request).await?;

        if response.success {
            if let Some(result) = response.result {
                if let Ok(tools) = serde_json::from_value::<Vec<Tool>>(result) {
                    self.tools = tools;
                }
            }
        }

        Ok(())
    }

    pub async fn send_request(&mut self, request: &McpRequest) -> Result<McpResponse> {
        if let Some(process) = &mut self.process {
            let request_json = serde_json::to_string(request)?;
            debug!("Sending MCP request: {}", request_json);

            if let Some(stdin) = &mut process.stdin {
                stdin.write_all(request_json.as_bytes()).await?;
                stdin.write_all(b"\n").await?;
                stdin.flush().await?;
            }

            if let Some(stdout) = &mut process.stdout {
                let mut reader = BufReader::new(stdout);
                let mut response_line = String::new();
                reader.read_line(&mut response_line).await?;

                let response: McpResponse = serde_json::from_str(&response_line)?;
                debug!("Received MCP response: {:?}", response);

                return Ok(response);
            }
        }

        Ok(McpResponse::error("MCP server not running"))
    }

    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let request = McpRequest::CallTool {
            name: name.to_string(),
            arguments,
        };

        let response = self.send_request(&request).await?;

        if response.success {
            response
                .result
                .ok_or_else(|| anyhow::anyhow!("No result from tool call"))
        } else {
            Err(anyhow::anyhow!("Tool call failed: {:?}", response.error))
        }
    }

    pub fn get_tools(&self) -> &[Tool] {
        &self.tools
    }

    pub fn get_tool(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|t| t.name == name)
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            info!("Stopping MCP server: {}", self.config.name);
            process.kill().await?;
        }
        Ok(())
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        if let Some(process) = self.process.take() {
            if let Some(pid) = process.id() {
                let _ = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .spawn();
            }
        }
    }
}
