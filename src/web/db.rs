use crate::utils::format_date;
use actix_web::{Error, error, web};
use log::warn;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{MAIN_DB, Statement, params};
use serde::{Deserialize, Serialize};
use std::io::{Seek, SeekFrom, Write};

pub type Pool = r2d2::Pool<SqliteConnectionManager>;
pub type R2connection = r2d2::PooledConnection<SqliteConnectionManager>;
type VoicemailResult = Result<Vec<DataType>, rusqlite::Error>;

#[derive(Debug, Serialize, Deserialize)]
pub enum DataType {
    VoiceList {
        id: usize,
        event_time: String,
        caller: String,
    },
    Data { data: Vec<u8>, },
    Id { id: usize, },
    BlobSize { offset: u64, },
}

#[allow(clippy::enum_variant_names)]
pub enum Queries {
    AllVoicemail,
    VoiceData(i64),
    DeleteVoicemail(i64),
    InsertData(usize, String, Vec<u8>),
}

pub fn all_voicemail(conn: R2connection) -> VoicemailResult {
    let stmt = conn.prepare("SELECT * FROM voicemail")?;
    map_stmt_rows(stmt)
}

fn map_stmt_rows(mut stmt: Statement) -> VoicemailResult {
    stmt.query_map([], |row| {
        Ok(DataType::VoiceList {
            id: row.get(0)?,
            event_time: row.get(1)?,
            caller: row.get(2)?,
        })
    })
    .and_then(Iterator::collect)
}

pub fn voice_data(conn: R2connection, id: i64) -> VoicemailResult {
    let data = conn.query_row("SELECT data FROM voicemail WHERE id = (?1)",
                              [id], |row| { row.get(0) })?;
    Ok(vec![DataType::Data { data }])
}

pub fn del_voicemail(conn: R2connection, id: i64) -> VoicemailResult {
    conn.execute("DELETE FROM voicemail WHERE id = (?1)", [id])?;
    let stmt = conn.prepare("SELECT * FROM voicemail")?;
    map_stmt_rows(stmt)
}

pub fn insert_data(conn: R2connection, id: usize, caller: &str, data: &[u8]) -> VoicemailResult {
    conn.execute(
        "INSERT INTO voicemail (id, event_time, caller, data) VALUES (?1, ?2, ?3, ?4)",
        params![id as i64, format_date(id as i64), caller, data],
    )?;
    Ok(vec![DataType::Id { id }])
}

pub fn append_chunk_blob(
    conn: &R2connection,
    id: usize,
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
            Queries::AllVoicemail => all_voicemail(conn),
            Queries::VoiceData(id) => voice_data(conn, id),
            Queries::DeleteVoicemail(id) => del_voicemail(conn, id),
            Queries::InsertData(id, caller, data)
                => insert_data(conn, id, &caller, &data),
        }
    })
    .await?
    .map_err(error::ErrorInternalServerError)
}

#[cfg(test)]
mod tests {
    use crate::utils::{chunked, file_open, local_time};
    use crate::web::db::{Pool, Queries, execute};
    use r2d2_sqlite::SqliteConnectionManager;
    #[actix_web::test]
    async fn test_blob() {
        let manager = SqliteConnectionManager::file("voicemail.db");
        let pool = Pool::new(manager).unwrap();

        let zero_blob: Vec<u8> = vec![0; 3000000];
        let data = file_open("recv_voice/recv_20250913013410_102.au").expect("file open");
        let id = local_time().parse::<usize>().unwrap();

        let caller = "test caller".to_string();

        let result = execute(&pool, Queries::InsertData(id, caller, zero_blob))
            .await
            .expect("exec");
        let mut con = pool.get().unwrap();
        let tx = con.transaction().unwrap();
        let mut n = 0;
        for i in chunked(data, 160) {
            // if let Ok(offset) = append_chunk_blob(&tx, id, n, &i) {
            //     println!("write {offset:?}");
            //     n = offset;
            // }
        }
        tx.commit().expect("commit tx");

        println!("{result:?}");
    }
}
