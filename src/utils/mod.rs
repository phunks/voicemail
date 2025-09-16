use chrono::NaiveDateTime;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use tokio::io::AsyncWriteExt;
use tracing::info;

pub mod macros;

pub fn open(x: PathBuf) -> String {
    fs::read_to_string(x).expect("Something went wrong")
}

#[allow(unused)]
pub fn local_time() -> String {
    chrono::Local::now()
        .naive_local()
        .format("%Y%m%d%H%M%S")
        .to_string()
}

pub fn utc_time() -> String {
    chrono::Utc::now()
        .format("%Y%m%d%H%M%S")
        .to_string()
}

pub fn format_date(date: i64) -> String {
    // 20250903120320 -> 2025-09-03 12:03:20
    let ndt = NaiveDateTime::parse_from_str(&date.to_string(), "%Y%m%d%H%M%S").unwrap();
    // ndt.format("%Y-%m-%dT%H:%M:%S%z").to_string()
    ndt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[test]
fn test_format_date() {
    let aa = utc_time().parse::<i64>().unwrap();
    let ndt = NaiveDateTime::parse_from_str(&aa.to_string(), "%Y%m%d%H%M%S").unwrap();
    // println!("{}", Utc.from_utc_datetime(&ndt).to_string());
    println!("{}", ndt.format("%Y-%m-%dT%H:%M:%SZ").to_string());
}

pub fn trim_null_bytes(data: &[u8]) -> &[u8] {
    let start = data.iter().position(|&b| b != 0x00).unwrap_or(0);
    let end = data
        .iter()
        .rposition(|&b| b != 0x00)
        .map(|i| i + 1)
        .unwrap_or(0);
    &data[start..end]
}

#[allow(unused)]
pub fn delete_file(file: &str) {
    fs::remove_file(file).expect("TODO: panic message");
}

#[allow(unused)]
pub async fn write_file(file: &str, dat: &[u8]) {
    let mut pcm = tokio::fs::File::create(file).await.expect("write failer");
    match pcm.write_all(dat).await {
        Ok(_) => {}
        Err(e) => {
            info!("Failed to write pcm: {:?}", e);
        }
    };
    pcm.flush().await.expect("buf flush");
}

#[allow(unused)]
pub fn file_open(file: &str) -> anyhow::Result<Vec<u8>> {
    let mut data = vec![];
    let mut f = File::open(PathBuf::from_str(file)?)?;
    f.read_to_end(&mut data)?;
    Ok(data)
}

#[allow(unused)]
pub struct Chunked<I> {
    iterator: I,
    chunk_size: usize,
}

#[allow(unused)]
pub fn chunked<Collection>(a: Collection, chunk_size: usize) -> Chunked<Collection::IntoIter>
where
    Collection: IntoIterator,
{
    let iterator = a.into_iter();
    Chunked {
        iterator,
        chunk_size,
    }
}

impl<I: Iterator> Iterator for Chunked<I> {
    type Item = Vec<I::Item>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.iterator.by_ref().take(self.chunk_size).collect())
            .filter(|chunk: &Vec<_>| !chunk.is_empty())
    }
}
