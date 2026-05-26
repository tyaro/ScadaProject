pub const ROOT_TOPIC: &str = "scada";

pub fn tag_value_topic(project_id: &str, tag_id: &str) -> String {
    format!("{}/{}/tag/{}/value", ROOT_TOPIC, project_id, tag_id)
}

pub fn tag_quality_topic(project_id: &str, tag_id: &str) -> String {
    format!("{}/{}/tag/{}/quality", ROOT_TOPIC, project_id, tag_id)
}

pub fn alarm_state_topic(project_id: &str, alarm_id: &str) -> String {
    format!("{}/{}/alarm/{}/state", ROOT_TOPIC, project_id, alarm_id)
}

pub fn driver_status_topic(project_id: &str, driver_id: &str) -> String {
    format!("{}/{}/driver/{}/status", ROOT_TOPIC, project_id, driver_id)
}

pub fn system_event_topic(project_id: &str) -> String {
    format!("{}/{}/event/system", ROOT_TOPIC, project_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_value_topic_matches_design() {
        assert_eq!("scada/demo/tag/t1/value", tag_value_topic("demo", "t1"));
    }

    #[test]
    fn driver_status_topic_matches_design() {
        assert_eq!(
            "scada/demo/driver/mock-driver/status",
            driver_status_topic("demo", "mock-driver")
        );
    }
}
