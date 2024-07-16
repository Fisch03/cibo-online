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
use tracing::{error, info, span, warn, Instrument, Span};

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
                error!("sending message to client: {:?}", e);
            });
        },
    ))
});

struct PerClientState {
    tx: mpsc::Sender<ServerMessage>,
}

static BANNED_IPS: LazyLock<Mutex<HashSet<IpAddr>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static BANNED_WORDS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

fn read_banned_ips(path: &Path) {
    let banned_ips = match std::fs::read_to_string(path) {
        Ok(banned_ips) => banned_ips,
        Err(e) => {
            error!("reading banned_ips.txt: {:?}", e);
            return;
        }
    };
    let banned_ips = banned_ips
        .lines()
        .filter_map(|line| {
            let res = line.parse().ok();
            if res.is_none() {
                warn!("invalid IP in banned_ips.txt: {:?}", line);
            }

            res
        })
        .collect::<HashSet<_>>();

    info!("loaded {} banned IPs", banned_ips.len());

    let connected_ips = CONNECTED_IPS.lock().unwrap();
    for ip in &banned_ips {
        if let Some(client_id) = connected_ips.get(ip) {
            info!("kicking client with banned IP: {:?}", ip);
            GAME_STATE.lock().unwrap().remove_client(*client_id);
            CONNECTED_IPS.lock().unwrap().remove(ip);
        }
    }

    *BANNED_IPS.lock().unwrap() = banned_ips
}

fn read_banned_words(path: &Path) {
    let banned_words = match std::fs::read_to_string(path) {
        Ok(banned_words) => banned_words,
        Err(e) => {
            error!("reading banned_words.txt: {:?}", e);
            return;
        }
    };
    let banned_words = banned_words
        .lines()
        .map(|line| line.to_lowercase())
        .collect::<HashSet<_>>();
    info!("loaded {} banned words", banned_words.len());
    *BANNED_WORDS.lock().unwrap() = banned_words;
}

fn watch_config_file<P: AsRef<Path>, F: Fn(&Path) + Sync + Send + 'static>(path: P, handler: F) {
    let path = path.as_ref().to_path_buf();

    handler(&path);

    let mut watcher = {
        let path = path.clone();

        new_debouncer(
            Duration::from_secs(1),
            move |res: DebounceEventResult| match res {
                Ok(events) => events.iter().for_each(|_| {
                    handler(&path);
                }),
                Err(e) => error!("watching banned_words.txt: {:?}", e),
            },
        )
        .unwrap()
    };

    watcher
        .watcher()
        .watch(&path, RecursiveMode::NonRecursive)
        .unwrap();
}

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt().with_target(false).finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

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

    watch_config_file("../data/banned_ips.txt", read_banned_ips);
    watch_config_file("../data/banned_words.txt", read_banned_words);

    info!(
        "ready! listening on port {}",
        listener.local_addr().unwrap().port()
    );
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

    ws.on_upgrade(move |socket| handle_client(socket, client_id, addr, actual_ip))
}

async fn handle_client(
    socket: WebSocket,
    client_id: ClientId,
    client_addr: SocketAddr,
    client_ip: Option<IpAddr>,
) {
    let (client_tx, client_rx) = mpsc::channel(10);
    let display_ip = client_ip.unwrap_or(client_addr.ip());

    let span = span!(tracing::Level::INFO, "client", id=client_id.as_u32(), ip = %display_ip, name = tracing::field::Empty);

    async move {
        info!("connected");

        GAME_STATE
            .lock()
            .unwrap()
            .new_client(client_id, PerClientState { tx: client_tx });
        handle_client_inner(client_id, socket, client_rx, client_ip).await;

        info!("client disconnected");
    }
    .instrument(span)
    .await;

    GAME_STATE.lock().unwrap().remove_client(client_id);
    if let Some(client_ip) = client_ip {
        CONNECTED_IPS.lock().unwrap().remove(&client_ip);
    }
}

async fn handle_client_inner(
    client_id: ClientId,
    socket: WebSocket,
    mut client_rx: mpsc::Receiver<ServerMessage>,
    client_ip: Option<IpAddr>,
) {
    let (mut socket_tx, mut socket_rx) = socket.split();

    let mut connected = false;

    let recv_task = tokio::spawn(
        async move {
            while let Some(Ok(Message::Binary(msg))) = socket_rx.next().await {
                if let Some(client_ip) = client_ip {
                    if CONNECTED_IPS.lock().unwrap().get(&client_ip).is_none() {
                        warn!("received message from disconnected client");
                        break;
                    }
                }

                let mut client_msg = match ClientMessage::from_bytes(&msg) {
                    Ok(client_msg) => client_msg,
                    Err(e) => {
                        error!("deserializing message: {:?}", e);
                        continue;
                    }
                };

                if !matches!(client_msg, ClientMessage::Connect { .. }) && !connected {
                    warn!("sent message before connecting");
                    continue;
                }

                match client_msg {
                    ClientMessage::Connect { ref mut name } => {
                        if connected {
                            warn!("tried to connect twice");
                            continue;
                        }

                        let banned_words = BANNED_WORDS.lock().unwrap();
                        let name_lower = name.to_lowercase();
                        Span::current().record("name", &name.as_str());
                        if banned_words.iter().any(|word| name_lower.contains(word)) {
                            warn!("tried to connect with banned name");
                            *name = "*****".to_string();
                        }
                        connected = true;
                    }
                    ClientMessage::Chat(ref mut msg) => {
                        info!("says '{}'", msg);

                        let banned_words = BANNED_WORDS.lock().unwrap();
                        let msg_lower = msg.to_lowercase();
                        if banned_words.iter().any(|word| msg_lower.contains(word)) {
                            client_msg = ClientMessage::Chat("*****".to_string());
                            warn!("tried to send banned word");
                        }
                    }
                    _ => (),
                }

                GAME_STATE.lock().unwrap().update(client_id, client_msg);
            }
        }
        .in_current_span(),
    );

    let send_task = tokio::spawn(
        async move {
            while let Some(server_msg) = client_rx.recv().await {
                let server_msg_bytes = match server_msg.to_bytes() {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        error!("serializing message: {:?}", e);
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
        }
        .in_current_span(),
    );

    tokio::select! {
        _ = recv_task => (),
        _ = send_task => ()
    }
}
