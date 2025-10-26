mod agents;
mod context;
mod llm;
mod mcp;
mod orchestrator;
mod permissions;
mod tools;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::{info, Level};

use llm::{LlmClient, LmStudioClient, OllamaClient};
use orchestrator::AgentSystem;
use permissions::{PermissionManager, PermissionMode};

#[derive(Parser)]
#[command(name = "lca")]
#[command(about = "Local agent system using LLMs", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, default_value = "ollama")]
    provider: String,

    #[arg(short, long, default_value = ".")]
    working_dir: String,

    #[arg(short, long)]
    verbose: bool,

    #[arg(
        long,
        help = "Allow all operations without prompting (USE WITH CAUTION)"
    )]
    allow_all: bool,
}

#[derive(Subcommand)]
enum Commands {
    Execute {
        task: String,
    },
    Init {
        #[arg(default_value = ".")]
        path: String,
    },
    Agent {
        name: String,
        task: String,
    },
    Interactive,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    use tracing_subscriber::EnvFilter;

    // Filter out noisy rustyline debug logs even in verbose mode
    let filter = if cli.verbose {
        EnvFilter::new("lca=debug,warn") // Our app DEBUG, others WARN
    } else {
        EnvFilter::new("lca=info,warn") // Our app INFO, others WARN
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_env_filter(filter)
        .init();

    let llm_client: Arc<dyn LlmClient> = match cli.provider.as_str() {
        "ollama" => Arc::new(OllamaClient::default()),
        "lmstudio" => Arc::new(LmStudioClient::default()),
        _ => {
            eprintln!("Unknown provider: {}. Using Ollama.", cli.provider);
            Arc::new(OllamaClient::default())
        }
    };

    let permission_mode = if cli.allow_all {
        info!("WARNING: Running with --allow-all flag (blanket permissions enabled)");
        PermissionMode::AllowAll
    } else {
        PermissionMode::Ask
    };

    let permission_manager = Arc::new(PermissionManager::new(permission_mode));
    let system = AgentSystem::new(llm_client, &cli.working_dir, permission_manager)?;

    match cli.command {
        Commands::Execute { task } => {
            info!("Executing task: {}", task);
            let result = system.execute_task(&task).await?;

            if result.success {
                println!("\nSUCCESS\n{}", result.output);
            } else {
                eprintln!("\nFAILED\n{}", result.output);
            }
        }
        Commands::Init { path } => {
            info!("Initializing project at: {}", path);
            system.initialize_project(&path).await?;
            println!("Project initialized successfully!");
        }
        Commands::Agent { name, task } => {
            info!("Executing task with {} agent: {}", name, task);

            let agent = system
                .get_agent(&name)
                .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", name))?;

            let mut context = agents::AgentContext::new(&cli.working_dir);
            let result = agent
                .execute(
                    &task,
                    &mut context,
                    Arc::clone(&system.llm_client),
                    Arc::clone(&system.tool_executor),
                    Arc::clone(&system.context_manager),
                )
                .await?;

            if result.success {
                println!("\nSUCCESS\n{}", result.output);
            } else {
                eprintln!("\nFAILED\n{}", result.output);
            }
        }
        Commands::Interactive => {
            use rustyline::error::ReadlineError;
            use rustyline::DefaultEditor;

            println!("Interactive mode - type 'exit' to quit");
            println!("Use arrow keys to navigate history, Ctrl+C or Ctrl+D to exit");

            let mut rl = DefaultEditor::new()?;

            // Load history from file if it exists
            let history_path = std::env::var("HOME")
                .map(|h| format!("{}/.lca/history.txt", h))
                .unwrap_or_else(|_| ".lca-history.txt".to_string());

            let _ = rl.load_history(&history_path);

            loop {
                let readline = rl.readline("\n> ");

                match readline {
                    Ok(line) => {
                        let task = line.trim();

                        if task.is_empty() {
                            continue;
                        }

                        if task == "exit" || task == "quit" {
                            println!("Goodbye!");
                            break;
                        }

                        // Add to history
                        let _ = rl.add_history_entry(task);

                        match system.execute_task(task).await {
                            Ok(result) => {
                                info!(
                                    "Task result - Success: {}, Output length: {}",
                                    result.success,
                                    result.output.len()
                                );
                                if result.success {
                                    println!("\n{}", result.output);
                                } else {
                                    eprintln!("\nError: {}", result.output);
                                }
                            }
                            Err(e) => {
                                eprintln!("\nFailed to execute task: {}", e);
                            }
                        }
                    }
                    Err(ReadlineError::Interrupted) => {
                        println!("\nGoodbye!");
                        break;
                    }
                    Err(ReadlineError::Eof) => {
                        println!("\nGoodbye!");
                        break;
                    }
                    Err(err) => {
                        eprintln!("Error reading input: {}", err);
                        break;
                    }
                }
            }

            // Save history on exit
            let _ = rl.save_history(&history_path);
        }
    }

    Ok(())
}
