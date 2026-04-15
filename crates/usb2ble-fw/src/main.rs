//! Host-compilable firmware bootstrap for the USB-to-BLE bridge workspace.

mod app;

pub use app::App;

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

#[cfg(not(target_os = "espidf"))]
fn run_host_demo() -> HostDemoResult {
    use usb2ble_platform_espidf::ble_hid::{BleConnectionState, PersonaWireRecordingBleOutput};
    use usb2ble_platform_espidf::console_uart::FramedConsoleBuffer;
    use usb2ble_platform_espidf::nvs_store::{MemoryBondStore, MemoryProfileStore};
    use usb2ble_platform_espidf::usb_host::{DeviceMeta, QueuedUsbIngress, UsbDeviceId, UsbEvent};

    let mut app_instance = match app::bootstrap_default() {
        Ok(app) => app,
        Err(e) => panic!("demo bootstrap failed: {:?}", e),
    };

    let mut console_buffer = FramedConsoleBuffer::new();
    let mut profile_store = MemoryProfileStore::new();
    let mut bond_store = MemoryBondStore::new();
    let mut usb_ingress = QueuedUsbIngress::new();
    let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

    let boot_profile = app_instance.runtime().active_profile();
    let boot_persona = app_instance.current_output_persona();
    let boot_descriptor = app_instance.current_ble_persona_descriptor();
    let boot_encoded = app_instance.current_encoded_ble_input_report();

    if let Err(e) = console_buffer.push_rx_bytes(b"GET_INFO\n") {
        panic!("demo console push failed: {:?}", e);
    }

    let console_outcome = match app_instance.service_once_with_console_buffer_persona(
        &mut console_buffer,
        &mut profile_store,
        &mut bond_store,
        BleConnectionState::Connected,
        &mut usb_ingress,
        &mut ble_output,
    ) {
        Ok(app::BufferedPersonaAppPumpOutcome::Console(outcome)) => outcome,
        other => panic!("expected console outcome, got {:?}", other),
    };
    let console_tx = console_buffer.tx_bytes().to_vec();

    let device_id = UsbDeviceId::new(101);

    usb_ingress.queue_event(UsbEvent::DeviceAttached(DeviceMeta {
        device_id,
        vendor_id: 1,
        product_id: 2,
    }));
    let usb_attach_outcome = match app_instance.service_once_with_console_buffer_persona(
        &mut console_buffer,
        &mut profile_store,
        &mut bond_store,
        BleConnectionState::Connected,
        &mut usb_ingress,
        &mut ble_output,
    ) {
        Ok(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcome,
        other => panic!("expected usb outcome, got {:?}", other),
    };

    let mut descriptor_bytes = [0_u8; 64];
    descriptor_bytes[..18].copy_from_slice(&[
        0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02, 0x09,
        0x31, 0x81, 0x02,
    ]);
    usb_ingress.queue_event(UsbEvent::ReportDescriptorReceived {
        device_id,
        bytes: descriptor_bytes,
        len: 18,
    });
    let usb_descriptor_outcome = match app_instance.service_once_with_console_buffer_persona(
        &mut console_buffer,
        &mut profile_store,
        &mut bond_store,
        BleConnectionState::Connected,
        &mut usb_ingress,
        &mut ble_output,
    ) {
        Ok(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcome,
        other => panic!("expected usb outcome, got {:?}", other),
    };

    let mut report_payload = [0_u8; 64];
    report_payload[0] = 0x05;
    report_payload[1] = 0xF6;
    usb_ingress.queue_event(UsbEvent::InputReportReceived {
        device_id,
        report_id: 0,
        bytes: report_payload,
        len: 2,
    });
    let usb_input_outcome = match app_instance.service_once_with_console_buffer_persona(
        &mut console_buffer,
        &mut profile_store,
        &mut bond_store,
        BleConnectionState::Connected,
        &mut usb_ingress,
        &mut ble_output,
    ) {
        Ok(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcome,
        other => panic!("expected usb outcome, got {:?}", other),
    };

    HostDemoResult {
        boot_profile,
        boot_persona,
        boot_descriptor,
        boot_encoded,
        console_outcome,
        console_tx,
        usb_attach_outcome,
        usb_descriptor_outcome,
        usb_input_outcome,
        final_report: app_instance.runtime().current_report(),
        final_persona: app_instance.current_output_persona(),
        final_encoded: app_instance.current_encoded_ble_input_report(),
        last_persona: ble_output.last_persona(),
        last_wire: ble_output.last_wire(),
    }
}

fn main() {
    usb2ble_platform_espidf::link_patches_if_needed();

    let args: Vec<String> = std::env::args().collect();
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
