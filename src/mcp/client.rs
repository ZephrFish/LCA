use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::protocol::Tool;
use super::server::{McpServer, McpServerConfig};

#[allow(dead_code)]
pub struct McpClient {
    servers: Arc<RwLock<HashMap<String, McpServer>>>,
}

#[allow(dead_code)]
impl McpClient {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_server(&self, config: McpServerConfig) -> Result<()> {
        let name = config.name.clone();
        info!("Registering MCP server: {}", name);

        let mut server = McpServer::new(config);
        server.start().await?;

        let mut servers = self.servers.write().await;
        servers.insert(name, server);

        Ok(())
    }

    pub async fn list_all_tools(&self) -> Result<HashMap<String, Vec<Tool>>> {
        let servers = self.servers.read().await;
        let mut all_tools = HashMap::new();

        for (name, server) in servers.iter() {
            all_tools.insert(name.clone(), server.get_tools().to_vec());
        }

        Ok(all_tools)
    }

    pub async fn find_tool(&self, tool_name: &str) -> Result<Option<(String, Tool)>> {
        let servers = self.servers.read().await;

        for (server_name, server) in servers.iter() {
            if let Some(tool) = server.get_tool(tool_name) {
                return Ok(Some((server_name.clone(), tool.clone())));
            }
        }

        Ok(None)
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let (server_name, _tool) = self
            .find_tool(tool_name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found", tool_name))?;

        debug!("Calling tool '{}' on server '{}'", tool_name, server_name);

        let mut servers = self.servers.write().await;
        let server = servers
            .get_mut(&server_name)
            .ok_or_else(|| anyhow::anyhow!("Server '{}' not found", server_name))?;

        server.call_tool(tool_name, arguments).await
    }

    pub async fn stop_all(&self) -> Result<()> {
        let mut servers = self.servers.write().await;

        for (name, server) in servers.iter_mut() {
            info!("Stopping MCP server: {}", name);
            server.stop().await?;
        }

        servers.clear();
        Ok(())
    }

    pub async fn stop_server(&self, name: &str) -> Result<()> {
        let mut servers = self.servers.write().await;

        if let Some(server) = servers.get_mut(name) {
            server.stop().await?;
            servers.remove(name);
        }

        Ok(())
    }

    pub async fn get_server_count(&self) -> usize {
        let servers = self.servers.read().await;
        servers.len()
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let servers = self.servers.clone();
        tokio::spawn(async move {
            let mut servers = servers.write().await;
            for (_, server) in servers.iter_mut() {
                let _ = server.stop().await;
            }
        });
    }
}
