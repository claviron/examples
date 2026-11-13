use lumenize::{Options, Runner};
mod app;
use app::routes::rust_interceptor;
use app::services::OrbitalCellService;

// Use mimalloc as the global allocator.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> lumenize::Result<()> {
    Runner::new()
        .with_options(Options::from_args()?)
        .with_services::<OrbitalCellService>()
        .with_interceptor(rust_interceptor)
        .run()
        .await
}
