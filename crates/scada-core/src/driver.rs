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

pub fn raw_driver_value_to_json_value(value: &RawDriverValue) -> serde_json::Value {
    serde_json::json!({
        "tag_id": value.tag_id,
        "value": value.value.to_json_value(),
        "data_type": value.value.data_type().as_str(),
        "quality": value.quality.as_str(),
        "source_timestamp": value.source_timestamp,
        "driver_id": value.driver_id,
        "endpoint_id": value.endpoint_id,
    })
}

pub fn raw_driver_value_to_json(value: &RawDriverValue) -> String {
    raw_driver_value_to_json_value(value).to_string()
}

pub fn raw_driver_value_from_json_value(
    value: &serde_json::Value,
) -> Result<RawDriverValue, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "raw driver value must be a JSON object".to_string())?;
    let data_type = required_string(object, "data_type")?;
    let raw_value = object
        .get("value")
        .ok_or_else(|| "missing value".to_string())?;

    Ok(RawDriverValue {
        tag_id: required_string(object, "tag_id")?.to_string(),
        value: TagValueData::from_json_value(data_type, raw_value)?,
        quality: QualityCode::parse(required_string(object, "quality")?)?,
        source_timestamp: required_string(object, "source_timestamp")?.to_string(),
        driver_id: required_string(object, "driver_id")?.to_string(),
        endpoint_id: required_string(object, "endpoint_id")?.to_string(),
    })
}

pub fn raw_driver_value_from_json_str(input: &str) -> Result<RawDriverValue, String> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|error| format!("invalid raw driver value JSON: {error}"))?;
    raw_driver_value_from_json_value(&value)
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("missing or invalid {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_driver_value_json_round_trips_scalar_shape() {
        let value = RawDriverValue {
            tag_id: "mock.running.001".to_string(),
            value: TagValueData::Boolean(true),
            quality: QualityCode::Simulated,
            source_timestamp: "1970-01-01T00:00:00Z".to_string(),
            driver_id: "mock-driver".to_string(),
            endpoint_id: "mock-endpoint".to_string(),
        };

        let encoded = raw_driver_value_to_json(&value);
        let decoded = raw_driver_value_from_json_str(&encoded).expect("decode raw driver value");

        assert_eq!(value, decoded);
        assert!(encoded.contains(r#""data_type":"boolean""#));
        assert!(encoded.contains(r#""value":true"#));
    }
}
