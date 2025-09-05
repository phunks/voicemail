use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use chrono::NaiveDateTime;
use rustc_serialize::hex::ToHex;

pub mod macros;

pub fn open(x: PathBuf) -> String {
    fs::read_to_string(x).expect("Something went wrong")
}

#[allow(unused)]
pub fn digest_md5(s: &str) -> String {
    md5::compute(s).to_hex()
}

pub fn local_time() -> String {
    chrono::Local::now().naive_local().format("%Y%m%d%H%M%S").to_string()
}

pub fn format_date(date: i64) -> String {
    // 20250903120320 -> 2025-09-03 12:03:20
    let ndt = NaiveDateTime::parse_from_str(&date.to_string(), "%Y%m%d%H%M%S").unwrap();
    ndt.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[allow(unused)]
pub fn file_open(file: &str) -> anyhow::Result<Vec<u8>> {
    let mut data = vec![];
    let mut f = File::open(PathBuf::from_str(file)?)?;
    f.read_to_end(&mut data)?;
    Ok(data)
}

#[test]
fn test_digest_md5() {
    let aa = "<sip:102@192.168.101.1:5060>;tag=RCxUu42pu3VJRPibsDI4SXk2rAf8uJxs";
    println!("{:?}", digest_md5(aa));
}
