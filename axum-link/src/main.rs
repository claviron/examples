mod options;

use clap::Parser;
use claviron::AppState;
use claviron::axum::Router;
use claviron::axum::response::Response;
use claviron::axum::routing::{any, get};
use claviron_runner::Runner;
use claviron_runner::websocket::{Message, WebSocket, WebSocketOffer};
use options::Options;

fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(|| async { "Axum application on Claviron\n" }))
        .route("/ws", any(socket))
        .merge(claviron::bench::routes_with_state(&state))
}

/// A WebSocket, on the runner's own sockets and over Orbit from an Edge alike.
async fn socket(offer: WebSocketOffer) -> Response {
    offer.accept(echo)
}

async fn echo(mut socket: WebSocket) {
    while let Some(message) = socket.recv().await {
        if matches!(message, Message::Close { .. }) || socket.send(message).await.is_err() {
            return;
        }
    }
}

#[tokio::main]
async fn main() -> claviron::Result<()> {
    let options = Options::parse();
    Runner::from(options)
        .with_router(router)
        .run()
        .await
}
