#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    Boolean,
    Integer,
    Float,
    String,
}

impl DataType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::String => "string",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TagValueData {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

impl TagValueData {
    pub fn data_type(&self) -> DataType {
        match self {
            Self::Boolean(_) => DataType::Boolean,
            Self::Integer(_) => DataType::Integer,
            Self::Float(_) => DataType::Float,
            Self::String(_) => DataType::String,
        }
    }

    pub fn from_json_value(data_type: &str, value: &serde_json::Value) -> Result<Self, String> {
        match data_type {
            "boolean" => value
                .as_bool()
                .map(Self::Boolean)
                .ok_or_else(|| "boolean value must be true or false".to_string()),
            "integer" => value
                .as_i64()
                .map(Self::Integer)
                .ok_or_else(|| "integer value must be a signed integer".to_string()),
            "float" => value
                .as_f64()
                .map(Self::Float)
                .ok_or_else(|| "float value must be numeric".to_string()),
            "string" => value
                .as_str()
                .map(|value| Self::String(value.to_string()))
                .ok_or_else(|| "string value must be a string".to_string()),
            other => Err(format!("unsupported data_type: {other}")),
        }
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Self::Boolean(value) => serde_json::Value::Bool(*value),
            Self::Integer(value) => serde_json::Value::Number((*value).into()),
            Self::Float(value) => serde_json::Number::from_f64(*value)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Self::String(value) => serde_json::Value::String(value.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityCode {
    Good,
    Uncertain,
    Bad,
    Stale,
    CommLost,
    OutOfRange,
    Manual,
    Simulated,
}

impl QualityCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Good => "Good",
            Self::Uncertain => "Uncertain",
            Self::Bad => "Bad",
            Self::Stale => "Stale",
            Self::CommLost => "CommLost",
            Self::OutOfRange => "OutOfRange",
            Self::Manual => "Manual",
            Self::Simulated => "Simulated",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "Good" => Ok(Self::Good),
            "Uncertain" => Ok(Self::Uncertain),
            "Bad" => Ok(Self::Bad),
            "Stale" => Ok(Self::Stale),
            "CommLost" => Ok(Self::CommLost),
            "OutOfRange" => Ok(Self::OutOfRange),
            "Manual" => Ok(Self::Manual),
            "Simulated" => Ok(Self::Simulated),
            other => Err(format!("unsupported quality: {other}")),
        }
    }

    pub fn is_usable(&self) -> bool {
        matches!(
            self,
            Self::Good | Self::Uncertain | Self::Manual | Self::Simulated
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TagValue {
    pub tag_id: String,
    pub value: TagValueData,
    pub quality: QualityCode,
    pub source_timestamp: String,
    pub server_timestamp: String,
    pub sequence: u64,
    pub scan_interval_ms: u64,
    pub stale_after_ms: u64,
    pub driver_id: String,
    pub endpoint_id: String,
    pub read_status: String,
    pub write_status: String,
}

impl TagValue {
    pub fn data_type(&self) -> DataType {
        self.value.data_type()
    }

    pub fn is_newer_than(&self, other: &TagValue) -> bool {
        self.sequence > other.sequence
    }
}

pub fn tag_value_to_json_value(value: &TagValue) -> serde_json::Value {
    serde_json::json!({
        "tag_id": value.tag_id,
        "value": value.value.to_json_value(),
        "data_type": value.value.data_type().as_str(),
        "quality": value.quality.as_str(),
        "source_timestamp": value.source_timestamp,
        "server_timestamp": value.server_timestamp,
        "sequence": value.sequence,
        "scan_interval_ms": value.scan_interval_ms,
        "stale_after_ms": value.stale_after_ms,
        "driver_id": value.driver_id,
        "endpoint_id": value.endpoint_id,
        "read_status": value.read_status,
        "write_status": value.write_status,
    })
}

pub fn tag_value_to_json(value: &TagValue) -> String {
    tag_value_to_json_value(value).to_string()
}

pub fn tag_value_from_json_value(value: &serde_json::Value) -> Result<TagValue, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "tag value must be a JSON object".to_string())?;
    let data_type = required_string(object, "data_type")?;
    let raw_value = object
        .get("value")
        .ok_or_else(|| "missing value".to_string())?;

    Ok(TagValue {
        tag_id: required_string(object, "tag_id")?.to_string(),
        value: TagValueData::from_json_value(data_type, raw_value)?,
        quality: QualityCode::parse(required_string(object, "quality")?)?,
        source_timestamp: required_string(object, "source_timestamp")?.to_string(),
        server_timestamp: required_string(object, "server_timestamp")?.to_string(),
        sequence: required_u64(object, "sequence")?,
        scan_interval_ms: required_u64(object, "scan_interval_ms")?,
        stale_after_ms: required_u64(object, "stale_after_ms")?,
        driver_id: required_string(object, "driver_id")?.to_string(),
        endpoint_id: required_string(object, "endpoint_id")?.to_string(),
        read_status: required_string(object, "read_status")?.to_string(),
        write_status: required_string(object, "write_status")?.to_string(),
    })
}

pub fn tag_value_from_json_str(input: &str) -> Result<TagValue, String> {
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|error| format!("invalid tag value JSON: {error}"))?;
    tag_value_from_json_value(&value)
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

fn required_u64(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<u64, String> {
    object
        .get(key)
        .and_then(|value| value.as_u64())
        .ok_or_else(|| format!("missing or invalid {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_code_marks_good_values_usable() {
        assert!(QualityCode::Good.is_usable());
        assert!(QualityCode::Simulated.is_usable());
        assert!(!QualityCode::CommLost.is_usable());
        assert!(!QualityCode::Bad.is_usable());
    }

    #[test]
    fn tag_value_reports_data_type() {
        let value = TagValueData::Float(12.5);
        assert_eq!(DataType::Float, value.data_type());
    }

    #[test]
    fn tag_value_json_round_trips_scalar_shape() {
        let value = TagValue {
            tag_id: "mock.temperature.001".to_string(),
            value: TagValueData::Float(22.5),
            quality: QualityCode::Simulated,
            source_timestamp: "1970-01-01T00:00:00Z".to_string(),
            server_timestamp: "1970-01-01T00:00:01Z".to_string(),
            sequence: 7,
            scan_interval_ms: 1000,
            stale_after_ms: 3000,
            driver_id: "mock-driver".to_string(),
            endpoint_id: "mock-endpoint".to_string(),
            read_status: "ok".to_string(),
            write_status: "idle".to_string(),
        };

        let encoded = tag_value_to_json(&value);
        let decoded = tag_value_from_json_str(&encoded).expect("decode tag value");

        assert_eq!(value, decoded);
        assert!(encoded.contains(r#""data_type":"float""#));
        assert!(encoded.contains(r#""value":22.5"#));
    }
}
