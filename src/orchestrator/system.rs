use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use crate::agents::{
    Agent, AgentContext, AgentRegistry, AgentResult, AnalysisAgent, CodeAgent, CoordinatorAgent,
    FileAgent, ShellAgent,
};
use crate::context::ContextManager;
use crate::llm::LlmClient;
use crate::permissions::PermissionManager;
use crate::tools::ToolExecutor;

pub struct AgentSystem {
    coordinator: Arc<CoordinatorAgent>,
    registry: Arc<AgentRegistry>,
    pub llm_client: Arc<dyn LlmClient>,
    pub tool_executor: Arc<ToolExecutor>,
    pub context_manager: Arc<ContextManager>,
    #[allow(dead_code)]
    pub permission_manager: Arc<PermissionManager>,
}

impl AgentSystem {
    pub fn new(
        llm_client: Arc<dyn LlmClient>,
        working_directory: impl Into<String>,
        permission_manager: Arc<PermissionManager>,
    ) -> Result<Self> {
        let working_dir = working_directory.into();

        let mut registry = AgentRegistry::new();

        registry.register(Arc::new(CodeAgent::new()));
        registry.register(Arc::new(ShellAgent::new()));
        registry.register(Arc::new(FileAgent::new()));
        registry.register(Arc::new(AnalysisAgent::new()));

        let registry = Arc::new(registry);
        let coordinator = Arc::new(CoordinatorAgent::new(registry.clone()));

        let tool_executor =
            Arc::new(ToolExecutor::new(working_dir).with_permissions(permission_manager.clone()));

        let context_manager = Arc::new(ContextManager::default()?);

        Ok(Self {
            coordinator,
            registry,
            llm_client,
            tool_executor,
            context_manager,
            permission_manager,
        })
    }

    pub async fn execute_task(&self, task: &str) -> Result<AgentResult> {
        info!("Executing task: {}", task);

        let capable_agents = self.registry.find_capable(task);

        if capable_agents.len() == 1 {
            let agent = &capable_agents[0];
            info!("Routing to single capable agent: {}", agent.name());

            let mut context = AgentContext::new(".");
            agent
                .execute(
                    task,
                    &mut context,
                    self.llm_client.clone(),
                    self.tool_executor.clone(),
                    self.context_manager.clone(),
                )
                .await
        } else {
            info!("Using coordinator for multi-agent orchestration");

            let mut context = AgentContext::new(".");
            self.coordinator
                .execute(
                    task,
                    &mut context,
                    self.llm_client.clone(),
                    self.tool_executor.clone(),
                    self.context_manager.clone(),
                )
                .await
        }
    }

    pub async fn initialize_project(&self, _root_path: &str) -> Result<()> {
        Ok(())
    }

    pub fn get_agent(&self, name: &str) -> Option<Arc<dyn Agent>> {
        self.registry.get(name)
    }
}
