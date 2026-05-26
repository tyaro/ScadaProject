use scada_core::driver::RawDriverValue;
use scada_core::tag::TagValue;

#[derive(Debug, Clone)]
pub struct ValueNormalizer {
    sequence: u64,
    scan_interval_ms: u64,
    stale_after_ms: u64,
}

impl ValueNormalizer {
    pub fn new(scan_interval_ms: u64, stale_after_ms: u64) -> Self {
        Self {
            sequence: 0,
            scan_interval_ms,
            stale_after_ms,
        }
    }

    pub fn normalize(&mut self, raw: RawDriverValue, server_timestamp: &str) -> TagValue {
        self.sequence += 1;

        TagValue {
            tag_id: raw.tag_id,
            value: raw.value,
            quality: raw.quality,
            source_timestamp: raw.source_timestamp,
            server_timestamp: server_timestamp.to_string(),
            sequence: self.sequence,
            scan_interval_ms: self.scan_interval_ms,
            stale_after_ms: self.stale_after_ms,
            driver_id: raw.driver_id,
            endpoint_id: raw.endpoint_id,
            read_status: "ok".to_string(),
            write_status: "idle".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scada_core::driver::RawDriverValue;
    use scada_core::tag::{QualityCode, TagValueData};

    #[test]
    fn normalizer_assigns_sequence_and_server_timestamp() {
        let mut normalizer = ValueNormalizer::new(1000, 3000);
        let tag_value = normalizer.normalize(
            RawDriverValue {
                tag_id: "mock.temperature.001".to_string(),
                value: TagValueData::Float(25.0),
                quality: QualityCode::Simulated,
                source_timestamp: "1970-01-01T00:00:00Z".to_string(),
                driver_id: "mock-driver".to_string(),
                endpoint_id: "mock-endpoint".to_string(),
            },
            "1970-01-01T00:00:01Z",
        );

        assert_eq!(1, tag_value.sequence);
        assert_eq!("1970-01-01T00:00:01Z", tag_value.server_timestamp);
    }
}
