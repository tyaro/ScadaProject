use scada_core::driver::{DriverWriteRequest, DriverWriteResponse, RawDriverValue};
use scada_core::tag::{QualityCode, TagValueData};

#[derive(Debug, Clone)]
pub struct MockDriver {
    driver_id: String,
    endpoint_id: String,
    sequence: u64,
}

impl MockDriver {
    pub fn new(driver_id: &str, endpoint_id: &str) -> Self {
        Self {
            driver_id: driver_id.to_string(),
            endpoint_id: endpoint_id.to_string(),
            sequence: 0,
        }
    }

    pub fn next_values(&mut self) -> Vec<RawDriverValue> {
        self.sequence += 1;
        let temperature = 20.0 + (self.sequence % 10) as f64;
        let running = self.sequence % 2 == 0;

        vec![
            self.raw_value(
                "mock.temperature.001",
                TagValueData::Float(temperature),
                QualityCode::Simulated,
            ),
            self.raw_value(
                "mock.running.001",
                TagValueData::Boolean(running),
                QualityCode::Simulated,
            ),
        ]
    }

    pub fn write(&self, request: &DriverWriteRequest) -> DriverWriteResponse {
        if request.tag_id.starts_with("mock.") {
            DriverWriteResponse::accepted(&request.command_id)
        } else {
            DriverWriteResponse::rejected(&request.command_id, "unknown mock tag")
        }
    }

    fn raw_value(&self, tag_id: &str, value: TagValueData, quality: QualityCode) -> RawDriverValue {
        RawDriverValue {
            tag_id: tag_id.to_string(),
            value,
            quality,
            source_timestamp: "1970-01-01T00:00:00Z".to_string(),
            driver_id: self.driver_id.clone(),
            endpoint_id: self.endpoint_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_driver_generates_deterministic_values() {
        let mut driver = MockDriver::new("mock-driver", "mock-endpoint");
        let values = driver.next_values();

        assert_eq!(2, values.len());
        assert_eq!("mock.temperature.001", values[0].tag_id);
        assert_eq!(QualityCode::Simulated, values[0].quality);
    }

    #[test]
    fn mock_driver_accepts_mock_writes() {
        let driver = MockDriver::new("mock-driver", "mock-endpoint");
        let response = driver.write(&DriverWriteRequest {
            command_id: "cmd-1".to_string(),
            tag_id: "mock.running.001".to_string(),
            value: "true".to_string(),
        });

        assert!(response.accepted);
    }
}
