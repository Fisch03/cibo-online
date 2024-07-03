use axum::Router;
use tower_http::{compression::CompressionLayer, services::ServeDir};

#[tokio::main]
async fn main() {
    let app = Router::new();

    if let Err(_) = std::fs::read_dir("./static") {
        panic!("could not read static directory");
    }

    let serve_dir = ServeDir::new("./static").append_index_html_on_directories(true);
    let app = app.fallback_service(serve_dir);

    let compression = CompressionLayer::new()
        .gzip(true)
        .zstd(true)
        .br(true)
        .deflate(true);
    let app = app.layer(compression);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
