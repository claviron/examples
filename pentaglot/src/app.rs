mod native;

use std::sync::Arc;

use claviron::runtime::{AppState, registry};

pub fn register(state: &AppState) -> registry::Result<()> {
    claviron::edge::register_routable(
        state,
        Arc::new(native::Native::default()),
    )?;
    claviron_lua::register(state)?;
    claviron_lua::edge::register(state)?;
    claviron_deno::register(state)?;
    claviron_php::register(state)?;
    claviron_asgi::register(state)?;
    let hosts = state
        .registry_ref()
        .resolve::<claviron::edge::HostAppRegistry>()
        .ok_or_else(|| {
            registry::Error::msg("Pentaglot requires the Edge host registry")
        })?;
    claviron_php::backend::register(hosts.site())?;
    claviron_asgi::backend::register(hosts.site())
}
