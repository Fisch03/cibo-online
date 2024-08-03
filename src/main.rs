mod admin_panel;
mod db;
mod game_server;

use tokio::sync::mpsc::channel;

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt().with_target(false).finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let (tx, rx) = channel(16);
    let admin_panel_task = tokio::spawn(admin_panel::run(tx));
    let game_server_task = tokio::spawn(game_server::run(rx));

    tokio::select! {
        _ = admin_panel_task => {},
        _ = game_server_task => {},
    }
}
