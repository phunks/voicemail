use crate::sip::voice_mail;
use crate::web::db::Pool;
use actix_web::rt;
use anyhow::Result;
use r2d2_sqlite::SqliteConnectionManager;
use rsipstack::Error;

mod sip;
mod utils;
mod web;
mod sms;
mod speech_to_text;

#[actix_web::main]
async fn main() -> Result<()> {
    subscriber();

    let manager = SqliteConnectionManager::file("./database/voicemail.db").with_init(|c| {
        c.execute_batch(
            "BEGIN;
                create table if not exists voicemail (
                    id INTEGER PRIMARY KEY,
                    event_time TEXT NOT NULL DEFAULT current_timestamp,
                    caller TEXT,
                    time INTEGER,
                    data BLOB
                );
                create table if not exists contacts (
                    caller TEXT PRIMARY KEY,
                    name TEXT
                );
            COMMIT;",
        )
    });
    let pool = Pool::new(manager)?;

    let srv = web::server(pool.clone());
    rt::spawn(srv);
    voice_mail(pool).await?;
    Ok(())
}

fn subscriber() {
    tracing_subscriber::fmt()
        .with_ansi(false)
        // .with_max_level(tracing::Level::INFO)
        .with_file(true)
        .with_line_number(true)
        .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
        .try_init()
        .ok();
}
