#![deny(clippy::unwrap_used)]
#![deny(dead_code)]
#![deny(unused_variables)]

use clap::Parser;
use eyre::{Context, Result};
use log::info;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

mod cli;
mod config;

use cli::{Cli, OutputFormat};
use config::Config;
use lint_unused::filter;
use lint_unused::reporter;

fn setup_logging() -> Result<()> {
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lint-unused")
        .join("logs");

    fs::create_dir_all(&log_dir).context("Failed to create log directory")?;

    let log_file = log_dir.join("lint-unused.log");

    let target = Box::new(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .context("Failed to open log file")?,
    );

    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(target))
        .init();

    info!("Logging initialized, writing to: {}", log_file.display());
    Ok(())
}

fn run(cli: &Cli, config: &Config) -> Result<bool> {
    // Determine paths to scan
    let paths = if cli.paths.is_empty() { config.paths.clone() } else { cli.paths.clone() };

    // Run the linter
    let result = lint_unused::lint_files(&paths, &config.exclude_paths);

    // Print warnings to stderr
    for warning in &result.warnings {
        eprintln!("warning: {}", warning);
    }

    // Filter findings
    let include_defaults = !cli.no_default_filters;
    let allow_names = config.effective_allow_names(include_defaults);
    let filter_result = filter::filter_findings(result.findings, &allow_names, &config.allow_patterns);

    let mut stdout = std::io::stdout().lock();

    // Report based on format
    match cli.format {
        OutputFormat::Human => {
            if cli.quiet {
                reporter::report_quiet(&filter_result.reported, &mut stdout)?;
            } else {
                reporter::report_human(&filter_result.reported, &mut stdout)?;
                // Show verbose (allowed) findings
                if cli.verbose && !filter_result.allowed.is_empty() {
                    for (finding, reason) in &filter_result.allowed {
                        writeln!(
                            stdout,
                            "{}:{}:{}: [allowed] `{}` ({}) \u{2014} {}",
                            finding.file.display(),
                            finding.line,
                            finding.column,
                            finding.name,
                            finding.kind,
                            reason,
                        )?;
                    }
                    writeln!(stdout, "\n({} allowed)", filter_result.allowed.len())?;
                }
            }
        }
        OutputFormat::Json => reporter::report_json(&filter_result.reported, &mut stdout)?,
    }

    Ok(filter_result.reported.is_empty())
}

fn main() -> ExitCode {
    if let Err(e) = setup_logging() {
        eprintln!("Warning: Failed to setup logging: {}", e);
    }

    let cli = Cli::parse();

    let config = match Config::load(cli.config.as_ref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            return ExitCode::from(2);
        }
    };

    info!("Starting with config from: {:?}", cli.config);

    match run(&cli, &config) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(2)
        }
    }
}
