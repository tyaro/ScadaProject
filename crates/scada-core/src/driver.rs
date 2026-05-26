use crate::tag::{QualityCode, TagValueData};

#[derive(Debug, Clone, PartialEq)]
pub struct RawDriverValue {
    pub tag_id: String,
    pub value: TagValueData,
    pub quality: QualityCode,
    pub source_timestamp: String,
    pub driver_id: String,
    pub endpoint_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverWriteRequest {
    pub command_id: String,
    pub tag_id: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverWriteResponse {
    pub command_id: String,
    pub accepted: bool,
    pub message: String,
}

impl DriverWriteResponse {
    pub fn accepted(command_id: &str) -> Self {
        Self {
            command_id: command_id.to_string(),
            accepted: true,
            message: "accepted".to_string(),
        }
    }

    pub fn rejected(command_id: &str, message: &str) -> Self {
        Self {
            command_id: command_id.to_string(),
            accepted: false,
            message: message.to_string(),
        }
    }
}
