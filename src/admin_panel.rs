mod login;

use crate::{db::db, game_server};
use axum::{
    body::Body,
    extract::{Form, Path},
    http, middleware,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Extension, Router,
};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use cibo_online::server::SpecialEvent;
use maud::{html, Markup};
use serde::Deserialize;
use sqlx::FromRow;
use std::{
    collections::VecDeque,
    net::IpAddr,
    sync::{LazyLock, Mutex},
};
use tokio::sync::mpsc::Sender;
use tower_http::{compression::CompressionLayer, services::ServeDir};
use tracing::{error, info, instrument};

static ADMIN_CHAT_LOG: LazyLock<Mutex<VecDeque<AdminChatMessage>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

struct AdminChatMessage {
    pub msg: String,
    pub sender_name: String,
    pub sender_ip: IpAddr,
    pub contains_banned: bool,
    pub timestamp: DateTime<Utc>,
}

pub fn log_admin_message(msg: &str, sender_name: &str, sender_ip: IpAddr, contains_banned: bool) {
    let mut log = ADMIN_CHAT_LOG.lock().unwrap();
    log.push_back(AdminChatMessage {
        msg: msg.to_string(),
        sender_name: sender_name.to_string(),
        sender_ip,
        contains_banned,
        timestamp: Utc::now(),
    });

    if Utc::now() - log.front().unwrap().timestamp > Duration::days(1) && log.len() > 100 {
        log.pop_front();
    }
}

pub enum AdminAction {
    BanIp(IpAddr),
    UnbanIp(IpAddr),

    BanWord(BannedWord),
    UnbanWord(String),
}

#[instrument(name = "admin", skip(action_tx))]
pub async fn run(action_tx: Sender<AdminAction>) {
    let app = Router::new();

    let serve_admin_dir = ServeDir::new("./static/admin").append_index_html_on_directories(true);
    let serve_shared_dir = ServeDir::new("./static/shared");

    let db = db().await;
    let banned_ips: Vec<IpAddr> = sqlx::query_scalar("SELECT ip FROM banned_ips")
        .fetch_all(db)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|ip: String| ip.parse().ok())
        .collect();
    {
        info!("loaded {} banned ips", banned_ips.len());
        for ip in banned_ips {
            action_tx.send(AdminAction::BanIp(ip)).await.unwrap();
        }
    }

    let banned_words = sqlx::query_as("SELECT word, full_ban FROM banned_words")
        .fetch_all(db)
        .await
        .unwrap();
    {
        info!("loaded {} banned words", banned_words.len());
        for word in banned_words {
            action_tx.send(AdminAction::BanWord(word)).await.unwrap();
        }
    }

    let app = app
        .route("/", get(main_page))
        .route("/login", post(post_login))
        .route("/chat_log", get(get_chat_log))
        .route("/stream_mode", get(get_stream_mode).put(put_stream_mode))
        .route("/banned_ips", get(get_banned_ips).post(post_banned_ip))
        .route("/banned_ips/:ip", delete(delete_banned_ip))
        .route(
            "/banned_words",
            get(get_banned_words).post(post_banned_word),
        )
        .route(
            "/banned_words/:word",
            delete(delete_banned_word).put(put_banned_word),
        )
        .route("/special_events", get(get_special_events))
        .route("/special_events/:event", put(put_special_event))
        .nest_service("/shared", serve_shared_dir)
        .layer(middleware::from_fn(move |req, next| login::auth(req, next)))
        .layer(Extension(action_tx))
        .fallback_service(serve_admin_dir);

    let compression = CompressionLayer::new().gzip(true).zstd(true).br(true);
    let app = app.layer(compression);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081").await.unwrap();

    info!(
        "ready! listening on port {}",
        listener.local_addr().unwrap().port()
    );

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

fn page_base(body: Markup) -> Markup {
    html! {
        (maud::DOCTYPE)
        html {
            head {
                title { "Cibo Online! (Admin Panel)" }
                meta content="text/html;charset=utf-8" http-equiv="Content-Type";
                link rel="apple-touch-icon" sizes="120x120" href="/shared/apple-touch-icon.png";
                link rel="icon" type="image/png" sizes="32x32" href="/shared/favicon-32x32.png";
                link rel="icon" type="image/png" sizes="16x16" href="/shared/favicon-16x16.png";
                link rel="manifest" href="/shared/site.webmanifest";
                link rel="mask-icon" href="/shared/safari-pinned-tab.svg" color="#5bbad5";
                link rel="shortcut icon" href="/shared/favicon.ico";
                meta name="msapplication-TileColor" content="#da532c";
                meta name="msapplication-config" content="/shared/browserconfig.xml";
                meta name="theme-color" content="#ffffff";

                link rel="stylesheet" href="/style.css";
            }

            body {
                (body)
                script src="https://unpkg.com/htmx.org@2.0.1" {}
                script src="/script.js" {}
            }
        }
    }
}

async fn main_page(Extension(auth): Extension<login::AuthState>) -> impl IntoResponse {
    if !auth.is_authenticated() {
        return login_page();
    }

    let is_admin = auth.user().unwrap().username == "admin";

    page_base(html! {
        h1 { "Admin Dashboard" }
        p { "that is incredibly scuffed because im lazy, sowwy ><" }
        p {
            "most things should be self explanatory (i hope)." br; br;
            "a quick explanation for stream mode and banned words:" br;
            "a 'fully banned' word will always be filtered. otherwise, it will only be filtered when stream mode is enabled. " br;
            "this allows using a stricter banlist while the game is being shown on stream :)"
        }

        (get_stream_mode(Extension(auth.clone())).await)
        @if is_admin {
            (get_special_events(Extension(auth.clone())).await)
        }
        div class="panel" {
            div {
                h2 { "Banned IPs" }
                form hx-post="/banned_ips" hx-target="next" hx-swap="beforeend" {
                    input type="text" name="ip" placeholder="IP" required;
                    button type="submit" { "ban" }
                }
                (get_banned_ips(Extension(auth.clone())).await)
            }
            div {
                h2 { "Banned Words" }
                form hx-post="/banned_words" hx-target="next" hx-swap="beforeend" {
                    input type="text" name="word" placeholder="Word" required;
                    button type="submit" { "ban" }
                }
                (get_banned_words(Extension(auth.clone())).await)
            }
        }

        div id="ChatLog" {
            h2 { "Chat Log" }
            table hx-get="/chat_log" hx-trigger="load, every 2s" {}
        }
    })
}

fn login_page() -> Markup {
    page_base(html! {
        form action="/login" method="post" {
            label for="username" { "enter username" }
            input type="text" name="username" placeholder="Username" required
            br;
            label for="password" { "enter password" }
            input type="password" name="password" placeholder="Password" required;
            br;
            input type="submit" value="Login";
        }
    })
}

async fn post_login(Form(data): Form<login::LoginData>) -> impl IntoResponse {
    match login::login(data).await {
        Ok(session_id) => http::Response::builder()
            .status(http::StatusCode::SEE_OTHER)
            .header("Location", "/")
            .header("Set-Cookie", format!("session_id={}", session_id.as_u128()))
            .body(Body::empty())
            .unwrap(),
        Err(err) => page_base(html! {
            "login failed:" (err) br;
            a href="/" { "Try again" }
        })
        .into_response(),
    }
}

#[derive(Deserialize)]
struct TimeOffset {
    offset: i32,
}

async fn get_chat_log(
    Extension(auth): Extension<login::AuthState>,
    Form(TimeOffset { offset }): Form<TimeOffset>,
) -> Markup {
    if !auth.is_authenticated() {
        return page_base(html! {
            p { "authentication failed" }
        });
    }

    let log = ADMIN_CHAT_LOG.lock().unwrap();
    let log = log.iter().rev();

    let offset = FixedOffset::east_opt(offset).unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());

    html! {
        table {
            tr {
                th { "Time" }
                th { "Sender" }
                th { "Message" }
                th { }
            }
            @for msg in log {
                tr style=(if msg.contains_banned { "background-color: orange" } else { "" }) {
                    td { (msg.timestamp.with_timezone(&offset).format("%H:%M:%S")) }
                    td { (msg.sender_name) }
                    td { (msg.msg) }
                    td {
                        form hx-post="/banned_ips" hx-target="#BannedIPs" hx-swap="beforeend" {
                            input type="text" name="ip" value=(msg.sender_ip) style="display: none";
                            button type="submit" { "ban" }
                        }
                    }
                }
            }
        }
    }
}

fn ip_table_row(ip: &str) -> Markup {
    html! {
        tr {
            td { (ip) }
            td { button hx-delete={"/banned_ips/"(ip)} { "x" } }
        }
    }
}

fn ip_table(rows: Vec<String>) -> Markup {
    html! {
        table id="BannedIPs" hx-confirm="sure?" hx-target="closest tr" hx-swap="outerHTML" {
            tr {
                th { "IP" }
                th {  }
            }
            @for row in rows {
                (ip_table_row(&row))
            }
        }
    }
}

#[derive(Deserialize)]
struct StreamMode {
    stream_mode: Option<String>,
}

async fn get_stream_mode(Extension(auth): Extension<login::AuthState>) -> Markup {
    if !auth.is_authenticated() {
        return page_base(html! {
            p { "authentication failed" }
        });
    }
    let is_stream_mode = game_server::get_stream_mode();

    html! {
        label for="stream_mode" { "Enable/Disable Stream Mode" }
        @if is_stream_mode {
            input type="checkbox" name="stream_mode" hx-put="/stream_mode" checked;
        } @else {
            input type="checkbox" name="stream_mode" hx-put="/stream_mode";
        }
    }
}

async fn put_stream_mode(
    Extension(auth): Extension<login::AuthState>,
    Form(StreamMode { stream_mode }): Form<StreamMode>,
) -> Markup {
    if !auth.is_authenticated() {
        return html! {"authentication failed"};
    }
    game_server::set_stream_mode(stream_mode.is_some());
    get_stream_mode(Extension(auth)).await
}

#[derive(Deserialize)]
struct SpecialEventData {
    active: Option<String>,
}

async fn get_special_events(Extension(auth): Extension<login::AuthState>) -> Markup {
    if !auth.is_authenticated() {
        return page_base(html! {
            p { "authentication failed" }
        });
    }

    let special_events = [(
        "Beach Episode",
        game_server::get_special_event(SpecialEvent::BeachEpisode),
    )];

    html! {
        table {
            tr {
                th { "Event" }
                th { "Active" }
            }
            @for (event, active) in special_events {
                tr {
                    td { (event) }
                    td {
                        @if active {
                            input type="checkbox" name="active" hx-put={"/special_events/"(event)} checked;
                        } @else {
                            input type="checkbox" name="active" hx-put={"/special_events/"(event)};
                        }
                    }
                }
            }
        }
    }
}

async fn put_special_event(
    Path(event): Path<String>,
    Extension(auth): Extension<login::AuthState>,
    Form(SpecialEventData { active }): Form<SpecialEventData>,
) -> Markup {
    if !auth.is_authenticated() {
        return html! {"authentication failed"};
    }

    let event = match event.as_str() {
        "Beach Episode" => SpecialEvent::BeachEpisode,
        _ => return html! {"unknown event"},
    };

    game_server::set_special_event(event, active.is_some());
    get_special_events(Extension(auth)).await
}

#[derive(Deserialize)]
struct BannedIp {
    ip: IpAddr,
}

async fn get_banned_ips(Extension(auth): Extension<login::AuthState>) -> Markup {
    if !auth.is_authenticated() {
        return page_base(html! {
            p { "authentication failed" }
        });
    }

    let db = db().await;
    let banned_ips: Vec<String> = sqlx::query_scalar("SELECT ip FROM banned_ips")
        .fetch_all(db)
        .await
        .unwrap()
        .into_iter()
        .collect();

    ip_table(banned_ips)
}

async fn post_banned_ip(
    Extension(action_tx): Extension<Sender<AdminAction>>,
    Extension(auth): Extension<login::AuthState>,
    Form(BannedIp { ip }): Form<BannedIp>,
) -> impl IntoResponse {
    if !auth.is_authenticated() {
        return html! {"authentication failed"}.into_response();
    }

    action_tx.send(AdminAction::BanIp(ip)).await.unwrap();

    let db = db().await;
    match sqlx::query("INSERT INTO banned_ips (ip) VALUES (?)")
        .bind(ip.to_string())
        .execute(db)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            error!("failed to save banned ip: {}", err);

            return http::Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap();
        }
    }

    ip_table_row(&ip.to_string()).into_response()
}

async fn delete_banned_ip(
    Path(ip): Path<IpAddr>,
    Extension(action_tx): Extension<Sender<AdminAction>>,
    Extension(auth): Extension<login::AuthState>,
) -> impl IntoResponse {
    if !auth.is_authenticated() {
        return html! {"authentication failed"}.into_response();
    }

    action_tx.send(AdminAction::UnbanIp(ip)).await.unwrap();

    let db = db().await;
    match sqlx::query("DELETE FROM banned_ips WHERE ip = ?")
        .bind(ip.to_string())
        .execute(db)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            error!("failed to delete banned ip: {}", err);
        }
    }

    http::Response::builder()
        .status(http::StatusCode::OK)
        .body(Body::empty())
        .unwrap()
}

#[derive(Debug, Clone, FromRow)]
pub struct BannedWord {
    pub word: String,
    pub full_ban: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct BannedWordForm {
    word: String,
}
impl From<BannedWordForm> for BannedWord {
    fn from(value: BannedWordForm) -> Self {
        BannedWord {
            word: value.word,
            full_ban: true,
        }
    }
}

fn word_table_row(word: BannedWord) -> Markup {
    html! {
        tr {
            td { (word.word) }
            td {
                @if word.full_ban {
                    input type="checkbox" name="full_ban" hx-put={"/banned_words/"(word.word)} checked;
                } @else {
                    input type="checkbox" name="full_ban" hx-put={"/banned_words/"(word.word)};
                }
            }
            td { button hx-delete={"/banned_words/"(word.word)} { "x" } }
        }
    }
}

fn word_table(rows: Vec<BannedWord>) -> Markup {
    html! {
        table hx-target="closest tr" hx-swap="outerHTML" {
            tr {
                th { "Word" }
                th { "Full Ban?" }
                th {  }
            }
            @for row in rows {
                (word_table_row(row))
            }
        }
    }
}

async fn get_banned_words(Extension(auth): Extension<login::AuthState>) -> Markup {
    if !auth.is_authenticated() {
        return page_base(html! {
            p { "authentication failed" }
        });
    }
    let db = db().await;
    let banned_words = sqlx::query_as("SELECT word, full_ban FROM banned_words")
        .fetch_all(db)
        .await
        .unwrap();

    word_table(banned_words)
}

async fn post_banned_word(
    Extension(action_tx): Extension<Sender<AdminAction>>,
    Extension(auth): Extension<login::AuthState>,
    Form(word): Form<BannedWordForm>,
) -> impl IntoResponse {
    if !auth.is_authenticated() {
        return html! {"authentication failed"}.into_response();
    }

    let word = BannedWord::from(word);

    action_tx
        .send(AdminAction::BanWord(word.clone()))
        .await
        .unwrap();

    let db = db().await;
    match sqlx::query("INSERT OR REPLACE INTO banned_words (word, full_ban) VALUES (?, ?)")
        .bind(&word.word)
        .bind(word.full_ban)
        .execute(db)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            error!("failed to save banned word: {}", err);

            return http::Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap();
        }
    }

    word_table_row(word).into_response()
}

#[derive(Deserialize)]
struct BannedWordParams {
    full_ban: Option<String>,
}

async fn put_banned_word(
    Path(word): Path<String>,
    Extension(action_tx): Extension<Sender<AdminAction>>,
    Extension(auth): Extension<login::AuthState>,
    Form(params): Form<BannedWordParams>,
) -> Markup {
    if !auth.is_authenticated() {
        return html! {"authentication failed"};
    }
    let word = BannedWord {
        word,
        full_ban: params.full_ban.is_some(),
    };

    action_tx
        .send(AdminAction::BanWord(word.clone()))
        .await
        .unwrap();

    let db = db().await;
    match sqlx::query("INSERT OR REPLACE INTO banned_words (word, full_ban) VALUES (?, ?)")
        .bind(&word.word)
        .bind(word.full_ban)
        .execute(db)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            error!("failed to save banned word: {}", err);
            return html! {"failed to save banned word"};
        }
    }
    word_table_row(word)
}

async fn delete_banned_word(
    Path(word): Path<String>,
    Extension(action_tx): Extension<Sender<AdminAction>>,
    Extension(auth): Extension<login::AuthState>,
) -> impl IntoResponse {
    if !auth.is_authenticated() {
        return html! {"authentication failed"}.into_response();
    }

    action_tx
        .send(AdminAction::UnbanWord(word.clone()))
        .await
        .unwrap();

    let db = db().await;
    match sqlx::query("DELETE FROM banned_words WHERE word = ?")
        .bind(word)
        .execute(db)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            error!("failed to delete banned word: {}", err);
        }
    }

    http::Response::builder()
        .status(http::StatusCode::OK)
        .body(Body::empty())
        .unwrap()
}
