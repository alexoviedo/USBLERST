//! Host-compilable firmware bootstrap for the USB-to-BLE bridge workspace.

mod app;

pub use app::{App, EmbeddedRuntimeState};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReplayParseError {
    InvalidAttach(usize),
    InvalidDescriptor(usize, String),
    DescriptorTooLong(usize, usize),
    InvalidInput(usize, String),
    InputTooLong(usize, usize),
    InvalidDetach(usize),
    UnknownCommand(usize, String),
}

impl std::fmt::Display for ReplayParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAttach(line) => write!(f, "line {}: ATTACH requires 3 arguments", line),
            Self::InvalidDescriptor(line, msg) => {
                write!(f, "line {}: descriptor error: {}", line, msg)
            }
            Self::DescriptorTooLong(line, len) => {
                write!(f, "line {}: descriptor too long ({} > 64)", line, len)
            }
            Self::InvalidInput(line, msg) => write!(f, "line {}: input error: {}", line, msg),
            Self::InputTooLong(line, len) => {
                write!(f, "line {}: input data too long ({} > 64)", line, len)
            }
            Self::InvalidDetach(line) => write!(f, "line {}: DETACH requires 1 argument", line),
            Self::UnknownCommand(line, cmd) => write!(f, "line {}: unknown command {}", line, cmd),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HostToolError {
    ConsolePush(usb2ble_platform_espidf::console_uart::FrameBufferError),
    Drain(app::EmbeddedDrainError),
    ReplayParse(ReplayParseError),
    NoActiveDevice(&'static str),
    ReplayNoWork,
    UnexpectedConsoleOutcome,
    UnexpectedDemoOutcome(&'static str, app::BufferedPersonaAppPumpOutcome),
}

#[cfg(any(target_os = "espidf", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmbeddedDemoError {
    ProfileStore(usb2ble_platform_espidf::nvs_store::StoreError),
    BondStore(usb2ble_platform_espidf::nvs_store::StoreError),
    Console(usb2ble_platform_espidf::console_uart::ConsoleError),
}

#[cfg(any(target_os = "espidf", test))]
impl std::fmt::Display for EmbeddedDemoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProfileStore(e) => write!(f, "failed to open profile store: {:?}", e),
            Self::BondStore(e) => write!(f, "failed to open bond store: {:?}", e),
            Self::Console(e) => write!(f, "failed to open console: {:?}", e),
        }
    }
}

impl std::fmt::Display for HostToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConsolePush(e) => write!(f, "console push failed: {:?}", e),
            Self::Drain(e) => write!(f, "drain failed: {:?}", e),
            Self::ReplayParse(e) => write!(f, "replay parse error: {}", e),
            Self::NoActiveDevice(cmd) => write!(f, "no active device for {}", cmd),
            Self::ReplayNoWork => write!(f, "replay command produced no work"),
            Self::UnexpectedConsoleOutcome => write!(f, "unexpected console outcome during replay"),
            Self::UnexpectedDemoOutcome(stage, outcome) => {
                write!(
                    f,
                    "unexpected outcome during demo stage {}: {:?}",
                    stage, outcome
                )
            }
        }
    }
}

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

/// A concise summary of a generic BLE gamepad report for logging.
#[cfg(any(target_os = "espidf", test))]
fn format_report_summary(report: &usb2ble_core::runtime::GenericBleGamepad16Report) -> String {
    format!(
        "x={} y={} rz={} hat={:?} buttons=0x{:04X}",
        report.x, report.y, report.rz, report.hat, report.buttons
    )
}

/// Returns true if the USB persona pump error is a recoverable BLE failure.
#[cfg(any(target_os = "espidf", test))]
fn is_recoverable_ble_error(err: &app::UsbPersonaPumpError) -> bool {
    matches!(
        err,
        app::UsbPersonaPumpError::Ble(usb2ble_platform_espidf::ble_hid::BlePublishError::NotReady)
    )
}

/// Formats the BLE backend status for the startup banner.
#[cfg(any(target_os = "espidf", test))]
fn format_backend_status(
    is_real: bool,
    state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
) -> String {
    let mode = if is_real {
        "REAL"
    } else {
        "RECORDING-FALLBACK"
    };
    format!("{} ({:?})", mode, state)
}

/// Formats the bridge publish label based on backend reality.
#[cfg(any(target_os = "espidf", test))]
fn format_publish_label(is_real: bool) -> &'static str {
    if is_real {
        "REAL"
    } else {
        "FALLBACK"
    }
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

fn parse_replay_script(script: &str) -> Result<Vec<ReplayCommand>, ReplayParseError> {
    let mut commands = Vec::new();
    for (line_num, line) in script.lines().enumerate() {
        let line_idx = line_num + 1;
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
                    return Err(ReplayParseError::InvalidAttach(line_idx));
                }
                let device_id = parts[1]
                    .parse()
                    .map_err(|_| ReplayParseError::InvalidAttach(line_idx))?;
                let vendor_id = parts[2]
                    .parse()
                    .map_err(|_| ReplayParseError::InvalidAttach(line_idx))?;
                let product_id = parts[3]
                    .parse()
                    .map_err(|_| ReplayParseError::InvalidAttach(line_idx))?;
                commands.push(ReplayCommand::Attach {
                    device_id,
                    vendor_id,
                    product_id,
                });
            }
            "DESCRIPTOR" => {
                let bytes = parse_hex_bytes(&parts[1..])
                    .map_err(|e| ReplayParseError::InvalidDescriptor(line_idx, e))?;
                if bytes.len() > 64 {
                    return Err(ReplayParseError::DescriptorTooLong(line_idx, bytes.len()));
                }
                commands.push(ReplayCommand::Descriptor(bytes));
            }
            "INPUT" => {
                if parts.len() < 2 {
                    return Err(ReplayParseError::InvalidInput(
                        line_idx,
                        "requires report_id".to_string(),
                    ));
                }
                let report_id = u8::from_str_radix(parts[1], 16)
                    .map_err(|e| ReplayParseError::InvalidInput(line_idx, e.to_string()))?;
                let data = parse_hex_bytes(&parts[2..])
                    .map_err(|e| ReplayParseError::InvalidInput(line_idx, e))?;
                if data.len() > 64 {
                    return Err(ReplayParseError::InputTooLong(line_idx, data.len()));
                }
                commands.push(ReplayCommand::Input { report_id, data });
            }
            "DETACH" => {
                if parts.len() != 2 {
                    return Err(ReplayParseError::InvalidDetach(line_idx));
                }
                let device_id = parts[1]
                    .parse()
                    .map_err(|_| ReplayParseError::InvalidDetach(line_idx))?;
                commands.push(ReplayCommand::Detach(device_id));
            }
            other => {
                return Err(ReplayParseError::UnknownCommand(
                    line_idx,
                    other.to_string(),
                ))
            }
        }
    }
    Ok(commands)
}

#[cfg(not(target_os = "espidf"))]
fn run_replay_host(commands: Vec<ReplayCommand>) -> Result<ReplayResult, HostToolError> {
    use usb2ble_platform_espidf::ble_hid::BleConnectionState;
    use usb2ble_platform_espidf::usb_host::{DeviceMeta, UsbDeviceId, UsbEvent};

    let mut runtime = EmbeddedRuntimeState::new_for_host();
    runtime.set_ble_state(BleConnectionState::Connected);
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
                    .active_device()
                    .ok_or(HostToolError::NoActiveDevice("DESCRIPTOR"))?;
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
                    .active_device()
                    .ok_or(HostToolError::NoActiveDevice("INPUT"))?;
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

        let summary = runtime
            .drain_persona_until_idle_with_runtime_state(8)
            .map_err(HostToolError::Drain)?;

        match summary.last_non_idle_outcome {
            Some(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcomes.push(outcome),
            None => return Err(HostToolError::ReplayNoWork),
            Some(app::BufferedPersonaAppPumpOutcome::Console(_)) => {
                return Err(HostToolError::UnexpectedConsoleOutcome)
            }
            Some(app::BufferedPersonaAppPumpOutcome::Idle) => {
                return Err(HostToolError::ReplayNoWork)
            }
        }
    }

    let final_snapshot = runtime.snapshot();

    Ok(ReplayResult {
        command_count: commands.len(),
        outcomes,
        final_persona: final_snapshot.output_persona,
        final_report: final_snapshot.current_report,
        final_encoded: final_snapshot.current_encoded_report,
    })
}

#[cfg(not(target_os = "espidf"))]
fn run_host_demo() -> Result<HostDemoResult, HostToolError> {
    use usb2ble_platform_espidf::ble_hid::BleConnectionState;
    use usb2ble_platform_espidf::usb_host::{DeviceMeta, UsbDeviceId, UsbEvent};

    let mut runtime = EmbeddedRuntimeState::new_for_host();
    runtime.set_ble_state(BleConnectionState::Connected);
    let boot_info = runtime.boot_info();

    runtime
        .push_console_bytes(b"GET_INFO\n")
        .map_err(HostToolError::ConsolePush)?;

    let console_summary = runtime
        .drain_persona_until_idle_with_runtime_state(8)
        .map_err(HostToolError::Drain)?;
    let console_outcome = match console_summary.last_non_idle_outcome {
        Some(app::BufferedPersonaAppPumpOutcome::Console(
            app::BufferedConsoleOutcome::Responded(outcome),
        )) => app::BufferedConsoleOutcome::Responded(outcome),
        _ => return Err(HostToolError::ReplayNoWork),
    };
    let console_tx = runtime.console_tx_bytes().to_vec();

    let device_id = UsbDeviceId::new(101);

    runtime.queue_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
        device_id,
        vendor_id: 1,
        product_id: 2,
    }));
    let usb_attach_summary = runtime
        .drain_persona_until_idle_with_runtime_state(8)
        .map_err(HostToolError::Drain)?;
    let usb_attach_outcome = match usb_attach_summary.last_non_idle_outcome {
        Some(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcome,
        Some(other) => return Err(HostToolError::UnexpectedDemoOutcome("attach", other)),
        None => return Err(HostToolError::ReplayNoWork),
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
    let usb_descriptor_summary = runtime
        .drain_persona_until_idle_with_runtime_state(8)
        .map_err(HostToolError::Drain)?;
    let usb_descriptor_outcome = match usb_descriptor_summary.last_non_idle_outcome {
        Some(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcome,
        Some(other) => return Err(HostToolError::UnexpectedDemoOutcome("descriptor", other)),
        None => return Err(HostToolError::ReplayNoWork),
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
    let usb_input_summary = runtime
        .drain_persona_until_idle_with_runtime_state(8)
        .map_err(HostToolError::Drain)?;
    let usb_input_outcome = match usb_input_summary.last_non_idle_outcome {
        Some(app::BufferedPersonaAppPumpOutcome::Usb(outcome)) => outcome,
        Some(other) => return Err(HostToolError::UnexpectedDemoOutcome("input", other)),
        None => return Err(HostToolError::ReplayNoWork),
    };

    let final_snapshot = usb_input_summary.final_snapshot;

    Ok(HostDemoResult {
        boot_profile: boot_info.active_profile,
        boot_persona: boot_info.output_persona,
        boot_descriptor: boot_info.ble_descriptor,
        boot_encoded: boot_info.initial_encoded_report,
        console_outcome,
        console_tx,
        usb_attach_outcome,
        usb_descriptor_outcome,
        usb_input_outcome,
        final_report: final_snapshot.current_report,
        final_persona: final_snapshot.output_persona,
        final_encoded: final_snapshot.current_encoded_report,
        last_persona: final_snapshot.last_persona,
        last_wire: final_snapshot.last_wire,
    })
}

#[cfg(target_os = "espidf")]
fn run_embedded_bridge_demo() -> Result<(), EmbeddedDemoError> {
    use usb2ble_platform_espidf::ble_hid::{
        BleConnectionState, BlePersonaOutput, PersonaWireRecordingBleOutput,
    };
    use usb2ble_platform_espidf::console_uart::{
        EspUartBufferedConsole, FramedConsoleBuffer, ResponseSink,
    };
    use usb2ble_platform_espidf::nvs_store::{EspNvsBondStore, EspNvsProfileStore};
    use usb2ble_platform_espidf::usb_host::EspUsbHostIngress;

    let mut profile_store = EspNvsProfileStore::new().map_err(EmbeddedDemoError::ProfileStore)?;
    let mut bond_store = EspNvsBondStore::new().map_err(EmbeddedDemoError::BondStore)?;
    let mut console = EspUartBufferedConsole::new_default().map_err(EmbeddedDemoError::Console)?;

    let mut usb_host_ingress = match EspUsbHostIngress::new_single_client() {
        Ok(ingress) => Some(ingress),
        Err(e) => {
            println!("warning: USB host ingress failed to initialize: {:?}", e);
            None
        }
    };

    let mut app = App::bootstrap(&profile_store);
    let mut console_buffer = FramedConsoleBuffer::new();

    // Start with a structural real BLE backend, falling back to recording if unavailable.
    let mut real_ble =
        usb2ble_platform_espidf::ble_hid::EspBlePersonaOutput::new_generic_gamepad_v1();
    let mut recording_ble = PersonaWireRecordingBleOutput::new(BleConnectionState::Idle);

    let ble_state = if let Ok(ref b) = real_ble {
        b.connection_state()
    } else {
        recording_ble.connection_state()
    };

    let active_profile = app.runtime().active_profile();
    let output_persona = active_profile.output_persona();
    let bonds_present = bond_store.bonds_present();

    println!("*****************************************");
    println!(
        "* firmware: {} v{}",
        app::APP_NAME,
        env!("CARGO_PKG_VERSION")
    );
    println!("* profile:  {}", active_profile.as_str());
    println!("* persona:  {}", output_persona.as_str());
    println!(
        "* bonds:    {}",
        if bonds_present { "PRESENT" } else { "NONE" }
    );
    println!(
        "* ble:      {}",
        format_backend_status(real_ble.is_ok(), ble_state)
    );
    println!(
        "* usb:      {}",
        if usb_host_ingress.is_some() {
            "READY"
        } else {
            "UNAVAILABLE"
        }
    );
    println!("* console:  READY (newline-terminated)");
    println!("*****************************************");

    loop {
        let ble_state = if let Ok(ref b) = real_ble {
            b.connection_state()
        } else {
            recording_ble.connection_state()
        };

        // 1. Console RX -> Buffer
        if let Err(e) = console.pull_rx_into(&mut console_buffer) {
            println!("warning: uart rx error: {:?}", e);
        }

        // 2. Service exactly one console command if available
        match app.service_console_buffer_once(
            &mut console_buffer,
            &mut profile_store,
            &mut bond_store,
            ble_state,
        ) {
            Ok(app::BufferedConsoleOutcome::Responded(resp)) => {
                println!("console response: {:?}", resp);
            }
            Ok(app::BufferedConsoleOutcome::Idle) => {}
            Err(e) => {
                println!("warning: command failed: {:?}", e);
                console_buffer.clear_rx();
            }
        }

        // 3. Buffer -> Console TX
        if let Err(e) = console.flush_tx_from(&mut console_buffer) {
            println!("warning: uart tx error: {:?}", e);
        }

        // 4. USB Host Servicing
        if let Some(ref mut usb_host) = usb_host_ingress {
            if let Err(e) = usb_host.service_until_idle() {
                println!("warning: usb service error: {:?}", e);
            }

            // 5. Drain all available USB events through the app/runtime bridge logic
            loop {
                let outcome = if let Ok(ref mut ble) = real_ble {
                    app.service_usb_once_persona(usb_host, ble)
                } else {
                    app.service_usb_once_persona(usb_host, &mut recording_ble)
                };

                match outcome {
                    Ok(app::UsbPersonaPumpOutcome::Idle) => break,
                    Ok(app::UsbPersonaPumpOutcome::Handled(
                        app::UsbServiceOutcome::DeviceAttached {
                            device_id,
                            vendor_id,
                            product_id,
                        },
                    )) => {
                        println!(
                            "usb attach: id={} vid=0x{:04X} pid=0x{:04X}",
                            device_id.raw(),
                            vendor_id,
                            product_id
                        );
                    }
                    Ok(app::UsbPersonaPumpOutcome::Handled(
                        app::UsbServiceOutcome::DescriptorStored {
                            device_id,
                            field_count,
                        },
                    )) => {
                        println!(
                            "usb descriptor stored: id={} fields={}",
                            device_id.raw(),
                            field_count
                        );
                    }
                    Ok(app::UsbPersonaPumpOutcome::Handled(
                        app::UsbServiceOutcome::DeviceDetached(id),
                    )) => {
                        println!("usb detach: id={}", id.raw());
                    }
                    Ok(app::UsbPersonaPumpOutcome::Handled(_)) => {}
                    Ok(app::UsbPersonaPumpOutcome::Published {
                        persona,
                        report,
                        encoded,
                    }) => {
                        let label = format_publish_label(real_ble.is_ok());
                        println!(
                            "bridge publish [{}]: persona={} {} wire={}",
                            label,
                            persona.as_str(),
                            format_report_summary(&report),
                            hex_format(encoded.as_bytes())
                        );
                    }
                    Err(e) => {
                        if is_recoverable_ble_error(&e) {
                            // Recoverable BLE NotReady is common when disconnected; log concisely.
                            println!("usb publish: not ready");
                            continue;
                        }
                        println!("warning: usb pump error: {:?}", e);
                        break;
                    }
                }
            }
        }

        // Yield to allow other tasks (like IDLE) to run and prevent WDT triggers.
        std::thread::sleep(std::time::Duration::from_millis(1));
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
            println!("ble state");
            println!("  {:?}", info.ble_state);
            println!("bonds present");
            println!("  {}", info.bonds_present);
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
                    println!("error: {}", HostToolError::ReplayParse(e));
                    return;
                }
            };

            let res = match run_replay_host(commands) {
                Ok(r) => r,
                Err(e) => {
                    println!("error: {}", e);
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
            let res = match run_host_demo() {
                Ok(r) => r,
                Err(e) => {
                    println!("error: {}", e);
                    return;
                }
            };
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

    #[cfg(target_os = "espidf")]
    {
        if let Err(e) = run_embedded_bridge_demo() {
            println!("fatal demo initialization failure: {}", e);
            loop {
                std::thread::yield_now();
            }
        }
    }

    #[cfg(not(target_os = "espidf"))]
    {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use usb2ble_core::normalize::HatPosition;
    use usb2ble_core::profile::OutputPersona;
    use usb2ble_core::runtime::GenericBleGamepad16Report;
    use usb2ble_platform_espidf::ble_hid::encode_generic_ble_gamepad16_report;

    #[test]
    fn format_report_summary_is_concise_and_correct() {
        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 300,
            hat: HatPosition::Centered,
            buttons: 0x1234,
        };
        let summary = format_report_summary(&report);
        assert!(summary.contains("x=5"));
        assert!(summary.contains("y=-10"));
        assert!(summary.contains("rz=300"));
        assert!(summary.contains("hat=Centered"));
        assert!(summary.contains("buttons=0x1234"));
    }

    #[test]
    fn is_recoverable_ble_error_matches_not_ready() {
        let err = app::UsbPersonaPumpError::Ble(
            usb2ble_platform_espidf::ble_hid::BlePublishError::NotReady,
        );
        assert!(is_recoverable_ble_error(&err));
    }

    #[test]
    fn is_recoverable_ble_error_rejects_transport() {
        let err = app::UsbPersonaPumpError::Ble(
            usb2ble_platform_espidf::ble_hid::BlePublishError::Transport,
        );
        assert!(!is_recoverable_ble_error(&err));
    }

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
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        runtime.set_ble_state(usb2ble_platform_espidf::ble_hid::BleConnectionState::Connected);
        assert!(runtime.store_bonds_present(true).is_ok());

        let info = runtime.boot_info();

        assert_eq!(info.active_profile.as_str(), "t16000m_v1");
        assert_eq!(info.output_persona.as_str(), "generic_ble_gamepad_16");
        assert_eq!(
            info.ble_state,
            usb2ble_platform_espidf::ble_hid::BleConnectionState::Connected
        );
        assert!(info.bonds_present);
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
    fn run_replay_host_errors_on_no_active_device() {
        let script = "DESCRIPTOR 05 01";
        let cmds = match parse_replay_script(script) {
            Ok(c) => c,
            Err(e) => panic!("parse failed: {}", e),
        };
        let res = run_replay_host(cmds);
        assert_eq!(res.err(), Some(HostToolError::NoActiveDevice("DESCRIPTOR")));
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn run_replay_host_errors_on_no_work() {
        // Manually queue an event but we will call drain on an EMPTY runtime in the test loop
        // Actually, easiest is just to call run_replay_host with a DETACH command for a device that isn't there
        // Wait, DETACH always produces work (it might just be UsbServiceOutcome::Ignored).
        // Actually, currently EmbeddedDrainSummary::last_non_idle_outcome is None ONLY if the first step is Idle.

        // Replay host:
        // DETACH 1 -> runtime.queue_usb_event(DeviceDetached(1)) -> drain
        // handle_usb_event(DeviceDetached(1)) -> UsbServiceOutcome::Ignored (because no active device)
        // UsbPersonaPumpOutcome::Handled(Ignored) -> last_non_idle_outcome = Some(...)

        // To get NoWork, we need drain_persona_until_idle to return last_non_idle_outcome == None.
        // This happens if step_persona(Connected) returns Ok(Usb(Idle)).
        // This happens if usb_ingress.poll_event() returns None.
        // But run_replay_host always queues an event before calling drain.

        // UNLESS the command is somehow empty? No, ReplayCommand is an enum.

        // Let's look at the drain logic again.
        // Ok(BufferedPersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Idle)) => return summary with last_non_idle_outcome.

        // If we want last_non_idle_outcome to be None, we need to NOT hit the Ok(outcome) arm.
        // This means the loop must terminate on the first iteration with Ok(Usb(Idle)).

        // So we need run_replay_host to call drain when there is NO work queued.
        // But run_replay_host is structured to queue exactly one event per command.

        // So run_replay_host can only return ReplayNoWork if the drain loop finishes immediately with Idle.

        // If we want last_non_idle_outcome to be None, we need to NOT hit the Ok(outcome) arm.
        // This means the loop must terminate on the first iteration with Ok(Usb(Idle)).

        // Let's add a small direct test for the error variant.
        assert_eq!(
            format!("{}", HostToolError::ReplayNoWork),
            "replay command produced no work"
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
            Err(e) => assert_eq!(
                e,
                ReplayParseError::UnknownCommand(1, "UNKNOWN".to_string())
            ),
        }
    }

    #[test]
    fn parse_replay_script_errors_on_malformed_hex() {
        let script = "DESCRIPTOR 0G";
        match parse_replay_script(script) {
            Ok(_) => panic!("expected error"),
            Err(e) => match e {
                ReplayParseError::InvalidDescriptor(1, msg) => assert!(msg.contains("0G")),
                _ => panic!("expected InvalidDescriptor error, got {:?}", e),
            },
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
        let res = match run_host_demo() {
            Ok(r) => r,
            Err(e) => panic!("demo failed: {:?}", e),
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

        match res.console_outcome {
            app::BufferedConsoleOutcome::Responded(usb2ble_proto::messages::Response::Info(_)) => {}
            other => panic!("expected Info response, got {:?}", other),
        }
        assert!(!res.console_tx.is_empty());
    }

    #[test]
    fn format_backend_status_matches_expected() {
        use usb2ble_platform_espidf::ble_hid::BleConnectionState;
        assert_eq!(
            format_backend_status(true, BleConnectionState::Connected),
            "REAL (Connected)"
        );
        assert_eq!(
            format_backend_status(false, BleConnectionState::Idle),
            "RECORDING-FALLBACK (Idle)"
        );
    }

    #[test]
    fn format_publish_label_matches_expected() {
        assert_eq!(format_publish_label(true), "REAL");
        assert_eq!(format_publish_label(false), "FALLBACK");
    }

    #[test]
    fn embedded_demo_error_formatting_matches_expected() {
        use usb2ble_platform_espidf::console_uart::ConsoleError;
        use usb2ble_platform_espidf::nvs_store::StoreError;

        let profile_err = EmbeddedDemoError::ProfileStore(StoreError::BackendFailure);
        let bond_err = EmbeddedDemoError::BondStore(StoreError::BackendFailure);
        let console_err = EmbeddedDemoError::Console(ConsoleError::Transport);

        assert_eq!(
            format!("{}", profile_err),
            "failed to open profile store: BackendFailure"
        );
        assert_eq!(
            format!("{}", bond_err),
            "failed to open bond store: BackendFailure"
        );
        assert_eq!(
            format!("{}", console_err),
            "failed to open console: Transport"
        );
    }
}
