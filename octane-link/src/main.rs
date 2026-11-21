use lumenize::{Options, Runner};
mod app;
use crate::app::services::{HitsService, TilesService};
use app::routes::rust_interceptor;
use demo_certs::DemoCertificates;

// Use mimalloc as the global allocator.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> lumenize::Result<()> {
    Runner::new()
        .with_options(Options::from_args()?)
        // Opened once in every process at boot; handlers take them as
        // Provide<Hits> and Provide<Tiles>.
        .with_services::<(HitsService, TilesService)>()
        // Makes what the launch's `tls` section names before TLS reads it,
        // and a client certificate for trying the `application-mtls` door.
        .with_bootstrap(|state| {
            DemoCertificates::register_with_clients(state, &["client.octane.test"])
        })
        .with_interceptor(rust_interceptor)
        .run()
        .await
}
