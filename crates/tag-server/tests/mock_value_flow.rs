use driver_manager::ValueNormalizer;
use mock_driver::MockDriver;
use tag_server::InMemoryTagCache;

#[test]
fn mock_values_flow_through_normalizer_into_tag_cache() {
    let mut mock_driver = MockDriver::new("mock-driver", "mock-endpoint");
    let mut normalizer = ValueNormalizer::new(1000, 3000);
    let mut cache = InMemoryTagCache::new();

    let raw_values = mock_driver.next_values();

    for raw_value in raw_values {
        let tag_value = normalizer.normalize(raw_value, "1970-01-01T00:00:01Z");
        assert!(cache.ingest(tag_value));
    }

    assert_eq!(2, cache.len());
    assert!(cache.get("mock.temperature.001").is_some());
    assert!(cache.get("mock.running.001").is_some());
}
