use crate::utils::trim_null_bytes;
use crate::web::db::DataType::Data;
use crate::web::db::{Pool, Queries};
use anyhow::Result;
use bytes::Bytes;
use crate::sip::AiModels;

mod assemblyai;
mod gcp;

pub async fn execute(
    pool: &Pool,
    id: i64,
    transcript_type: AiModels,
) -> Result<String, Box<dyn std::error::Error>> {
    let pcmu: Bytes = match crate::web::db::execute(&pool, Queries::VoiceData(id))
        .await?
        .first()
    {
        Some(Data { data }) => {
            let pcmu = trim_null_bytes(data);
            if pcmu.is_empty() {
                return Err("no data".into());
            };
            pcmu
        }
        _ => return Err("no data".into()),
    };
    log::info!("transcript: {transcript_type}");
    match transcript_type {
        AiModels::Gcp => gcp::transcript(pcmu)
            .await
            .map(|r| gcp::join_transcript(&r)),
        AiModels::Assemblyai => assemblyai::transcript(pcmu).await,
    }
}
