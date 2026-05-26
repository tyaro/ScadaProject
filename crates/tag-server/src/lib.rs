use std::collections::HashMap;

use scada_core::tag::TagValue;

#[derive(Debug, Default)]
pub struct InMemoryTagCache {
    values: HashMap<String, TagValue>,
}

impl InMemoryTagCache {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn ingest(&mut self, value: TagValue) -> bool {
        let should_update = match self.values.get(&value.tag_id) {
            Some(current) => value.is_newer_than(current),
            None => true,
        };

        if should_update {
            self.values.insert(value.tag_id.clone(), value);
        }

        should_update
    }

    pub fn get(&self, tag_id: &str) -> Option<&TagValue> {
        self.values.get(tag_id)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scada_core::tag::{QualityCode, TagValue, TagValueData};

    fn tag_value(sequence: u64) -> TagValue {
        TagValue {
            tag_id: "mock.temperature.001".to_string(),
            value: TagValueData::Float(sequence as f64),
            quality: QualityCode::Simulated,
            source_timestamp: "1970-01-01T00:00:00Z".to_string(),
            server_timestamp: "1970-01-01T00:00:00Z".to_string(),
            sequence,
            scan_interval_ms: 1000,
            stale_after_ms: 3000,
            driver_id: "mock-driver".to_string(),
            endpoint_id: "mock-endpoint".to_string(),
            read_status: "ok".to_string(),
            write_status: "idle".to_string(),
        }
    }

    #[test]
    fn cache_accepts_newer_values() {
        let mut cache = InMemoryTagCache::new();

        assert!(cache.ingest(tag_value(1)));
        assert!(cache.ingest(tag_value(2)));
        assert_eq!(2, cache.get("mock.temperature.001").unwrap().sequence);
    }

    #[test]
    fn cache_rejects_older_values() {
        let mut cache = InMemoryTagCache::new();

        assert!(cache.ingest(tag_value(2)));
        assert!(!cache.ingest(tag_value(1)));
        assert_eq!(2, cache.get("mock.temperature.001").unwrap().sequence);
    }
}
