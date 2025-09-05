
use rsipstack::Error;
use actix_web::rt;
use r2d2_sqlite::SqliteConnectionManager;
use anyhow::Result;
use crate::sip::voice_mail;
use crate::web::db::Pool;

mod utils;
mod web;
mod sip;

#[actix_web::main]
async fn main() -> Result<()> {
    subscriber();

    // connect to SQLite DB
    let manager = SqliteConnectionManager::file("voicemail.db")
        .with_init(|c| c.execute_batch(
            "create table if not exists voicemail (
                id INTEGER PRIMARY KEY,
                event_time TEXT NOT NULL DEFAULT current_timestamp,
                caller TEXT,
                data BLOB
            )",));
    let pool = Pool::new(manager)?;
    let poolc = pool.clone();

    let srv = web::server(poolc);
    rt::spawn(srv);
    voice_mail(pool).await?;
    Ok(())
}


fn subscriber() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_file(true)
        .with_line_number(true)
        .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
        .try_init()
        .ok();
}