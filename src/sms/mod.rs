use std::collections::HashMap;
use std::env;
use aws_sdk_sns::types::{MessageAttributeValue, RouteType};
use anyhow::{Error, Result};

// Send SMS notification to a phone number using AWS SNS.
// The following environment variables need to be defined in the .env file.
// AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION, AWS_SNS_PHONE_NO, AWS_SNS_SENDER_ID
pub async fn notify(message: &str) -> Result<()> {
    let sender_id = env::var("AWS_SNS_SENDER_ID")
        .map_err(|e| Error::msg(format!("AWS_SNS_SENDER_ID not found: {e}")))?;
    let topic_arn = env::var("AWS_SNS_TOPIC_ARN")
        .map_err(|e| Error::msg(format!("AWS_SNS_TOPIC_ARN not found: {e}")))?;

    let config = aws_config::from_env().load().await;
    let client = aws_sdk_sns::Client::new(&config);
    let mut attributes = HashMap::new();
    attributes.insert("AWS.SNS.SMS.SMSType".to_string(),
                      MessageAttributeValue::builder()
                          .data_type("String")
                          .string_value(RouteType::Transactional.to_string())
                          .build()?);
    attributes.insert("AWS.SNS.SMS.SenderID".to_string(),
                      MessageAttributeValue::builder()
                          .data_type("String")
                          .string_value(sender_id)
                          .build()?);

    let publish_output = client.publish().set_message(Some(message.into()))
        .set_message_attributes(Some(attributes))
        .topic_arn(topic_arn)
        .send().await.map_err(|e| e)?;
    log::info!("PublishOutput: {:?}", publish_output);
    Ok(())
}

#[test]
fn test_notify() {
    if let Err(e) = dotenv::dotenv() {
        log::info!("Failed to load .env file: {}", e);
    }
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(notify("test sms targetArn")).expect("TODO: panic message");
}