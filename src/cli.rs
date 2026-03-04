use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "lint-unused",
    about = "Detect underscore-prefixed variable bindings that suppress unused warnings",
    version = env!("GIT_DESCRIBE"),
    after_help = "Logs are written to: ~/.local/share/lint-unused/logs/lint-unused.log"
)]
pub struct Cli {
    /// Paths to scan (default: from config or "src/")
    #[arg(help = "Paths to scan")]
    pub paths: Vec<PathBuf>,

    /// Path to config file
    #[arg(short, long, help = "Path to config file")]
    pub config: Option<PathBuf>,

    /// Enable verbose output (show allowed/filtered findings)
    #[arg(short, long, help = "Show allowed/filtered findings too")]
    pub verbose: bool,

    /// Only output finding count
    #[arg(short, long, help = "Only output finding count")]
    pub quiet: bool,

    /// Output format: human (default), json
    #[arg(long, default_value = "human", help = "Output format: human, json")]
    pub format: OutputFormat,

    /// Disable built-in drop-guard filters
    #[arg(long, help = "Disable built-in drop-guard filters")]
    pub no_default_filters: bool,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}
