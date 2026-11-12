use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use claviron::runner::{Runner, RunnerOptions};
use claviron::{FleetName, ServerName};

#[derive(Parser)]
#[command(about = "Serve an Axum application over Claviron Orbit Link", version)]
pub(crate) struct Options {
    /// The launch file. Its directory is the server's configuration directory.
    #[arg(long, default_value = "runner.yaml")]
    config: PathBuf,
    #[arg(long, default_value = env!("CARGO_PKG_NAME"), value_parser = ServerName::from_str)]
    name: ServerName,
    #[arg(long, visible_alias = "fleet-name", default_value = "axum", value_parser = FleetName::from_str)]
    fleet: FleetName,
    #[arg(long, visible_alias = "worker-count")]
    workers: Option<u8>,
    #[arg(long, conflicts_with = "production")]
    environment: Option<String>,
    #[arg(long)]
    production: bool,
}

impl From<Options> for Runner {
    fn from(options: Options) -> Self {
        Self::new()
            .with_config_path(options.config)
            .with_options(RunnerOptions {
                name: Some(options.name.to_string()),
                fleet_name: Some(options.fleet.to_string()),
                workers: options.workers,
                environment: options.environment,
                production: options.production,
                ..RunnerOptions::default()
            })
    }
}
