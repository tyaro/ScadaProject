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
}
