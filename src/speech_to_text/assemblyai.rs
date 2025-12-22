use ai_sdk_assemblyai::AssemblyAIClient;
use ai_sdk_provider::shared::provider_options::SharedProviderOptions;
use ai_sdk_provider::transcription_model::call_options::TranscriptionModelCallOptions;
use bytes::Bytes;
use mp3lame_encoder::{Builder, FlushNoGap, MonoPcm};
use std::env;

pub(crate) async fn transcript(pcmu: Bytes) -> Result<String, Box<dyn std::error::Error>> {
    let api_key = env::var("ASSEMBLYAI_API_KEY")
        .map_err(|_| "ASSEMBLYAI_API_KEY not found")?;
    // ISO-639-1
    let language_code = env::var("ASSEMBLYAI_LANGUAGE_CODE")
        .unwrap_or_else(|_| "en".to_string());

    let mut v = Vec::new();
    for i in 0..pcmu.len() {
        v.push(audio_codec_algorithms::decode_ulaw(pcmu[i]));
    }
    let audio_data = pcm_to_mp3(&v);

    let provider = AssemblyAIClient::new().api_key(api_key).build();
    let model = provider.transcription_model("best");

    let mut provider_options = SharedProviderOptions::new();
    provider_options.insert(
        "assemblyai".to_string(),
        vec![("languageCode".to_string(), serde_json::json!(language_code))]
            .into_iter()
            .collect(),
    );

    let call_options = TranscriptionModelCallOptions::mp3(audio_data.to_vec())
        .with_provider_options(provider_options);

    let result = model.do_generate(call_options).await?;

    Ok(result.text)
}

fn pcm_to_mp3(pcm: &[i16]) -> Bytes {
    let mut mp3_encoder = Builder::new()
        .expect("Create LAME builder")
        .with_num_channels(1)
        .expect("set channels")
        .with_sample_rate(8000)
        .expect("set sample rate")
        .with_brate(mp3lame_encoder::Bitrate::Kbps16)
        .expect("set brate")
        .with_quality(mp3lame_encoder::Quality::Best)
        .expect("set quality")
        .build()
        .expect("Build encoder");

    let mut mp3_out_buffer = Vec::new();

    let input = MonoPcm(&pcm);
    mp3_out_buffer.reserve(mp3lame_encoder::max_required_buffer_size(input.0.len()));
    let encoded_size = mp3_encoder
        .encode(input, mp3_out_buffer.spare_capacity_mut())
        .expect("To encode");
    unsafe {
        mp3_out_buffer.set_len(mp3_out_buffer.len().wrapping_add(encoded_size));
    }

    let encoded_size = mp3_encoder
        .flush::<FlushNoGap>(mp3_out_buffer.spare_capacity_mut())
        .expect("to flush");
    unsafe {
        mp3_out_buffer.set_len(mp3_out_buffer.len().wrapping_add(encoded_size));
    }
    Bytes::from(mp3_out_buffer)
}
