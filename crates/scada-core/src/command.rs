#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlCommandStatus {
    Requested,
    Accepted,
    Validated,
    Sent,
    DriverAck,
    DeviceAck,
    Verified,
    Timeout,
    Failed,
    Rejected,
}

impl ControlCommandStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Requested => "Requested",
            Self::Accepted => "Accepted",
            Self::Validated => "Validated",
            Self::Sent => "Sent",
            Self::DriverAck => "DriverAck",
            Self::DeviceAck => "DeviceAck",
            Self::Verified => "Verified",
            Self::Timeout => "Timeout",
            Self::Failed => "Failed",
            Self::Rejected => "Rejected",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Verified | Self::Timeout | Self::Failed | Self::Rejected
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlCommand {
    pub command_id: String,
    pub idempotency_key: String,
    pub user_id: String,
    pub tag_id: String,
    pub requested_value: String,
    pub status: ControlCommandStatus,
    pub requested_at: String,
    pub timeout_ms: u64,
}

impl ControlCommand {
    pub fn transition_to(&mut self, status: ControlCommandStatus) {
        self.status = status;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_are_identified() {
        assert!(ControlCommandStatus::Verified.is_terminal());
        assert!(ControlCommandStatus::Failed.is_terminal());
        assert!(!ControlCommandStatus::Sent.is_terminal());
    }
}
