mod native;

use claviron::runtime::{AppState, Register, registry};
use native::Native;

pub fn register(state: &AppState) -> registry::Result<()> {
    Native::register(state)?;
    claviron_lua::register(state)?;
    claviron_deno::register(state)?;
    claviron_php::register(state)?;
    claviron_asgi::register(state)
}
