use std::sync::Arc;

use lumenize::axum::extract::RawQuery;
use lumenize::axum::http::StatusCode;
use lumenize::axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use lumenize::axum::response::{IntoResponse, Response};
use lumenize::axum::{Extension, Router, routing::get};
use lumenize::claviron::http::ext::{ConnCtx, Provide};
use lumenize::claviron::runtime::AppState;

use super::services::{
    BURST_MAX, Fault, Hits, Tiles, burst, tiles_index, tiles_next_round,
    tiles_round,
};

/// Rust owns only the declared routes; the runner forwards the rest to Laravel.
pub fn rust_interceptor(state: AppState) -> Router {
    // Fixed for the process: read here, once in each process that builds the
    // router, never per request.
    let process = Process { pid: state.pid(), worker: state.worker_id() };
    Router::new()
        .route("/_lumenize/rust", get(hit))
        .route("/_lumenize/rust/value", get(value))
        .route("/_lumenize/rust/client", get(client))
        .route(
            "/_lumenize/rust/tiles",
            get(move |Provide(tiles): Provide<Tiles>| claim(tiles, process))
                .delete(next_round),
        )
        .route("/_lumenize/rust/tiles/id", get(tiles_id))
        .route(
            "/_lumenize/rust/burst",
            get(move |Provide(tiles): Provide<Tiles>, RawQuery(query)| {
                claim_burst(tiles, query, process)
            }),
        )
        .with_state(state)
}

/// Add one and return the count after it.
async fn hit(Provide(hits): Provide<Hits>) -> String {
    format!("hits={}\n", hits.fetch_add(1).wrapping_add(1))
}

async fn value(Provide(hits): Provide<Hits>) -> String {
    format!("hits={}\n", hits.load())
}

/// What the connection's TLS handshake verified of the client, in the words
/// `/_lumenize/php/client` reads from PHP's server variables. Blank without
/// TLS; `NONE` and no names when the client presented no certificate.
async fn client(connection: Option<Extension<Arc<ConnCtx>>>) -> String {
    let tls =
        connection.as_ref().and_then(|Extension(connection)| connection.tls());
    let certificate = tls.and_then(|tls| tls.client_cert.as_deref());
    format!(
        "verify={}\nsubject={}\nissuer={}\n",
        tls.map_or("", |tls| tls.ssl_client_verify()),
        certificate.map_or("", |certificate| &certificate.subject_dn),
        certificate.map_or("", |certificate| &certificate.issuer_dn),
    )
}

/// The process serving a claim, as the runtime recorded it at boot.
#[derive(Clone, Copy)]
struct Process {
    pid: u32,
    /// The supervisor's worker number; none in a standalone process.
    worker: Option<u8>,
}

thread_local! {
    /// The Tokio worker thread that runs a claim, numbered as the standard
    /// library numbers this process's threads; written once per thread.
    static THREAD: Box<str> = thread_label();
}

fn thread_label() -> Box<str> {
    let id = format!("{:?}", std::thread::current().id());
    let number = id
        .strip_prefix("ThreadId(")
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or(&id);
    format!("thread {number}").into_boxed_str()
}

/// Take the next tile with one add of 1: the tile and its round are the
/// value before the add. `/_lumenize/php/tiles` answers in the same shape.
async fn claim(
    tiles: Arc<Tiles>,
    process: Process,
) -> impl IntoResponse {
    let taken = tiles.fetch_add(1);
    let body = THREAD.with(|thread| {
        format!(
            "{{\"side\":\"rust\",\"round\":{},\"index\":{},\"pid\":{},\"worker\":{},\"execution\":\"{thread}\"}}\n",
            tiles_round(taken),
            tiles_index(taken),
            process.pid,
            JsonWorker(process.worker),
        )
    });
    json(body)
}

/// A worker number in JSON: the number, or `null` in a standalone process.
struct JsonWorker(Option<u8>);

impl std::fmt::Display for JsonWorker {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self.0 {
            Some(worker) => write!(formatter, "{worker}"),
            None => formatter.write_str("null"),
        }
    }
}

/// Start the next round with no tile taken, and return its number. A claim
/// that lands before the exchange belongs to the round it read; one after it
/// takes tile 0 of the new round. PHP's counter functions have no
/// compare-exchange, so the reset lives here.
async fn next_round(Provide(tiles): Provide<Tiles>) -> impl IntoResponse {
    let mut current = tiles.load();
    let round = loop {
        let next = tiles_next_round(current);
        match tiles.compare_exchange(current, next) {
            Ok(_) => break tiles_round(next),
            Err(actual) => current = actual,
        }
    };
    json(format!("{{\"round\":{round}}}\n"))
}

/// The id of the declared value, as text (`i64:<slot>:<epoch>`), for a
/// client to hand to PHP, which opens the same value by it. Formatted per
/// request: `Provide` resolves the value per request so that a replaced
/// provider is seen, and a client asks for the id once per run.
async fn tiles_id(Provide(tiles): Provide<Tiles>) -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/plain"), (CACHE_CONTROL, "no-store")],
        tiles.id().to_string(),
    )
}

/// Perform `n` claims in one loop on this thread, with no request per claim,
/// and answer what they took. `/_lumenize/php/burst` answers in the same
/// shape: `{"side","round","count","first","last","sum","pid","worker",
/// "execution","fault"}`, `sum` as a decimal string and `fault` null unless a
/// check failed.
async fn claim_burst(
    tiles: Arc<Tiles>,
    query: Option<String>,
    process: Process,
) -> Response {
    let count = match burst_size(query.as_deref()) {
        Ok(count) => count,
        Err(message) => {
            return (StatusCode::BAD_REQUEST, message).into_response();
        }
    };
    let taken = burst(&tiles, count);
    let body = THREAD.with(|thread| {
        format!(
            "{{\"side\":\"rust\",\"round\":{},\"count\":{},\"first\":{},\"last\":{},\"sum\":\"{}\",\"pid\":{},\"worker\":{},\"execution\":\"{thread}\",\"fault\":{}}}\n",
            taken.round,
            taken.count,
            taken.first,
            taken.last,
            taken.sum,
            process.pid,
            JsonWorker(process.worker),
            JsonFault(taken.fault.as_ref()),
        )
    });
    json(body).into_response()
}

/// The `n` of a burst's query: a whole number from 1 to [`BURST_MAX`].
fn burst_size(query: Option<&str>) -> Result<u32, String> {
    query
        .unwrap_or("")
        .split('&')
        .find_map(|pair| pair.strip_prefix("n="))
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|n| (1..=BURST_MAX).contains(n))
        .ok_or_else(|| {
            format!("n must be a whole number from 1 to {BURST_MAX}\n")
        })
}

/// A burst's failed check in JSON, or `null`.
struct JsonFault<'a>(Option<&'a Fault>);

impl std::fmt::Display for JsonFault<'_> {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let (check, previous, taken) = match self.0 {
            None => return formatter.write_str("null"),
            Some(Fault::Round { previous, taken }) => {
                ("round", previous, taken)
            }
            Some(Fault::Order { previous, taken }) => {
                ("order", previous, taken)
            }
        };
        write!(
            formatter,
            "{{\"check\":\"{check}\",\"previous\":{{\"round\":{},\"index\":{}}},\"taken\":{{\"round\":{},\"index\":{}}}}}",
            tiles_round(*previous),
            tiles_index(*previous),
            tiles_round(*taken),
            tiles_index(*taken),
        )
    }
}

fn json(body: String) -> impl IntoResponse {
    ([(CONTENT_TYPE, "application/json"), (CACHE_CONTROL, "no-store")], body)
}
