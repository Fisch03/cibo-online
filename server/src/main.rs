use axum::{
    extract::{
        connect_info::ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::{
    net::SocketAddr,
    sync::{LazyLock, Mutex},
};
use tokio::sync::mpsc;
use tower_http::{compression::CompressionLayer, services::ServeDir};

use cibo_online::{
    client::ClientMessage,
    server::{ServerGameState, ServerMessage},
};

static GAME_STATE: LazyLock<Mutex<ServerGameState<PerClientState>>> = LazyLock::new(|| {
    Mutex::new(ServerGameState::new(
        |client_state: &PerClientState, msg| {
            client_state.tx.try_send(msg).unwrap_or_else(|e| {
                eprintln!("error sending message to client: {:?}", e);
            });
        },
    ))
});

struct PerClientState {
    tx: mpsc::Sender<ServerMessage>,
}

#[tokio::main]
async fn main() {
    let app = Router::new();

    if let Err(_) = std::fs::read_dir("./static") {
        panic!("could not read static directory");
    }

    let serve_dir = ServeDir::new("./static").append_index_html_on_directories(true);
    let app = app
        .route("/ws", get(ws_handler))
        .fallback_service(serve_dir);

    let compression = CompressionLayer::new()
        .gzip(true)
        .zstd(true)
        .br(true)
        .deflate(true);
    let app = app.layer(compression);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(
                cibo_online::SERVER_TICK_RATE,
            ))
            .await;
            GAME_STATE.lock().unwrap().tick();
        }
    });

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

async fn handle_socket(socket: WebSocket, client_addr: SocketAddr) {
    let (mut socket_tx, mut socket_rx) = socket.split();
    let (client_tx, mut client_rx) = mpsc::channel(10);

    let client_id = GAME_STATE
        .lock()
        .unwrap()
        .new_client(PerClientState { tx: client_tx });

    println!("{} ({:?}): connected", client_addr, client_id);

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Binary(msg))) = socket_rx.next().await {
            let client_msg = match ClientMessage::from_bytes(&msg) {
                Ok(client_msg) => client_msg,
                Err(e) => {
                    eprintln!("error deserializing client message: {:?}", e);
                    continue;
                }
            };

            GAME_STATE.lock().unwrap().update(client_id, client_msg);
        }
    });

    let send_task = tokio::spawn(async move {
        while let Some(server_msg) = client_rx.recv().await {
            let server_msg_bytes = match server_msg.to_bytes() {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("error serializing server message: {:?}", e);
                    break;
                }
            };

            match socket_tx.send(Message::Binary(server_msg_bytes)).await {
                Ok(_) => (),
                Err(_) => {
                    break;
                }
            }
        }
    });

    tokio::select! {
        _ = recv_task => {
        }
        _ = send_task => {
        }
    }
    println!("{} ({:?}): disconnected", client_addr, client_id);
    GAME_STATE.lock().unwrap().remove_client(client_id);
}
