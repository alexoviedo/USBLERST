//! Host-compilable firmware bootstrap for the USB-to-BLE bridge workspace.

mod app;

pub use app::{App, EmbeddedRuntimeState};

#[derive(Debug, Clone)]
struct HostDemoResult {
    boot_profile: usb2ble_core::profile::ProfileId,
    boot_persona: usb2ble_core::profile::OutputPersona,
    boot_descriptor: usb2ble_platform_espidf::ble_hid::BlePersonaDescriptor,
    boot_encoded: usb2ble_platform_espidf::ble_hid::EncodedBleInputReport,
    console_outcome: app::BufferedConsoleOutcome,
    console_tx: Vec<u8>,
    usb_attach_outcome: app::UsbPersonaPumpOutcome,
    usb_descriptor_outcome: app::UsbPersonaPumpOutcome,
    usb_input_outcome: app::UsbPersonaPumpOutcome,
    final_report: usb2ble_core::runtime::GenericBleGamepad16Report,
    final_persona: usb2ble_core::profile::OutputPersona,
    final_encoded: usb2ble_platform_espidf::ble_hid::EncodedBleInputReport,
    last_persona: Option<usb2ble_core::profile::OutputPersona>,
    last_wire: Option<usb2ble_platform_espidf::ble_hid::EncodedBleInputReport>,
}

fn hex_format(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<String>>()
        .join(" ")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReplayCommand {
    Attach {
        device_id: u8,
        vendor_id: u16,
        product_id: u16,
    },
    Descriptor(Vec<u8>),
    Input {
        report_id: u8,
        data: Vec<u8>,
    },
    Detach(u8),
}

#[derive(Debug, Clone)]
struct ReplayResult {
    command_count: usize,
    outcomes: Vec<app::UsbPersonaPumpOutcome>,
    final_persona: usb2ble_core::profile::OutputPersona,
    final_report: usb2ble_core::runtime::GenericBleGamepad16Report,
    final_encoded: usb2ble_platform_espidf::ble_hid::EncodedBleInputReport,
}

fn parse_hex_bytes(parts: &[&str]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    for part in parts {
        if part.len() != 2 {
            return Err(format!("invalid hex byte: {}", part));
        }
        let byte = u8::from_str_radix(part, 16)
            .map_err(|e| format!("invalid hex byte {}: {}", part, e))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn parse_replay_script(script: &str) -> Result<Vec<ReplayCommand>, String> {
    let mut commands = Vec::new();
    for (line_num, line) in script.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "ATTACH" => {
                if parts.len() != 4 {
                    return Err(format!(
                        "line {}: ATTACH requires 3 arguments",
                        line_num + 1
                    ));
                }
                let device_id = parts[1]
                    .parse()
                    .map_err(|e| format!("line {}: invalid device_id: {}", line_num + 1, e))?;
                let vendor_id = parts[2]
                    .parse()
                    .map_err(|e| format!("line {}: invalid vendor_id: {}", line_num + 1, e))?;
                let product_id = parts[3]
                    .parse()
                    .map_err(|e| format!("line {}: invalid product_id: {}", line_num + 1, e))?;
                commands.push(ReplayCommand::Attach {
                    device_id,
                    vendor_id,
                    product_id,
                });
            }
            "DESCRIPTOR" => {
                let bytes = parse_hex_bytes(&parts[1..])
                    .map_err(|e| format!("line {}: {}", line_num + 1, e))?;
                if bytes.len() > 64 {
                    return Err(format!(
                        "line {}: descriptor too long (max 64)",
                        line_num + 1
                    ));
                }
                commands.push(ReplayCommand::Descriptor(bytes));
            }
            "INPUT" => {
                if parts.len() < 2 {
                    return Err(format!(
                        "line {}: INPUT requires report_id and optional data",
                        line_num + 1
                    ));
                }
                let report_id = u8::from_str_radix(parts[1], 16)
                    .map_err(|e| format!("line {}: invalid report_id: {}", line_num + 1, e))?;
                let data = parse_hex_bytes(&parts[2..])
                    .map_err(|e| format!("line {}: {}", line_num + 1, e))?;
                if data.len() > 64 {
                    return Err(format!(
                        "line {}: input data too long (max 64)",
                        line_num + 1
                    ));
                }
                commands.push(ReplayCommand::Input { report_id, data });
            }
            "DETACH" => {
                if parts.len() != 2 {
                    return Err(format!("line {}: DETACH requires 1 argument", line_num + 1));
                }
                let device_id = parts[1]
                    .parse()
                    .map_err(|e| format!("line {}: invalid device_id: {}", line_num + 1, e))?;
                commands.push(ReplayCommand::Detach(device_id));
            }
            other => return Err(format!("line {}: unknown command {}", line_num + 1, other)),
        }
    }
    Ok(commands)
}

#[cfg(not(target_os = "espidf"))]
fn run_replay_host(commands: Vec<ReplayCommand>) -> Result<ReplayResult, String> {
    use usb2ble_platform_espidf::ble_hid::BleConnectionState;
    use usb2ble_platform_espidf::usb_host::{DeviceMeta, UsbDeviceId, UsbEvent};

    let mut runtime = EmbeddedRuntimeState::new_for_host();
    let mut outcomes = Vec::new();

    for cmd in &commands {
        match cmd {
            ReplayCommand::Attach {
                device_id,
                vendor_id,
                product_id,
            } => {
                runtime.queue_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                    device_id: UsbDeviceId::new(*device_id),
                    vendor_id: *vendor_id,
                    product_id: *product_id,
                }));
            }
            ReplayCommand::Descriptor(bytes) => {
                let device_id = runtime
                    .app
                    .active_device()
                    .ok_or("no active device for DESCRIPTOR")?;
                let mut fixed_bytes = [0_u8; 64];
                let len = bytes.len();
                fixed_bytes[..len].copy_from_slice(bytes);
                runtime.queue_usb_event(UsbEvent::ReportDescriptorReceived {
                    device_id,
                    bytes: fixed_bytes,
                    len,
                });
            }
            ReplayCommand::Input { report_id, data } => {
                let device_id = runtime
                    .app
                    .active_device()
                    .ok_or("no active device for INPUT")?;
                let mut fixed_bytes = [0_u8; 64];
                let len = data.len();
                fixed_bytes[..len].copy_from_slice(data);
                runtime.queue_usb_event(UsbEvent::InputReportReceived {
                    device_id,
                    report_id: *report_id,
                    bytes: fixed_bytes,
                    len,
                });
            }
            ReplayCommand::Detach(device_id) => {
                runtime.queue_usb_event(UsbEvent::DeviceDetached(UsbDeviceId::new(*device_id)));
            }
        }

        match runtime.step_persona(BleConnectionState::Connected) {
            Ok(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcomes.push(outcome),
            Ok(app::BufferedPersonaAppPumpOutcome::Idle) => {
                outcomes.push(app::UsbPersonaPumpOutcome::Idle)
            }
            Ok(app::BufferedPersonaAppPumpOutcome::Console(_)) => {
                return Err("unexpected console outcome in replay".to_string())
            }
            Err(e) => return Err(format!("usb pump failed: {:?}", e)),
        }
    }

    Ok(ReplayResult {
        command_count: commands.len(),
        outcomes,
        final_persona: runtime.app.current_output_persona(),
        final_report: runtime.current_report(),
        final_encoded: runtime.app.current_encoded_ble_input_report(),
    })
}

#[cfg(not(target_os = "espidf"))]
fn run_host_demo() -> HostDemoResult {
    use usb2ble_platform_espidf::ble_hid::BleConnectionState;
    use usb2ble_platform_espidf::usb_host::{DeviceMeta, UsbDeviceId, UsbEvent};

    let mut runtime = EmbeddedRuntimeState::new_for_host();
    let boot_info = runtime.boot_info();

    if let Err(e) = runtime.push_console_bytes(b"GET_INFO\n") {
        panic!("demo console push failed: {:?}", e);
    }

    let console_outcome = match runtime.step_persona(BleConnectionState::Connected) {
        Ok(app::BufferedPersonaAppPumpOutcome::Console(outcome)) => outcome,
        other => panic!("expected console outcome, got {:?}", other),
    };
    let console_tx = runtime.console_buffer.tx_bytes().to_vec();

    let device_id = UsbDeviceId::new(101);

    runtime.queue_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
        device_id,
        vendor_id: 1,
        product_id: 2,
    }));
    let usb_attach_outcome = match runtime.step_persona(BleConnectionState::Connected) {
        Ok(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcome,
        other => panic!("expected usb outcome, got {:?}", other),
    };

    let mut descriptor_bytes = [0_u8; 64];
    descriptor_bytes[..18].copy_from_slice(&[
        0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02, 0x09,
        0x31, 0x81, 0x02,
    ]);
    runtime.queue_usb_event(UsbEvent::ReportDescriptorReceived {
        device_id,
        bytes: descriptor_bytes,
        len: 18,
    });
    let usb_descriptor_outcome = match runtime.step_persona(BleConnectionState::Connected) {
        Ok(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcome,
        other => panic!("expected usb outcome, got {:?}", other),
    };

    let mut report_payload = [0_u8; 64];
    report_payload[0] = 0x05;
    report_payload[1] = 0xF6;
    runtime.queue_usb_event(UsbEvent::InputReportReceived {
        device_id,
        report_id: 0,
        bytes: report_payload,
        len: 2,
    });
    let usb_input_outcome = match runtime.step_persona(BleConnectionState::Connected) {
        Ok(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcome,
        other => panic!("expected usb outcome, got {:?}", other),
    };

    HostDemoResult {
        boot_profile: boot_info.active_profile,
        boot_persona: boot_info.output_persona,
        boot_descriptor: boot_info.ble_descriptor,
        boot_encoded: boot_info.initial_encoded_report,
        console_outcome,
        console_tx,
        usb_attach_outcome,
        usb_descriptor_outcome,
        usb_input_outcome,
        final_report: runtime.current_report(),
        final_persona: runtime.app.current_output_persona(),
        final_encoded: runtime.app.current_encoded_ble_input_report(),
        last_persona: runtime.last_persona(),
        last_wire: runtime.last_wire(),
    }
}

fn main() {
    usb2ble_platform_espidf::link_patches_if_needed();

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--embedded-contract") {
        #[cfg(not(target_os = "espidf"))]
        {
            let runtime = EmbeddedRuntimeState::new_for_host();
            let info = runtime.boot_info();

            println!("== usb2ble embedded contract ==");
            println!("profile");
            println!("  {}", info.active_profile.as_str());
            println!("persona");
            println!("  {}", info.output_persona.as_str());
            println!("descriptor");
            println!("  name: {}", info.ble_descriptor.name);
            println!("  report id: 0x{:02X}", info.ble_descriptor.report_id);
            println!("  wire length: {}", info.ble_descriptor.wire_len);
            println!("initial encoded");
            println!("  {}", hex_format(info.initial_encoded_report.as_bytes()));
        }
        #[cfg(target_os = "espidf")]
        {
            println!("embedded contract view is not available on this target yet");
        }
        return;
    }

    if let Some(pos) = args.iter().position(|arg| arg == "--replay-host") {
        #[cfg(not(target_os = "espidf"))]
        {
            if pos + 1 >= args.len() {
                println!("error: --replay-host requires a path");
                return;
            }
            let path = &args[pos + 1];
            let script = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    println!("error reading script file: {}", e);
                    return;
                }
            };

            let commands = match parse_replay_script(&script) {
                Ok(cmds) => cmds,
                Err(e) => {
                    println!("error parsing script: {}", e);
                    return;
                }
            };

            let res = match run_replay_host(commands) {
                Ok(r) => r,
                Err(e) => {
                    println!("error running replay: {}", e);
                    return;
                }
            };

            println!("== usb2ble host replay ==");
            println!("commands");
            println!("  parsed: {}", res.command_count);

            println!("events");
            for outcome in &res.outcomes {
                println!("  outcome: {:?}", outcome);
            }

            println!("final report");
            println!("  persona: {}", res.final_persona.as_str());
            println!("  x: {}", res.final_report.x);
            println!("  y: {}", res.final_report.y);
            println!("  rz: {}", res.final_report.rz);
            println!("  hat: {:?}", res.final_report.hat);
            println!("  buttons: 0x{:04X}", res.final_report.buttons);

            println!("final encoded");
            println!("  wire: {}", hex_format(res.final_encoded.as_bytes()));
        }
        #[cfg(target_os = "espidf")]
        {
            println!("host replay is only available on non-espidf targets");
        }
        return;
    }

    if args.iter().any(|arg| arg == "--demo-host") {
        #[cfg(not(target_os = "espidf"))]
        {
            let res = run_host_demo();
            println!("== usb2ble host demo ==");
            println!("boot");
            println!("  profile: {}", res.boot_profile.as_str());
            println!("  persona: {}", res.boot_persona.as_str());
            println!("  descriptor: {}", res.boot_descriptor.name);
            println!(
                "  initial report: {}",
                hex_format(res.boot_encoded.as_bytes())
            );

            println!("console");
            println!("  outcome: {:?}", res.console_outcome);
            println!(
                "  tx: {}",
                String::from_utf8_lossy(&res.console_tx).trim_end()
            );

            println!("usb attach");
            println!("  outcome: {:?}", res.usb_attach_outcome);

            println!("usb descriptor");
            println!("  outcome: {:?}", res.usb_descriptor_outcome);

            println!("usb input");
            println!("  outcome: {:?}", res.usb_input_outcome);

            println!("final report");
            println!("  x: {}", res.final_report.x);
            println!("  y: {}", res.final_report.y);
            println!("  rz: {}", res.final_report.rz);
            println!("  hat: {:?}", res.final_report.hat);
            println!("  buttons: 0x{:04X}", res.final_report.buttons);

            println!("final encoded");
            println!("  persona: {}", res.final_persona.as_str());
            println!("  wire: {}", hex_format(res.final_encoded.as_bytes()));
            println!("  last persona: {:?}", res.last_persona);
            println!(
                "  last wire: {}",
                res.last_wire
                    .map(|w| hex_format(w.as_bytes()))
                    .unwrap_or_default()
            );
        }
        #[cfg(target_os = "espidf")]
        {
            println!("host demo is only available on non-espidf targets");
        }
        return;
    }

    let bootstrap_result = app::bootstrap_default();

    match bootstrap_result {
        Ok(app_instance) => {
            println!(
                "bootstrap: app={}, core={}, proto={}, platform={}, profile={}",
                app::APP_NAME,
                usb2ble_core::CORE_CRATE_NAME,
                usb2ble_proto::PROTO_CRATE_NAME,
                usb2ble_platform_espidf::PLATFORM_CRATE_NAME,
                app_instance.runtime().active_profile().as_str()
            );
        }
        Err(error) => {
            println!("bootstrap failed: {:?}", error);
        }
    }
}

#[cfg(test)]
mod host_demo_tests {
    use super::*;
    use usb2ble_core::normalize::HatPosition;
    use usb2ble_core::profile::OutputPersona;
    use usb2ble_core::runtime::GenericBleGamepad16Report;
    use usb2ble_platform_espidf::ble_hid::encode_generic_ble_gamepad16_report;

    #[test]
    fn hex_format_returns_expected_string_for_fixed_bytes() {
        let bytes = [0x01, 0x05, 0x00, 0xF6, 0xFF, 0x00, 0x00, 0x08, 0x00, 0x00];
        assert_eq!(hex_format(&bytes), "01 05 00 F6 FF 00 00 08 00 00");
    }

    #[test]
    fn hex_format_handles_empty_slice() {
        assert_eq!(hex_format(&[]), "");
    }

    #[test]
    fn hex_format_pads_single_digit_hex() {
        let bytes = [0x0A, 0x01, 0x0F];
        assert_eq!(hex_format(&bytes), "0A 01 0F");
    }

    #[test]
    fn embedded_contract_view_data_matches_expected_boot_info() {
        let runtime = EmbeddedRuntimeState::new_for_host();
        let info = runtime.boot_info();

        assert_eq!(info.active_profile.as_str(), "t16000m_v1");
        assert_eq!(info.output_persona.as_str(), "generic_ble_gamepad_16");
        assert_eq!(info.ble_descriptor.name, "generic_ble_gamepad_16");
        assert_eq!(info.ble_descriptor.report_id, 0x01);
        assert_eq!(info.ble_descriptor.wire_len, 10);

        let expected_hex = "01 00 00 00 00 00 00 08 00 00";
        assert_eq!(
            hex_format(info.initial_encoded_report.as_bytes()),
            expected_hex
        );
    }

    #[test]
    fn parse_replay_script_ignores_comments_and_blank_lines() {
        let script = r#"
            # comment
            ATTACH 101 1 2

            # another comment
        "#;
        let cmds = match parse_replay_script(script) {
            Ok(c) => c,
            Err(e) => panic!("parse failed: {}", e),
        };
        assert_eq!(cmds.len(), 1);
        assert_eq!(
            cmds[0],
            ReplayCommand::Attach {
                device_id: 101,
                vendor_id: 1,
                product_id: 2
            }
        );
    }

    #[test]
    fn parse_replay_script_parses_attach_descriptor_and_input() {
        let script = r#"
            ATTACH 101 1 2
            DESCRIPTOR 05 01 15 81 25 7F 75 08 95 01 09 30 81 02 09 31 81 02
            INPUT 00 05 F6
        "#;
        let cmds = match parse_replay_script(script) {
            Ok(c) => c,
            Err(e) => panic!("parse failed: {}", e),
        };
        assert_eq!(cmds.len(), 3);
        assert_eq!(
            cmds[0],
            ReplayCommand::Attach {
                device_id: 101,
                vendor_id: 1,
                product_id: 2
            }
        );
        match &cmds[1] {
            ReplayCommand::Descriptor(bytes) => {
                assert_eq!(bytes.len(), 18);
                assert_eq!(bytes[0], 0x05);
            }
            _ => panic!("expected Descriptor"),
        }
        assert_eq!(
            cmds[2],
            ReplayCommand::Input {
                report_id: 0,
                data: vec![0x05, 0xF6]
            }
        );
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn run_replay_script_produces_expected_final_report_and_wire_bytes() {
        let script = r#"
            ATTACH 101 1 2
            DESCRIPTOR 05 01 15 81 25 7F 75 08 95 01 09 30 81 02 09 31 81 02
            INPUT 00 05 F6
        "#;
        let cmds = match parse_replay_script(script) {
            Ok(c) => c,
            Err(e) => panic!("parse failed: {}", e),
        };
        let res = match run_replay_host(cmds) {
            Ok(r) => r,
            Err(e) => panic!("run failed: {}", e),
        };

        let expected_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };

        assert_eq!(res.final_persona, OutputPersona::GenericBleGamepad16);
        assert_eq!(res.final_report, expected_report);
        assert_eq!(
            res.final_encoded.as_bytes(),
            encode_generic_ble_gamepad16_report(expected_report).as_bytes()
        );
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn run_replay_script_supports_detach() {
        let script = r#"
            ATTACH 101 1 2
            DESCRIPTOR 05 01 15 81 25 7F 75 08 95 01 09 30 81 02 09 31 81 02
            INPUT 00 05 F6
            DETACH 101
        "#;
        let cmds = match parse_replay_script(script) {
            Ok(c) => c,
            Err(e) => panic!("parse failed: {}", e),
        };
        let res = match run_replay_host(cmds) {
            Ok(r) => r,
            Err(e) => panic!("run failed: {}", e),
        };

        assert_eq!(res.final_report, GenericBleGamepad16Report::default());
        assert_eq!(
            res.final_encoded.as_bytes(),
            encode_generic_ble_gamepad16_report(GenericBleGamepad16Report::default()).as_bytes()
        );
    }

    #[test]
    fn parse_replay_script_errors_on_unknown_command() {
        let script = "UNKNOWN 1 2 3";
        match parse_replay_script(script) {
            Ok(_) => panic!("expected error"),
            Err(e) => assert!(e.contains("unknown command UNKNOWN")),
        }
    }

    #[test]
    fn parse_replay_script_errors_on_malformed_hex() {
        let script = "DESCRIPTOR 0G";
        match parse_replay_script(script) {
            Ok(_) => panic!("expected error"),
            Err(e) => assert!(e.contains("invalid hex byte 0G")),
        }
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn replay_fixture_xy_input_produces_expected_report_and_wire() {
        let script = include_str!("../../../fixtures/replay/xy_input.txt");
        let cmds = match parse_replay_script(script) {
            Ok(c) => c,
            Err(e) => panic!("parse failed: {}", e),
        };
        let res = match run_replay_host(cmds) {
            Ok(r) => r,
            Err(e) => panic!("run failed: {}", e),
        };

        let expected_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };

        assert_eq!(res.final_persona, OutputPersona::GenericBleGamepad16);
        assert_eq!(res.final_report, expected_report);
        assert_eq!(
            res.final_encoded.as_bytes(),
            encode_generic_ble_gamepad16_report(expected_report).as_bytes()
        );
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn replay_fixture_xy_input_detach_resets_to_default_report() {
        let script = include_str!("../../../fixtures/replay/xy_input_detach.txt");
        let cmds = match parse_replay_script(script) {
            Ok(c) => c,
            Err(e) => panic!("parse failed: {}", e),
        };
        let res = match run_replay_host(cmds) {
            Ok(r) => r,
            Err(e) => panic!("run failed: {}", e),
        };

        assert_eq!(res.final_report, GenericBleGamepad16Report::default());
        assert_eq!(
            res.final_encoded.as_bytes(),
            encode_generic_ble_gamepad16_report(GenericBleGamepad16Report::default()).as_bytes()
        );
    }

    #[test]
    fn replay_fixture_xy_input_has_expected_command_count() {
        let script = include_str!("../../../fixtures/replay/xy_input.txt");
        let cmds = match parse_replay_script(script) {
            Ok(c) => c,
            Err(e) => panic!("parse failed: {}", e),
        };
        assert_eq!(cmds.len(), 3);
    }

    #[test]
    fn replay_fixture_xy_input_detach_has_expected_command_count() {
        let script = include_str!("../../../fixtures/replay/xy_input_detach.txt");
        let cmds = match parse_replay_script(script) {
            Ok(c) => c,
            Err(e) => panic!("parse failed: {}", e),
        };
        assert_eq!(cmds.len(), 4);
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn run_host_demo_produces_expected_gamepad_report() {
        let res = run_host_demo();

        let expected_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };

        assert_eq!(res.final_persona, OutputPersona::GenericBleGamepad16);
        assert_eq!(res.final_report, expected_report);
        assert_eq!(
            res.final_encoded.as_bytes(),
            encode_generic_ble_gamepad16_report(expected_report).as_bytes()
        );

        match res.console_outcome {
            app::BufferedConsoleOutcome::Responded(usb2ble_proto::messages::Response::Info(_)) => {}
            other => panic!("expected Info response, got {:?}", other),
        }
        assert!(!res.console_tx.is_empty());
    }
}
