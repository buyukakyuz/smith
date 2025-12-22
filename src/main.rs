use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};

use smith::config;
use smith::core;
use smith::providers;
use smith::tools;
use smith::tui;

use config::AppConfig;
use core::prompt::{PromptBuilder, TemplateType};
use core::{AgentError, AugmentedLLM, LLM, LoopConfig, Result};
use providers::{AnthropicProvider, ApiKey, OpenAIClient};
use tools::ToolEventEmitter;

#[derive(Parser, Debug)]
#[command(name = "smith")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Model to use (e.g., claude-sonnet-4-5, gpt-5.2)
    #[arg(short, long, global = true)]
    model: Option<String>,

    /// System prompt
    #[arg(short, long, global = true)]
    system: Option<String>,

    /// Maximum iterations for the agentic loop
    #[arg(long, default_value = "10", global = true)]
    max_iterations: usize,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Disable streaming output
    #[arg(long, global = true)]
    no_stream: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Load a previous session from file
    #[arg(long, global = true)]
    load_session: Option<PathBuf>,

    /// Save session to file on exit
    #[arg(long, global = true)]
    save_session: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigSubcommands,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigSubcommands {
    Init,
    Where,
}

fn infer_provider(model: &str) -> Option<&'static str> {
    if model.starts_with("claude") {
        Some("anthropic")
    } else if model.starts_with("gpt") || model.starts_with("o1") || model.starts_with("o3") {
        Some("openai")
    } else {
        None
    }
}

fn create_provider(cli: &Cli, config: &AppConfig) -> Result<Arc<dyn LLM>> {
    let model = cli.model.as_deref().or(config.model.as_deref());
    let provider = model.and_then(infer_provider).unwrap_or("anthropic");

    match provider {
        "openai" => {
            let mut client = OpenAIClient::from_env()?;
            if let Some(m) = model {
                client = client.with_model(m);
            }
            Ok(Arc::new(client))
        }
        "anthropic" => {
            let api_key = ApiKey::from_env("ANTHROPIC_API_KEY")
                .map_err(|e| AgentError::Config(e.to_string()))?;
            let mut provider =
                AnthropicProvider::new(api_key).map_err(|e| AgentError::Config(e.to_string()))?;
            if let Some(m) = model {
                provider = provider.with_model(m);
            }
            Ok(Arc::new(provider))
        }
        other => Err(AgentError::Config(format!(
            "Unknown provider: {other}. Use a model starting with 'claude', 'gpt'"
        ))),
    }
}
fn create_agent(llm: &Arc<dyn LLM>, cli: &Cli, config: &AppConfig) -> Result<AugmentedLLM> {
    let loop_config = LoopConfig {
        max_iterations: cli.max_iterations,
        ..Default::default()
    };

    let event_emitter = ToolEventEmitter::new();

    let mut agent = AugmentedLLM::with_config(llm.clone(), loop_config, event_emitter)?;
    let system_prompt = cli.system.as_ref().map_or_else(
        || {
            let name_lower = llm.name().to_lowercase();
            let template_type = if name_lower.contains("gpt") || name_lower.contains("openai") {
                TemplateType::OpenAI
            } else {
                TemplateType::Claude
            };

            let builder = PromptBuilder::new()
                .with_template(template_type)
                .with_model(llm.name(), llm.model());

            let mut base_prompt = builder.build(agent.tools());

            if let Some(custom) = config
                .custom_system_prompt
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                base_prompt = format!("{base_prompt}\n\n# Custom Instructions\n\n{custom}");
            }

            base_prompt
        },
        Clone::clone,
    );
    agent.set_system_prompt(&system_prompt);

    agent
        .tools_mut()
        .register(Arc::new(tools::ReadFileTool::new()));
    agent
        .tools_mut()
        .register(Arc::new(tools::WriteFileTool::new()));
    agent
        .tools_mut()
        .register(Arc::new(tools::UpdateFileTool::new()));
    agent
        .tools_mut()
        .register(Arc::new(tools::ListDirTool::new()));
    agent.tools_mut().register(Arc::new(tools::GlobTool::new()));
    agent.tools_mut().register(Arc::new(tools::GrepTool::new()));
    agent.tools_mut().register(Arc::new(tools::BashTool::new()));

    if let Err(e) = agent.enable_permissions() {
        eprintln!("Warning: Failed to enable permissions: {e}");
        eprintln!("Continuing without permission system.");
    }

    Ok(agent)
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let _ = cli.no_color;
    let config = AppConfig::load();

    if let Some(Commands::Config { command }) = &cli.command {
        match command {
            ConfigSubcommands::Init => match AppConfig::init_default() {
                Ok(path) => {
                    println!("✓ Created config file at {}", path.display());
                }
                Err(e) => {
                    eprintln!("✗ Failed to create config: {e}");
                }
            },
            ConfigSubcommands::Where => match AppConfig::get_config_path() {
                Some(path) => println!("{}", path.display()),
                None => eprintln!("✗ Could not determine config path"),
            },
        }
        return Ok(());
    }

    let model_specified = cli.model.is_some() || config.model.is_some();

    let llm = create_provider(&cli, &config)?;
    let agent = create_agent(&llm, &cli, &config)?;

    match &cli.command {
        None => {
            tui::run_tui(agent, !model_specified).await?;
        }
        Some(Commands::Config { .. }) => unreachable!(),
    }

    Ok(())
}
