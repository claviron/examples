use clap::Parser;
use claviron::bench::BenchService;
use claviron::{Result, Server};
use demo_certs::DemoCertificates;

use crate::options::Options;
use tracing::error;

mod demo_certs;
mod options;

// Use mimalloc as the global allocator.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let options = Options::parse();
    let server = Server::with_options(options.into())
        .with_services::<(DemoCertificates, BenchService)>();

    if let Err(e) = server.serve().await {
        error!("failed to start services: {}", e);
        return Err(e);
    }

    Ok(())
}
