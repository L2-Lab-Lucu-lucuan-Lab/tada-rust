mod adapters;
mod application;
mod domain;
mod interfaces;

pub use adapters::{audio, config, quran_api, storage};
pub use interfaces::{app, cli, doctor, interactive, output, tui};

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::app::{AppContext, execute_command};
use crate::cli::Cli;
use crate::output::OutputMode;

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose, cli.debug);
    let mut ctx = AppContext::bootstrap(cli.data_dir.as_deref())?;
    ctx.output_mode =
        OutputMode::from_flags_and_config(cli.plain, cli.json, ctx.config.ui_output())?;
    ctx.color_enabled = !cli.no_color;
    ctx.assume_yes = cli.yes;

    if let Some(command) = cli.command {
        execute_command(&mut ctx, command)?;
    } else {
        execute_command(&mut ctx, crate::cli::Command::Tui)?;
    }

    Ok(())
}

fn init_tracing(verbose: u8, debug: bool) {
    let computed = if debug {
        "debug".to_string()
    } else if verbose >= 2 {
        "trace".to_string()
    } else if verbose >= 1 {
        "debug".to_string()
    } else {
        "info".to_string()
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_log_filter(&computed)));

    let _ = tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(filter)
        .try_init();
}

fn default_log_filter(level: &str) -> String {
    let crate_name = env!("CARGO_CRATE_NAME");
    format!("{crate_name}={level}")
}

#[cfg(test)]
mod tests {
    use super::default_log_filter;

    #[test]
    fn default_log_filter_only_enables_app_target() {
        assert_eq!(default_log_filter("info"), "tada_rust=info");
        assert_eq!(default_log_filter("debug"), "tada_rust=debug");
    }
}
