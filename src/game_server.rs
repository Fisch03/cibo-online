use crate::admin_panel::{log_admin_message, AdminAction, BannedWord};
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
use cibo_online::{
    client::ClientMessage,
    server::{ServerGameState, ServerMessage, SpecialEvent},
    ClientId,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock, Mutex,
    },
};
use tokio::sync::mpsc;
use tower_http::{compression::CompressionLayer, services::ServeDir};
use tracing::{error, info, instrument, span, warn, Instrument, Span};

static CONNECTED_IPS: LazyLock<Mutex<HashMap<IpAddr, ClientId>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static GAME_STATE: LazyLock<Mutex<ServerGameState<PerClientState>>> = LazyLock::new(|| {
    Mutex::new(ServerGameState::new(
        |client_state: &PerClientState, msg| {
            client_state.tx.send(msg).unwrap_or_else(|e| {
                error!("sending message to client: {:?}", e);
            });
        },
    ))
});

struct PerClientState {
    tx: mpsc::UnboundedSender<ServerMessage>,
}

static BANNED_IPS: LazyLock<Mutex<HashSet<IpAddr>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static BANNED_WORDS: LazyLock<Mutex<HashMap<String, BannedWord>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static STREAM_MODE: AtomicBool = AtomicBool::new(false);
pub fn get_stream_mode() -> bool {
    STREAM_MODE.load(Ordering::Relaxed)
}
pub fn set_stream_mode(stream_mode: bool) {
    info!(
        "stream mode {}!",
        if stream_mode { "enabled" } else { "disabled" }
    );

    STREAM_MODE.store(stream_mode, Ordering::Relaxed);
}

pub fn get_special_event(event: SpecialEvent) -> bool {
    GAME_STATE.lock().unwrap().get_special_event(event)
}
pub fn set_special_event(event: SpecialEvent, active: bool) {
    GAME_STATE.lock().unwrap().set_special_event(event, active);
    info!(
        "special event {:?} {}!",
        event,
        if active { "enabled" } else { "disabled" }
    );
}

#[instrument(name = "game", skip(admin_rx))]
pub async fn run(mut admin_rx: mpsc::Receiver<AdminAction>) {
    let app = Router::new();

    let serve_game_dir = ServeDir::new("./static/game").append_index_html_on_directories(true);
    let serve_shared_dir = ServeDir::new("./static/shared");

    let app = app
        .route("/ws", get(ws_handler))
        .nest_service("/shared", serve_shared_dir)
        .fallback_service(serve_game_dir);

    let compression = CompressionLayer::new()
        .gzip(true)
        .zstd(true)
        .br(true)
        .deflate(true);
    let app = app.layer(compression);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(
            cibo_online::SERVER_TICK_RATE,
        ));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            GAME_STATE
                .lock()
                .unwrap()
                .tick(cibo_online::SERVER_TICK_RATE as u64);
        }
    });

    tokio::spawn(async move {
        while let Some(action) = admin_rx.recv().await {
            match action {
                AdminAction::BanIp(ip) => {
                    let mut banned_ips = BANNED_IPS.lock().unwrap();
                    let mut connected_ips = CONNECTED_IPS.lock().unwrap();
                    if let Some(client_id) = connected_ips.remove(&ip) {
                        GAME_STATE.lock().unwrap().remove_client(client_id);
                    }
                    banned_ips.insert(ip);
                }
                AdminAction::UnbanIp(ip) => {
                    let mut banned_ips = BANNED_IPS.lock().unwrap();
                    banned_ips.remove(&ip);
                }

                AdminAction::BanWord(word) => {
                    let mut banned_words = BANNED_WORDS.lock().unwrap();
                    banned_words.insert(word.word.clone(), word);
                }
                AdminAction::UnbanWord(word) => {
                    let mut banned_words = BANNED_WORDS.lock().unwrap();
                    banned_words.remove(&word);
                }
            }
        }
    });

    set_special_event(SpecialEvent::BeachEpisode, true);

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
    remote_client_ip: Option<IpAddr>,
) {
    let (client_tx, client_rx) = mpsc::unbounded_channel();
    let client_ip = remote_client_ip.unwrap_or(client_addr.ip());

    let span = span!(tracing::Level::INFO, "client", id=client_id.as_u32(), ip = %client_ip, name = tracing::field::Empty);

    async move {
        info!("connected");

        GAME_STATE
            .lock()
            .unwrap()
            .new_client(client_id, PerClientState { tx: client_tx });
        handle_client_inner(client_id, socket, client_rx, remote_client_ip, client_ip).await;

        info!("disconnected");
    }
    .instrument(span)
    .await;

    GAME_STATE.lock().unwrap().remove_client(client_id);
    if let Some(remote_client_ip) = remote_client_ip {
        CONNECTED_IPS.lock().unwrap().remove(&remote_client_ip);
    }
}

async fn handle_client_inner(
    client_id: ClientId,
    socket: WebSocket,
    mut client_rx: mpsc::UnboundedReceiver<ServerMessage>,
    remote_client_ip: Option<IpAddr>,
    client_ip: IpAddr,
) {
    let (mut socket_tx, mut socket_rx) = socket.split();
    let mut client_name = None;

    let mut connected = false;

    let recv_task = tokio::spawn(
        async move {
            while let Some(Ok(Message::Binary(msg))) = socket_rx.next().await {
                if let Some(remote_client_ip) = remote_client_ip {
                    if CONNECTED_IPS
                        .lock()
                        .unwrap()
                        .get(&remote_client_ip)
                        .is_none()
                    {
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
                        let display_name = if name.is_empty() {
                            "Anon".to_string()
                        } else {
                            name.clone()
                        };
                        Span::current().record("name", &display_name);
                        client_name = Some(display_name);

                        info!("fully connected");
                        let stream_mode = STREAM_MODE.load(Ordering::Relaxed);
                        if banned_words.values().any(|word| {
                            if name_lower.contains(&word.word) {
                                // allow light bans outside of stream mode
                                if !stream_mode && !word.full_ban {
                                    return false;
                                }
                                return true;
                            }
                            false
                        }) {
                            warn!("tried to connect with banned name");
                            *name = "*****".to_string();
                        }
                        name.truncate(cibo_online::NAME_LIMIT);
                        *name = name.trim().to_string();
                        connected = true;
                    }
                    ClientMessage::Chat(ref mut msg) => {
                        info!("says '{}'", msg);

                        let banned_words = BANNED_WORDS.lock().unwrap();

                        let msg_lower = msg.to_lowercase();
                        let stream_mode = STREAM_MODE.load(Ordering::Relaxed);

                        let contains_banned = banned_words.values().any(|word| {
                            if msg_lower.contains(&word.word) {
                                // allow light bans outside of stream mode
                                if !word.full_ban && !stream_mode {
                                    return false;
                                }
                                return true;
                            }
                            false
                        });

                        log_admin_message(
                            &msg,
                            client_name.as_ref().map_or("UNKNOWN", |name| name.as_str()),
                            client_ip,
                            contains_banned,
                        );
                        if contains_banned {
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
