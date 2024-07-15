use axum::{
    extract::{
        connect_info::ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    sync::{LazyLock, Mutex},
    time::Duration,
};
use tokio::sync::mpsc;
use tower_http::{compression::CompressionLayer, services::ServeDir};

use cibo_online::{
    client::ClientMessage,
    server::{ServerGameState, ServerMessage},
    ClientId,
};

static CONNECTED_IPS: LazyLock<Mutex<HashMap<IpAddr, ClientId>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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

static BANNED_IPS: LazyLock<Mutex<HashSet<IpAddr>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
fn read_banned_ips() {
    let banned_ips = match std::fs::read_to_string("../data/banned_ips.txt") {
        Ok(banned_ips) => banned_ips,
        Err(e) => {
            eprintln!("error reading banned_ips.txt: {:?}", e);
            return;
        }
    };
    let banned_ips = banned_ips
        .lines()
        .filter_map(|line| {
            let res = line.parse().ok();
            if res.is_none() {
                eprintln!("invalid IP in banned_ips.txt: {:?}", line);
            }

            res
        })
        .collect();

    println!("loaded banned IPs: {:?}", banned_ips);
    let connected_ips = CONNECTED_IPS.lock().unwrap();
    for ip in &banned_ips {
        if let Some(client_id) = connected_ips.get(ip) {
            GAME_STATE.lock().unwrap().remove_client(*client_id);
        }
    }

    *BANNED_IPS.lock().unwrap() = banned_ips
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

    read_banned_ips();
    let mut banned_ips_watcher = new_debouncer(
        Duration::from_secs(1),
        |res: DebounceEventResult| match res {
            Ok(events) => events.iter().for_each(|_| {
                read_banned_ips();
            }),
            Err(e) => eprintln!("error watching banned_ips.txt: {:?}", e),
        },
    )
    .unwrap();

    banned_ips_watcher
        .watcher()
        .watch(
            Path::new("../data/banned_ips.txt"),
            RecursiveMode::NonRecursive,
        )
        .unwrap();

    // read_banned_ips();

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
    headers: HeaderMap,
) -> impl IntoResponse {
    let client_id = ClientId::new();

    if BANNED_IPS.lock().unwrap().contains(&addr.ip()) {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body("you are banned".into())
            .unwrap();
    }

    let actual_ip = if let Some(ip) = headers.get("x-real-ip") {
        let ip = match ip.to_str() {
            Ok(ip) => ip,
            Err(_) => {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("invalid x-real-ip header".into())
                    .unwrap();
            }
        };

        let ip = match ip.parse() {
            Ok(ip) => ip,
            Err(_) => {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("invalid x-real-ip header".into())
                    .unwrap();
            }
        };

        if BANNED_IPS.lock().unwrap().contains(&ip) {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body("you are banned".into())
                .unwrap();
        }

        let mut connected_ips = CONNECTED_IPS.lock().unwrap();
        if connected_ips.insert(ip, client_id).is_some() {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body("only one connection per IP allowed".into())
                .unwrap();
        }

        Some(ip)
    } else if addr.ip() == IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)) {
        // allow connections from localhost without x-real-ip header
        None
    } else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("missing x-real-ip header. this is likely an issue with the server, please notify the administrator".into())
            .unwrap();
    };

    ws.on_upgrade(move |socket| handle_socket(socket, client_id, addr, actual_ip))
}

async fn handle_socket(
    socket: WebSocket,
    client_id: ClientId,
    client_addr: SocketAddr,
    client_ip: Option<IpAddr>,
) {
    let (mut socket_tx, mut socket_rx) = socket.split();
    let (client_tx, mut client_rx) = mpsc::channel(10);

    let client_id = GAME_STATE
        .lock()
        .unwrap()
        .new_client(client_id, PerClientState { tx: client_tx });

    println!(
        "{} ({:?}): connected",
        client_ip.unwrap_or(client_addr.ip()),
        client_id
    );

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Binary(msg))) = socket_rx.next().await {
            let client_msg = match ClientMessage::from_bytes(&msg) {
                Ok(client_msg) => client_msg,
                Err(e) => {
                    eprintln!("error deserializing client message: {:?}", e);
                    continue;
                }
            };

            match client_msg {
                ClientMessage::Chat(ref msg) => {
                    println!(
                        "{:?} ({:?}) says '{}'",
                        client_ip.unwrap_or(client_addr.ip()),
                        client_id,
                        msg
                    );
                }
                _ => (),
            }

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
    println!(
        "{:?} ({:?}): disconnected",
        client_ip.unwrap_or(client_addr.ip()),
        client_id
    );

    GAME_STATE.lock().unwrap().remove_client(client_id);
    if let Some(client_ip) = client_ip {
        CONNECTED_IPS.lock().unwrap().remove(&client_ip);
    }
}
