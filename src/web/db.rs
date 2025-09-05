
use actix_web::{error, web, Error};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Statement};
use serde::{Deserialize, Serialize};
use crate::utils::format_date;


pub type Pool = r2d2::Pool<SqliteConnectionManager>;
pub type R2connection = r2d2::PooledConnection<SqliteConnectionManager>;

type VoicemailResult = Result<Vec<DataType>, rusqlite::Error>;
#[allow(unreachable_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Voicemail {
    id: usize,
    event_time: String,
    caller: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DataType {
    VoiceList { id: usize, event_time: String, caller: String },
    Data { data: Vec<u8> },
    Id { id: usize },
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
    stmt
        .query_map([], |row| {
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
                                        [id], |row| row.get(0))?;

    Ok(vec![DataType::Data { data }])
}

pub fn del_voicemail(conn: R2connection, id: i64) -> VoicemailResult {
    conn.execute("DELETE FROM voicemail WHERE id = (?1)", [id])?;
    let stmt = conn.prepare("SELECT * FROM voicemail")?;
    map_stmt_rows(stmt)
}

pub fn insert_data(
    conn: R2connection,
    id: usize,
    caller: &str,
    data: Vec<u8>
) -> VoicemailResult {
    conn.execute("INSERT INTO voicemail (id, event_time, caller, data) VALUES (?1, ?2, ?3, ?4)",
                 params![id as i64, format_date(id as i64), caller, data])?;
    Ok(vec![DataType::Id { id }])
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
                => insert_data(conn, id, &caller, data),
        }
    })
        .await?
        .map_err(error::ErrorInternalServerError)
}


#[cfg(test)]
mod tests {
    use r2d2_sqlite::SqliteConnectionManager;
    use crate::web::db::{execute, Pool, Queries};
    use crate::utils::{file_open, local_time};

    #[actix_web::test]
    async fn test_blob() {
        let manager = SqliteConnectionManager::file("../../voicemail.db");
        let pool = Pool::new(manager).unwrap();

        let data = file_open("voicemail/recv_20250904212349_102.au").expect("file open");
        let id = local_time().parse::<usize>().unwrap();

        let caller = "test caller".to_string();

        let result = execute(&pool, Queries::InsertData(id, caller, data)).await.expect("exec");
        println!("{result:?}");

    }
}
