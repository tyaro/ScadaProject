use driver_manager::apply_driver_write_response;
use mock_driver::MockDriver;
use scada_core::command::{ControlCommand, ControlCommandStatus};
use tag_server::WritePolicy;

#[test]
fn write_flows_from_tag_server_policy_to_mock_driver_response() {
    let policy = WritePolicy::new().allow_tag("mock.running.001");
    let driver = MockDriver::new("mock-driver", "mock-endpoint");
    let mut command = ControlCommand::requested(
        "cmd-1",
        "idem-1",
        "operator",
        "mock.running.001",
        "true",
        "1970-01-01T00:00:00Z",
        3000,
    );

    command.transition_to(ControlCommandStatus::Accepted);
    let request = policy.validate(&mut command).expect("validated write");
    command.transition_to(ControlCommandStatus::Sent);
    let response = driver.write(&request);
    apply_driver_write_response(&mut command, &response);

    assert_eq!(ControlCommandStatus::DriverAck, command.status);
}
