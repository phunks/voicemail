use anyhow::Result;
use bytes::Bytes;
use google_cloud_auth::credentials;
use google_cloud_speech_v2::client::Speech;
use google_cloud_speech_v2::model::{RecognitionConfig, RecognizeResponse, recognize_request};
use std::env;

// Transcribe an audio file and return the transcription as a string.
// Speech to Text API using Google Cloud Speech.
// The following environment variables need to be defined in the .env file.
//
// GOOGLE_APPLICATION_CREDENTIALS: ex. /path/to/service-account.json
// GOOGLE_CLOUD_REGION, GOOGLE_CLOUD_PROJECT_ID,
// GOOGLE_CLOUD_SNS_RECOGNIZER_ID, GOOGLE_APPLICATION_LANGUAGE_CODES
pub async fn transcript(pcmu: Bytes) -> Result<RecognizeResponse, Box<dyn std::error::Error>> {
    let region =
        env::var("GOOGLE_CLOUD_REGION")
            .map_err(|_| "GOOGLE_CLOUD_REGION not found")?;
    let project_id =
        env::var("GOOGLE_CLOUD_PROJECT_ID")
            .map_err(|_| "GOOGLE_CLOUD_PROJECT_ID not found")?;
    let recognizer_id =
        env::var("GOOGLE_CLOUD_SNS_RECOGNIZER_ID")
        .map_err(|_| "GOOGLE_CLOUD_SNS_RECOGNIZER_ID not found")?;
    let language_codes =
        env::var("GOOGLE_APPLICATION_LANGUAGE_CODES")
            .unwrap_or_else(|_| "en-US".to_string());

    let audio_content = recognize_request::AudioSource::Content(pcmu);

    let config = RecognitionConfig::new()
        .set_model("telephony".to_string())
        .set_language_codes(vec![language_codes]);

    let credentials = credentials::Builder::default().build()?;

    let client = Speech::builder()
        // .with_tracing()
        .with_credentials(credentials)
        .with_endpoint(format!("https://{region}-speech.googleapis.com"))
        .build()
        .await?;

    let response = client
        .recognize()
        .set_recognizer(format!(
            "projects/{project_id}/locations/{region}/recognizers/{recognizer_id}"
        ))
        .set_config(config)
        .set_audio_source(Some(audio_content))
        .send()
        .await?;

    Ok(response)
}

pub fn join_transcript(resp: &RecognizeResponse) -> String {
    resp.results
        .iter()
        .filter_map(|r| r.alternatives.first())
        .map(|a| a.transcript.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
