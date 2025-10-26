use anyhow::Result;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::path::PathBuf;
use tracing::debug;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub name: String,
    pub root_path: String,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemory {
    pub session_id: String,
    pub timestamp: i64,
    pub messages: Vec<String>,
    pub results: Vec<String>,
}

pub struct ContextManager {
    #[allow(dead_code)]
    db: Db,
    project_context: Option<ProjectContext>,
}

#[allow(dead_code)]
impl ContextManager {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db = sled::open(db_path.into())?;
        Ok(Self {
            db,
            project_context: None,
        })
    }

    pub fn default() -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let db_path = PathBuf::from(home).join(".lca").join("context.db");
        Self::new(db_path)
    }

    pub async fn initialize_project(&mut self, root_path: impl Into<String>) -> Result<()> {
        let root_path = root_path.into();
        debug!("Initializing project context for: {}", root_path);

        let name = PathBuf::from(&root_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let language = self.detect_language(&root_path).await;
        let framework = self.detect_framework(&root_path).await;

        self.project_context = Some(ProjectContext {
            name,
            root_path: root_path.clone(),
            language,
            framework,
            metadata: std::collections::HashMap::new(),
        });

        self.save_project_context()?;

        Ok(())
    }

    pub async fn get_project_summary(&self) -> Result<String> {
        if let Some(ctx) = &self.project_context {
            let mut summary = format!("Project: {}\nPath: {}", ctx.name, ctx.root_path);

            if let Some(lang) = &ctx.language {
                summary.push_str(&format!("\nLanguage: {}", lang));
            }

            if let Some(fw) = &ctx.framework {
                summary.push_str(&format!("\nFramework: {}", fw));
            }

            Ok(summary)
        } else {
            Ok("No project context initialized".to_string())
        }
    }

    async fn detect_language(&self, root_path: &str) -> Option<String> {
        let indicators = vec![
            ("Cargo.toml", "Rust"),
            ("package.json", "JavaScript/TypeScript"),
            ("go.mod", "Go"),
            ("pom.xml", "Java"),
            ("setup.py", "Python"),
            ("requirements.txt", "Python"),
        ];

        for (file, lang) in indicators {
            let path = PathBuf::from(root_path).join(file);
            if tokio::fs::metadata(&path).await.is_ok() {
                return Some(lang.to_string());
            }
        }

        None
    }

    async fn detect_framework(&self, root_path: &str) -> Option<String> {
        if let Ok(content) =
            tokio::fs::read_to_string(PathBuf::from(root_path).join("package.json")).await
        {
            if content.contains("\"react\"") {
                return Some("React".to_string());
            } else if content.contains("\"vue\"") {
                return Some("Vue".to_string());
            } else if content.contains("\"next\"") {
                return Some("Next.js".to_string());
            }
        }

        None
    }

    fn save_project_context(&self) -> Result<()> {
        if let Some(ctx) = &self.project_context {
            let serialized = serde_json::to_vec(ctx)?;
            self.db.insert(b"project_context", serialized)?;
            self.db.flush()?;
        }
        Ok(())
    }

    pub fn load_project_context(&mut self) -> Result<()> {
        if let Some(data) = self.db.get(b"project_context")? {
            self.project_context = Some(serde_json::from_slice(&data)?);
        }
        Ok(())
    }

    pub fn save_session(&self, session: &SessionMemory) -> Result<()> {
        let key = format!("session:{}", session.session_id);
        let serialized = serde_json::to_vec(session)?;
        self.db.insert(key.as_bytes(), serialized)?;
        self.db.flush()?;
        Ok(())
    }

    pub fn load_session(&self, session_id: &str) -> Result<Option<SessionMemory>> {
        let key = format!("session:{}", session_id);
        if let Some(data) = self.db.get(key.as_bytes())? {
            Ok(Some(serde_json::from_slice(&data)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_sessions(&self) -> Result<Vec<String>> {
        let mut sessions = Vec::new();
        for (key, _) in self.db.scan_prefix(b"session:").flatten() {
            if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                if let Some(id) = key_str.strip_prefix("session:") {
                    sessions.push(id.to_string());
                }
            }
        }
        Ok(sessions)
    }

    pub fn set_metadata(&mut self, key: String, value: String) {
        if let Some(ctx) = &mut self.project_context {
            ctx.metadata.insert(key, value);
            let _ = self.save_project_context();
        }
    }

    pub fn get_metadata(&self, key: &str) -> Option<String> {
        self.project_context
            .as_ref()
            .and_then(|ctx| ctx.metadata.get(key).cloned())
    }
}
