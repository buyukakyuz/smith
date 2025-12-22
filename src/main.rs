use smith::cli::{Cli, Commands, ConfigSubcommands};
use smith::config::AppConfig;
use smith::core::Result;
use smith::tui;

use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load();

    if let Some(command) = cli.command {
        return handle_command(command);
    }

    run_interactive(&cli, &config).await
}

fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Config { command } => match command {
            ConfigSubcommands::Init => {
                let path = AppConfig::init_default()?;
                println!("Created config file at {}", path.display());
            }
            ConfigSubcommands::Where => match AppConfig::get_config_path() {
                Some(path) => println!("{}", path.display()),
                None => eprintln!("Could not determine config path"),
            },
        },
    }
    Ok(())
}

async fn run_interactive(cli: &Cli, config: &AppConfig) -> Result<()> {
    let model_specified = cli.model.is_some() || config.model.is_some();
    let llm = smith::cli::create_provider(cli, config)?;
    let agent = smith::cli::create_agent(&llm, cli, config)?;

    tui::run_tui(agent, !model_specified).await
}
