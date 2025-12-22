use crate::utils::format_date;
use actix_web::{Error, error, web};
use log::warn;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{MAIN_DB, Statement, params, Transaction};
use serde::{Deserialize, Serialize};
use std::io::{Seek, SeekFrom, Write};

pub type Pool = r2d2::Pool<SqliteConnectionManager>;
pub type R2connection = r2d2::PooledConnection<SqliteConnectionManager>;
type VoicemailResult = Result<Vec<DataType>, rusqlite::Error>;

#[derive(Debug, Serialize, Deserialize)]
pub enum DataType {
    VoiceList {
        id: i64,
        event_time: String,
        caller: String,
        tel: String,
        time: u64,
    },
    Data { data: Vec<u8>, },
    Id { id: i64, },
    BlobSize { offset: u64, },
}

#[allow(clippy::enum_variant_names)]
pub enum Queries {
    AllVoicemail,
    VoiceData(i64),
    DeleteVoicemail(i64),
    InsertData(i64, String, Vec<u8>),
    UpdateSampleTime(i64, u64),
    AddContacts(String, String),
    DeleteContacts(String),
    DeleteBlob(i64),
}

pub fn all_voicemail(conn: &R2connection) -> VoicemailResult {
    let stmt = conn.prepare("
    SELECT A.id, A.event_time, A.caller as tel,
        COALESCE(B.name, A.caller) AS caller,
        A.time
    FROM voicemail as A
    LEFT JOIN contacts as B
    ON A.caller = B.caller")?;
    map_stmt_rows(stmt)
}

fn map_stmt_rows(mut stmt: Statement) -> VoicemailResult {
    stmt.query_map([], |row| {
        Ok(DataType::VoiceList {
            id: row.get(0)?,
            event_time: row.get(1)?,
            tel: row.get(2)?,
            caller: row.get(3)?,
            time: row.get(4).unwrap_or_default(),
        })
    })
    .and_then(Iterator::collect)
}

fn voice_data(conn: &R2connection, id: i64) -> VoicemailResult {
    let data = conn.query_row("SELECT data FROM voicemail WHERE id = (?1)",
                              [id], |row| { row.get(0) })?;
    Ok(vec![DataType::Data { data }])
}

fn del_voicemail(conn: &R2connection, id: i64) -> VoicemailResult {
    conn.execute("DELETE FROM voicemail WHERE id = (?1)", [id])?;
    all_voicemail(conn)
}

fn del_blob(conn: &R2connection, id: i64) -> VoicemailResult {
    conn.execute(
        "UPDATE voicemail SET data = null WHERE id = (?1)",
        [id],
    )?;
    Ok(vec![DataType::Id { id }])
}

fn insert_data(conn: &R2connection, id: i64, caller: &str, data: &[u8]) -> VoicemailResult {
    conn.execute(
        "INSERT INTO voicemail (id, event_time, caller, data) VALUES (?1, ?2, ?3, ?4)",
        params![id, format_date(id), caller, data],
    )?;
    Ok(vec![DataType::Id { id }])
}

fn insert_contacts(conn: &R2connection, caller: &str, name: &str) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "INSERT INTO contacts (caller, name) VALUES (?1, ?2)",
        [caller, name],
    )
}

fn update_contacts(conn: &R2connection, caller: &str, name: &str) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "UPDATE contacts SET name = (?2) WHERE caller = (?1)",
        [caller, name],
    )
}

fn update_sample_time(conn: &R2connection, id: i64, time: u64) -> VoicemailResult {
    conn.execute(
        "UPDATE voicemail SET time = (?2) WHERE id = (?1)",
        params![id, time],
    )?;
    Ok(vec![DataType::Id { id }])
}


fn delete_contacts(conn: &R2connection, caller: &str) -> VoicemailResult {
    conn.execute(
        "DELETE FROM contacts WHERE caller = (?1)",
        [caller],
    )?;
    all_voicemail(conn)
}


fn add_contacts(conn: &R2connection, caller: &str, name: &str) -> VoicemailResult {
    insert_contacts(conn, caller, name).unwrap_or_else(|e|{
        log::info!("{e:?}");
        update_contacts(conn, caller, name).expect("update contacts")
    });
    all_voicemail(conn)
}

pub fn append_chunk_blob(
    conn: &R2connection,
    id: i64,
    offset: u64,
    data: &[u8],
) -> Result<u64, rusqlite::Error> {
    let rowid = conn.query_row("SELECT rowid FROM voicemail WHERE id = (?1)",
                               [id], |row| { row.get(0) })?;
    let mut blob = conn.blob_open(MAIN_DB, "voicemail", "data", rowid, false)?;

    match blob.seek(SeekFrom::Start(offset)) {
        Ok(_) => {
            let bytes_written = blob.write(data).expect("blob write") as u64;
            Ok(offset + bytes_written)
        }
        Err(e) => {
            warn!("{e:?}");
            Ok(offset)
        }
    }
}

pub async fn execute(pool: &Pool, query: Queries) -> Result<Vec<DataType>, Error> {
    let pool = pool.clone();

    let conn = web::block(move || pool.get())
        .await?
        .map_err(error::ErrorInternalServerError)?;

    web::block(move || {
        match query {
            Queries::AllVoicemail => all_voicemail(&conn),
            Queries::VoiceData(id) => voice_data(&conn, id),
            Queries::DeleteVoicemail(id) => del_voicemail(&conn, id),
            Queries::InsertData(id, caller, data)
                => insert_data(&conn, id, &caller, &data),
            Queries::UpdateSampleTime(id, time)
                => update_sample_time(&conn, id, time),
            Queries::AddContacts(caller, name)
                => add_contacts(&conn, &caller, &name),
            Queries::DeleteContacts(caller)
                => delete_contacts(&conn, &caller),
            Queries::DeleteBlob(id)
                => del_blob(&conn, id),
        }
    })
    .await?
    .map_err(error::ErrorInternalServerError)
}

#[allow(unused)]
pub fn tx_append_chunk_blob(
    conn: &Transaction,
    id: i64,
    offset: u64,
    data: &[u8],
) -> Result<u64, rusqlite::Error> {
    let rowid = conn.query_row("SELECT rowid FROM voicemail WHERE id = (?1)",
                               [id], |row| { row.get(0) })?;
    let mut blob = conn.blob_open(MAIN_DB, "voicemail", "data", rowid, false)?;

    match blob.seek(SeekFrom::Start(offset)) {
        Ok(_) => {
            let bytes_written = blob.write(data).expect("blob write") as u64;
            Ok(offset + bytes_written)
        }
        Err(e) => {
            warn!("{e:?}");
            Ok(offset)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::utils::{chunked, file_open, utc_time};
    use crate::web::db::{Pool, Queries, execute, tx_append_chunk_blob};
    use r2d2_sqlite::SqliteConnectionManager;
    #[actix_web::test]
    async fn test_blob() {
        let manager = SqliteConnectionManager::file("database/voicemail.db");
        let pool = Pool::new(manager).unwrap();

        let zero_blob: Vec<u8> = vec![0; 3000000];
        let data = file_open("recv_voice/recv_20250913013410_102.au").expect("file open");
        let id = utc_time().parse::<i64>().unwrap();

        let caller = "test caller".to_string();

        let result = execute(&pool, Queries::InsertData(id, caller, zero_blob))
            .await
            .expect("exec");
        let mut con = pool.get().unwrap();
        let tx = con.transaction().unwrap();
        let mut n = 0;
        let start = Instant::now();
        for i in chunked(data, 160) {
            if let Ok(offset) = tx_append_chunk_blob(&tx, id, n, &i) {
                // println!("write {offset:?}");
                n = offset;
            }
        }
        let end = start.elapsed();
        println!("{}.{:03}秒経過しました。", end.as_secs(), end.subsec_nanos() / 1_000_000);
        tx.commit().expect("commit tx");

        println!("{result:?}");
    }
}
