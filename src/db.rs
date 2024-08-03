use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool};
use sqlx::Error;

use std::str::FromStr;
use tokio::sync::OnceCell;

const DB_PATH: &str = "sqlite://./data/db.sqlite?mode=rwc";

pub async fn db() -> &'static SqlitePool {
    static DB: OnceCell<SqlitePool> = OnceCell::const_new();
    DB.get_or_init(|| async { init_db().await.expect("Failed to initialize database") })
        .await
}

async fn init_db() -> Result<SqlitePool, Error> {
    let options = SqliteConnectOptions::from_str(DB_PATH)?.journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePool::connect_with(options).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
