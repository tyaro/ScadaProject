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

    pub fn requested(
        command_id: &str,
        idempotency_key: &str,
        user_id: &str,
        tag_id: &str,
        requested_value: &str,
        requested_at: &str,
        timeout_ms: u64,
    ) -> Self {
        Self {
            command_id: command_id.to_string(),
            idempotency_key: idempotency_key.to_string(),
            user_id: user_id.to_string(),
            tag_id: tag_id.to_string(),
            requested_value: requested_value.to_string(),
            status: ControlCommandStatus::Requested,
            requested_at: requested_at.to_string(),
            timeout_ms,
        }
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
