//! CLI argument definitions.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "smith")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Model to use (e.g., claude-sonnet-4-5, gpt-5.2)
    #[arg(short, long, global = true)]
    pub model: Option<String>,

    /// System prompt
    #[arg(short, long, global = true)]
    pub system: Option<String>,

    /// Maximum iterations for the agentic loop
    #[arg(long, default_value = "10", global = true)]
    pub max_iterations: usize,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Disable streaming output
    #[arg(long, global = true)]
    pub no_stream: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Load a previous session from file
    #[arg(long, global = true)]
    pub load_session: Option<PathBuf>,

    /// Save session to file on exit
    #[arg(long, global = true)]
    pub save_session: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigSubcommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigSubcommands {
    /// Initialize a new config file
    Init,
    /// Print config file location
    Where,
}
