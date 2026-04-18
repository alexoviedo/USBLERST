//! Application-layer coordinator for the host-compilable lean v1 firmware.

/// The stable firmware application name.
pub const APP_NAME: &str = "usb2ble-fw";

use usb2ble_platform_espidf::nvs_store::BondStore;

/// The deterministic lean v1 firmware coordinator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    runtime: usb2ble_core::runtime::RuntimeState,
    active_device: Option<usb2ble_platform_espidf::usb_host::UsbDeviceId>,
    active_descriptor: Option<usb2ble_core::hid_descriptor::ReportDescriptorSummary>,
}

/// The result of servicing at most one console command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceOutcome {
    /// No command was available.
    Idle,
    /// One command was handled and its response was sent.
    Responded(usb2ble_proto::messages::Response),
}

/// Errors that can occur while servicing the console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceError {
    /// The console response path failed.
    Console(usb2ble_platform_espidf::console_uart::ConsoleError),
}

/// The result of servicing at most one framed console command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferedConsoleOutcome {
    /// No complete newline-terminated frame was available yet.
    Idle,
    /// One framed command was handled and its response was queued.
    Responded(usb2ble_proto::messages::Response),
}

/// Errors that can occur while servicing the framed console buffer path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferedConsoleError {
    /// The framed console buffer failed while decoding or queueing.
    Buffer(usb2ble_platform_espidf::console_uart::FrameBufferError),
}

/// Errors that can occur while servicing one raw protocol frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameServiceError {
    /// The input frame could not be decoded into a typed command.
    Decode(usb2ble_proto::framing::FrameError),
    /// The typed response could not be encoded into a wire frame.
    Encode(usb2ble_proto::framing::FrameError),
}

/// The result of servicing one USB-side event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbServiceOutcome {
    /// The event did not apply to the currently active device or state.
    Ignored,
    /// A device became the active USB source.
    DeviceAttached {
        /// The unique device identifier.
        device_id: usb2ble_platform_espidf::usb_host::UsbDeviceId,
        /// The vendor ID of the device.
        vendor_id: u16,
        /// The product ID of the device.
        product_id: u16,
    },
    /// A descriptor was parsed and stored for the active device.
    DescriptorStored {
        /// The descriptor source device.
        device_id: usb2ble_platform_espidf::usb_host::UsbDeviceId,
        /// The number of parsed fields stored in the summary.
        field_count: usize,
    },
    /// One input report was decoded, normalized, and applied to runtime state.
    InputApplied(usb2ble_core::runtime::GenericBleGamepad16Report),
    /// The active USB device was detached.
    DeviceDetached(usb2ble_platform_espidf::usb_host::UsbDeviceId),
}

/// Errors that can occur while servicing one USB-side event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbServiceError {
    /// A USB event reported a buffer length larger than the fixed event buffer.
    InvalidBufferLength {
        /// The invalid reported length.
        len: usize,
        /// The maximum supported fixed buffer length.
        max: usize,
    },
    /// Parsing a report descriptor failed.
    DescriptorParse(usb2ble_core::hid_descriptor::DescriptorParseError),
    /// Decoding an input report failed.
    ReportDecode(usb2ble_core::hid_decode::DecodeError),
    /// Normalizing a decoded report failed.
    Normalize(usb2ble_core::normalize::NormalizeError),
}

/// The result of servicing at most one USB ingress event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbPumpOutcome {
    /// No USB event was available.
    Idle,
    /// One non-publishing USB event was handled.
    Handled(UsbServiceOutcome),
    /// One input report was handled and the current BLE report was published.
    Published(usb2ble_core::runtime::GenericBleGamepad16Report),
}

/// Errors that can occur while servicing at most one USB ingress event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbPumpError {
    /// Handling the USB event failed before BLE publication.
    Usb(UsbServiceError),
    /// Publishing the current BLE report failed.
    Ble(usb2ble_platform_espidf::ble_hid::BlePublishError),
}

/// The result of servicing at most one persona-oriented USB ingress event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbPersonaPumpOutcome {
    /// No USB event was available.
    Idle,
    /// One non-publishing USB event was handled.
    Handled(UsbServiceOutcome),
    /// One input report was handled and the current persona-encoded BLE report was published.
    Published {
        /// The decoded and normalized input report.
        report: usb2ble_core::runtime::GenericBleGamepad16Report,
        /// The active output persona used for encoding.
        persona: usb2ble_core::profile::OutputPersona,
        /// The persona-encoded BLE report that was published.
        encoded: usb2ble_platform_espidf::ble_hid::EncodedBleInputReport,
    },
}

/// Errors that can occur while servicing at most one persona-oriented USB ingress event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbPersonaPumpError {
    /// Handling the USB event failed before BLE publication.
    Usb(UsbServiceError),
    /// Publishing the persona-encoded BLE report failed.
    Ble(usb2ble_platform_espidf::ble_hid::BlePublishError),
}

/// The result of servicing at most one top-level application action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPumpOutcome {
    /// Neither console nor USB had work to do.
    Idle,
    /// One console action was serviced.
    Console(ServiceOutcome),
    /// One USB-side action was serviced.
    Usb(UsbPumpOutcome),
}

/// Errors that can occur while servicing at most one top-level application action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPumpError {
    /// Console-side servicing failed.
    Console(ServiceError),
    /// USB-side servicing failed.
    Usb(UsbPumpError),
}

/// The result of servicing at most one top-level buffered-console or USB action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferedPersonaAppPumpOutcome {
    /// Neither buffered console nor USB had work to do.
    Idle,
    /// One buffered console action was serviced.
    Console(BufferedConsoleOutcome),
    /// One USB-side action was serviced.
    Usb(UsbPersonaPumpOutcome),
}

/// Errors that can occur while servicing at most one buffered-console or USB action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferedPersonaAppPumpError {
    /// Buffered console-side servicing failed.
    Console(BufferedConsoleError),
    /// USB-side servicing failed.
    Usb(UsbPersonaPumpError),
}

/// The result of servicing at most one top-level console or USB action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonaAppPumpOutcome {
    /// Neither console nor USB had work to do.
    Idle,
    /// One console action was serviced.
    Console(ServiceOutcome),
    /// One USB-side action was serviced.
    Usb(UsbPersonaPumpOutcome),
}

/// Errors that can occur while servicing at most one console or USB action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonaAppPumpError {
    /// Console-side servicing failed.
    Console(ServiceError),
    /// USB-side servicing failed.
    Usb(UsbPersonaPumpError),
}

/// The result of servicing at most one top-level buffered-console or USB action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferedAppPumpOutcome {
    /// Neither buffered console nor USB had work to do.
    Idle,
    /// One buffered console action was serviced.
    Console(BufferedConsoleOutcome),
    /// One USB-side action was serviced.
    Usb(UsbPumpOutcome),
}

/// Errors that can occur while servicing at most one buffered-console or USB action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferedAppPumpError {
    /// Buffered console-side servicing failed.
    Console(BufferedConsoleError),
    /// USB-side servicing failed.
    Usb(UsbPumpError),
}

/// Errors reserved for future stricter bootstrap policy.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapError {
    /// The platform profile store could not be opened.
    UnavailableProfileStore,
}

/// Embedded-facing runtime state for deterministic host testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRuntimeState {
    /// The core application coordinator.
    pub app: App,
    /// The in-memory profile store.
    pub profile_store: usb2ble_platform_espidf::nvs_store::MemoryProfileStore,
    /// The in-memory bond store.
    pub bond_store: usb2ble_platform_espidf::nvs_store::MemoryBondStore,
    /// The framed console buffer.
    pub console_buffer: usb2ble_platform_espidf::console_uart::FramedConsoleBuffer,
    /// The queued USB ingress source.
    pub usb_ingress: usb2ble_platform_espidf::usb_host::QueuedUsbIngress,
    /// The persona-wire recording BLE output sink.
    pub ble_output: usb2ble_platform_espidf::ble_hid::PersonaWireRecordingBleOutput,
    /// The current BLE link state.
    pub ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
}

impl EmbeddedRuntimeState {
    /// Bootstraps a new embedded-facing runtime state for host testing.
    pub fn new_for_host() -> Self {
        let profile_store = usb2ble_platform_espidf::nvs_store::MemoryProfileStore::new();
        let app = App::bootstrap(&profile_store);
        let bond_store = usb2ble_platform_espidf::nvs_store::MemoryBondStore::new();
        let console_buffer = usb2ble_platform_espidf::console_uart::FramedConsoleBuffer::new();
        let usb_ingress = usb2ble_platform_espidf::usb_host::QueuedUsbIngress::new();
        let ble_output = usb2ble_platform_espidf::ble_hid::PersonaWireRecordingBleOutput::new(
            usb2ble_platform_espidf::ble_hid::BleConnectionState::Idle,
        );
        let ble_state = usb2ble_platform_espidf::ble_hid::BleConnectionState::Idle;

        Self {
            app,
            profile_store,
            bond_store,
            console_buffer,
            usb_ingress,
            ble_output,
            ble_state,
        }
    }

    /// Returns the current BLE link state.
    pub fn ble_state(&self) -> usb2ble_platform_espidf::ble_hid::BleConnectionState {
        self.ble_state
    }

    /// Updates the stored BLE link state.
    pub fn set_ble_state(
        &mut self,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
    ) {
        self.ble_state = ble_state;
    }

    /// Returns whether any persisted bonds are present.
    pub fn bonds_present(&self) -> bool {
        self.bond_store.bonds_present()
    }

    /// Persists whether bonds are present.
    pub fn store_bonds_present(
        &mut self,
        bonds_present: bool,
    ) -> Result<(), usb2ble_platform_espidf::nvs_store::StoreError> {
        self.bond_store.store_bonds_present(bonds_present)
    }

    /// Returns the current typed device status for the owned state.
    pub fn device_status(&self) -> usb2ble_proto::messages::DeviceStatus {
        self.app.device_status(self.ble_state, &self.bond_store)
    }

    /// Returns a snapshot of the current boot and contract information.
    pub fn boot_info(&self) -> EmbeddedBootInfo {
        EmbeddedBootInfo {
            active_profile: self.app.runtime().active_profile(),
            output_persona: self.app.current_output_persona(),
            ble_descriptor: self.app.current_ble_persona_descriptor(),
            initial_encoded_report: self.app.current_encoded_ble_input_report(),
            ble_state: self.ble_state,
            bonds_present: self.bonds_present(),
        }
    }

    /// Services at most one embedded action using the persona-oriented pump.
    pub fn step_persona(
        &mut self,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
    ) -> Result<BufferedPersonaAppPumpOutcome, BufferedPersonaAppPumpError> {
        self.app.service_once_with_console_buffer_persona(
            &mut self.console_buffer,
            &mut self.profile_store,
            &mut self.bond_store,
            ble_state,
            &mut self.usb_ingress,
            &mut self.ble_output,
        )
    }

    /// Queues a USB event into the runtime ingress.
    pub fn queue_usb_event(&mut self, event: usb2ble_platform_espidf::usb_host::UsbEvent) {
        self.usb_ingress.queue_event(event);
    }

    /// Pushes raw bytes into the framed console buffer.
    pub fn push_console_bytes(
        &mut self,
        input: &[u8],
    ) -> Result<(), usb2ble_platform_espidf::console_uart::FrameBufferError> {
        self.console_buffer.push_rx_bytes(input)
    }

    /// Returns the current normalized input report.
    pub fn current_report(&self) -> usb2ble_core::runtime::GenericBleGamepad16Report {
        self.app.runtime().current_report()
    }

    /// Returns the last published output persona, if any.
    pub fn last_persona(&self) -> Option<usb2ble_core::profile::OutputPersona> {
        self.ble_output.last_persona()
    }

    /// Returns the last published persona-encoded wire report, if any.
    pub fn last_wire(&self) -> Option<usb2ble_platform_espidf::ble_hid::EncodedBleInputReport> {
        self.ble_output.last_wire()
    }

    /// Returns a full snapshot of the current runtime state.
    pub fn snapshot(&self) -> EmbeddedRuntimeSnapshot {
        EmbeddedRuntimeSnapshot {
            active_profile: self.app.runtime().active_profile(),
            output_persona: self.app.current_output_persona(),
            current_report: self.app.runtime().current_report(),
            current_encoded_report: self.app.current_encoded_ble_input_report(),
            last_persona: self.ble_output.last_persona(),
            last_wire: self.ble_output.last_wire(),
            ble_state: self.ble_state,
            bonds_present: self.bonds_present(),
        }
    }

    /// Services one embedded action and returns the outcome and updated state.
    pub fn step_persona_snapshot(
        &mut self,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
    ) -> EmbeddedStepSnapshot {
        let outcome = self.step_persona(ble_state);
        let runtime = self.snapshot();

        EmbeddedStepSnapshot { outcome, runtime }
    }

    /// Returns the currently active USB device, if any.
    pub fn active_device(&self) -> Option<usb2ble_platform_espidf::usb_host::UsbDeviceId> {
        self.app.active_device()
    }

    /// Returns the valid queued TX bytes from the console buffer.
    pub fn console_tx_bytes(&self) -> &[u8] {
        self.console_buffer.tx_bytes()
    }

    /// Services at most one embedded action using the owned BLE link state.
    pub fn step_persona_with_runtime_state(
        &mut self,
    ) -> Result<BufferedPersonaAppPumpOutcome, BufferedPersonaAppPumpError> {
        self.step_persona(self.ble_state)
    }

    /// Repeatedly services the runtime using the owned BLE link state until idle.
    pub fn drain_persona_until_idle_with_runtime_state(
        &mut self,
        max_steps: usize,
    ) -> Result<EmbeddedDrainSummary, EmbeddedDrainError> {
        self.drain_persona_until_idle(self.ble_state, max_steps)
    }

    /// Repeatedly services the runtime until an idle state or step limit is reached.
    pub fn drain_persona_until_idle(
        &mut self,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
        max_steps: usize,
    ) -> Result<EmbeddedDrainSummary, EmbeddedDrainError> {
        let mut actions_processed = 0;
        let mut last_non_idle_outcome = None;

        loop {
            let snapshot = self.step_persona_snapshot(ble_state);

            match snapshot.outcome {
                Ok(BufferedPersonaAppPumpOutcome::Idle)
                | Ok(BufferedPersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Idle)) => {
                    return Ok(EmbeddedDrainSummary {
                        actions_processed,
                        last_non_idle_outcome,
                        final_snapshot: snapshot.runtime,
                    });
                }
                Ok(outcome) => {
                    actions_processed += 1;
                    last_non_idle_outcome = Some(outcome);
                }
                Err(error) => {
                    return Err(EmbeddedDrainError::Step(error));
                }
            }

            if actions_processed >= max_steps {
                return Err(EmbeddedDrainError::StepLimitReached {
                    max_steps,
                    last_snapshot: self.snapshot(),
                });
            }
        }
    }
}

/// A snapshot of the runtime state for observation and testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedRuntimeSnapshot {
    /// The active profile ID.
    pub active_profile: usb2ble_core::profile::ProfileId,
    /// The implied output persona.
    pub output_persona: usb2ble_core::profile::OutputPersona,
    /// The current normalized input report.
    pub current_report: usb2ble_core::runtime::GenericBleGamepad16Report,
    /// The current persona-encoded BLE report.
    pub current_encoded_report: usb2ble_platform_espidf::ble_hid::EncodedBleInputReport,
    /// The last published output persona, if any.
    pub last_persona: Option<usb2ble_core::profile::OutputPersona>,
    /// The last published persona-encoded wire report, if any.
    pub last_wire: Option<usb2ble_platform_espidf::ble_hid::EncodedBleInputReport>,
    /// The BLE link state in the snapshot.
    pub ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
    /// Whether bonds were present in the snapshot.
    pub bonds_present: bool,
}

/// The result of one embedded runtime step together with the updated state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedStepSnapshot {
    /// The outcome of the step.
    pub outcome: Result<BufferedPersonaAppPumpOutcome, BufferedPersonaAppPumpError>,
    /// The updated runtime state snapshot after the step.
    pub runtime: EmbeddedRuntimeSnapshot,
}

/// A summary of processing actions until an idle state was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedDrainSummary {
    /// The number of non-idle actions processed.
    pub actions_processed: usize,
    /// The last non-idle successful outcome processed.
    pub last_non_idle_outcome: Option<BufferedPersonaAppPumpOutcome>,
    /// The final runtime state snapshot.
    pub final_snapshot: EmbeddedRuntimeSnapshot,
}

/// Errors that can occur while draining the embedded runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedDrainError {
    /// A single step failed.
    Step(BufferedPersonaAppPumpError),
    /// The maximum number of allowed steps was reached before idle.
    StepLimitReached {
        /// The step limit that was reached.
        max_steps: usize,
        /// The last observed runtime state snapshot.
        last_snapshot: EmbeddedRuntimeSnapshot,
    },
}

/// Reusable boot and contract information for the embedded runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedBootInfo {
    /// The active profile ID at boot.
    pub active_profile: usb2ble_core::profile::ProfileId,
    /// The implied output persona at boot.
    pub output_persona: usb2ble_core::profile::OutputPersona,
    /// The BLE persona descriptor for the active persona.
    pub ble_descriptor: usb2ble_platform_espidf::ble_hid::BlePersonaDescriptor,
    /// The initial encoded BLE report for the active persona.
    pub initial_encoded_report: usb2ble_platform_espidf::ble_hid::EncodedBleInputReport,
    /// The BLE link state at boot.
    pub ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
    /// Whether bonds were present at boot.
    pub bonds_present: bool,
}

/// Bootstraps the application with the default host-side profile store.
#[cfg(not(target_os = "espidf"))]
pub fn bootstrap_default() -> Result<App, BootstrapError> {
    let store = usb2ble_platform_espidf::nvs_store::MemoryProfileStore::new();

    Ok(App::bootstrap(&store))
}

/// Bootstraps the application with the default ESP-IDF profile store, falling back safely.
#[cfg(target_os = "espidf")]
pub fn bootstrap_default() -> Result<App, BootstrapError> {
    match usb2ble_platform_espidf::nvs_store::EspNvsProfileStore::new() {
        Ok(store) => Ok(App::bootstrap(&store)),
        Err(_) => Ok(App::new(usb2ble_core::profile::V1_PROFILE_ID)),
    }
}

impl App {
    /// Creates an application coordinator with the requested active profile.
    pub fn new(active_profile: usb2ble_core::profile::ProfileId) -> Self {
        Self {
            runtime: usb2ble_core::runtime::RuntimeState::new(active_profile),
            active_device: None,
            active_descriptor: None,
        }
    }

    /// Boots the application coordinator from persisted profile state.
    pub fn bootstrap(
        profile_store: &impl usb2ble_platform_espidf::nvs_store::ProfileStore,
    ) -> Self {
        let active_profile = match profile_store.load_active_profile() {
            Some(profile) => profile,
            None => usb2ble_core::profile::V1_PROFILE_ID,
        };

        Self::new(active_profile)
    }

    /// Returns the current runtime state snapshot.
    pub fn runtime(&self) -> usb2ble_core::runtime::RuntimeState {
        self.runtime
    }

    /// Returns the currently active USB device, if one is attached.
    pub fn active_device(&self) -> Option<usb2ble_platform_espidf::usb_host::UsbDeviceId> {
        self.active_device
    }

    /// Returns the currently stored active report descriptor summary, if any.
    pub fn active_descriptor(
        &self,
    ) -> Option<usb2ble_core::hid_descriptor::ReportDescriptorSummary> {
        self.active_descriptor
    }

    /// Replaces the current normalized input snapshot.
    pub fn update_input(&mut self, state: usb2ble_core::normalize::NormalizedJoystickState) {
        self.runtime.update_input(state);
    }

    /// Clears the current normalized input snapshot back to default.
    pub fn clear_input(&mut self) {
        self.runtime.clear_input();
    }

    /// Returns the active output persona implied by the current runtime profile.
    pub fn current_output_persona(&self) -> usb2ble_core::profile::OutputPersona {
        self.runtime().active_profile().output_persona()
    }

    /// Returns the BLE persona descriptor for the current output persona.
    pub fn current_ble_persona_descriptor(
        &self,
    ) -> usb2ble_platform_espidf::ble_hid::BlePersonaDescriptor {
        usb2ble_platform_espidf::ble_hid::output_persona_descriptor(self.current_output_persona())
    }

    /// Returns the currently encoded BLE input report for the active persona.
    pub fn current_encoded_ble_input_report(
        &self,
    ) -> usb2ble_platform_espidf::ble_hid::EncodedBleInputReport {
        usb2ble_platform_espidf::ble_hid::encode_input_report_for_output_persona(
            self.current_output_persona(),
            usb2ble_platform_espidf::ble_hid::BleInputReport::GenericBleGamepad16(
                self.runtime().current_report(),
            ),
        )
    }

    /// Returns typed device information for the current application state.
    pub fn device_info(&self) -> usb2ble_proto::messages::DeviceInfo {
        let active_profile = self.runtime.active_profile();

        usb2ble_proto::messages::DeviceInfo {
            protocol_version: usb2ble_proto::messages::ProtocolVersion::current(),
            firmware_name: APP_NAME,
            active_profile,
            output_persona: active_profile.output_persona(),
        }
    }

    /// Returns typed device status for the current application state.
    pub fn device_status(
        &self,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
        bond_store: &impl usb2ble_platform_espidf::nvs_store::BondStore,
    ) -> usb2ble_proto::messages::DeviceStatus {
        let active_profile = self.runtime.active_profile();

        usb2ble_proto::messages::DeviceStatus {
            active_profile,
            output_persona: active_profile.output_persona(),
            ble_link_state: map_ble_link_state(ble_state),
            bonds_present: bond_store.bonds_present(),
        }
    }

    /// Handles a typed protocol command against the current application state.
    pub fn handle_command(
        &mut self,
        command: usb2ble_proto::messages::Command,
        profile_store: &mut impl usb2ble_platform_espidf::nvs_store::ProfileStore,
        bond_store: &mut impl usb2ble_platform_espidf::nvs_store::BondStore,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
    ) -> usb2ble_proto::messages::Response {
        match command {
            usb2ble_proto::messages::Command::GetInfo => {
                usb2ble_proto::messages::Response::Info(self.device_info())
            }
            usb2ble_proto::messages::Command::GetStatus => {
                usb2ble_proto::messages::Response::Status(self.device_status(ble_state, bond_store))
            }
            usb2ble_proto::messages::Command::GetProfile => {
                usb2ble_proto::messages::Response::Profile {
                    active_profile: self.runtime().active_profile(),
                }
            }
            usb2ble_proto::messages::Command::SetProfile { profile } => {
                self.runtime.set_active_profile(profile);

                match profile_store.store_active_profile(profile) {
                    Ok(()) => usb2ble_proto::messages::Response::Ack,
                    Err(_) => usb2ble_proto::messages::Response::Error(
                        usb2ble_proto::messages::ErrorCode::Internal,
                    ),
                }
            }
            usb2ble_proto::messages::Command::Reboot => usb2ble_proto::messages::Response::Ack,
            usb2ble_proto::messages::Command::ForgetBonds => match bond_store.clear_bonds() {
                Ok(()) => usb2ble_proto::messages::Response::Ack,
                Err(_) => usb2ble_proto::messages::Response::Error(
                    usb2ble_proto::messages::ErrorCode::Internal,
                ),
            },
        }
    }

    /// Publishes the current fixed lean v1 report through the BLE output seam.
    pub fn publish_current_report(
        &self,
        ble_output: &mut impl usb2ble_platform_espidf::ble_hid::BleOutput,
    ) -> Result<(), usb2ble_platform_espidf::ble_hid::BlePublishError> {
        ble_output.publish_report(self.runtime.current_report())
    }

    /// Publishes the current encoded BLE input report through the persona-oriented BLE seam.
    pub fn publish_current_persona_report(
        &self,
        ble_output: &mut impl usb2ble_platform_espidf::ble_hid::BlePersonaOutput,
    ) -> Result<(), usb2ble_platform_espidf::ble_hid::BlePublishError> {
        ble_output.publish_encoded_report(
            self.current_output_persona(),
            self.current_encoded_ble_input_report(),
        )
    }

    /// Services at most one command from the console control plane.
    pub fn service_console_once(
        &mut self,
        command_source: &mut impl usb2ble_platform_espidf::console_uart::CommandSource,
        response_sink: &mut impl usb2ble_platform_espidf::console_uart::ResponseSink,
        profile_store: &mut impl usb2ble_platform_espidf::nvs_store::ProfileStore,
        bond_store: &mut impl usb2ble_platform_espidf::nvs_store::BondStore,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
    ) -> Result<ServiceOutcome, ServiceError> {
        let command = match command_source.poll_command() {
            Some(command) => command,
            None => return Ok(ServiceOutcome::Idle),
        };

        let response = self.handle_command(command, profile_store, bond_store, ble_state);

        response_sink
            .send_response(response)
            .map_err(ServiceError::Console)?;

        Ok(ServiceOutcome::Responded(response))
    }

    /// Services at most one newline-terminated framed command from the console buffer.
    pub fn service_console_buffer_once(
        &mut self,
        buffer: &mut usb2ble_platform_espidf::console_uart::FramedConsoleBuffer,
        profile_store: &mut impl usb2ble_platform_espidf::nvs_store::ProfileStore,
        bond_store: &mut impl usb2ble_platform_espidf::nvs_store::BondStore,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
    ) -> Result<BufferedConsoleOutcome, BufferedConsoleError> {
        let command = match buffer
            .try_decode_command()
            .map_err(BufferedConsoleError::Buffer)?
        {
            Some(command) => command,
            None => return Ok(BufferedConsoleOutcome::Idle),
        };

        let response = self.handle_command(command, profile_store, bond_store, ble_state);

        buffer
            .queue_response(response)
            .map_err(BufferedConsoleError::Buffer)?;

        Ok(BufferedConsoleOutcome::Responded(response))
    }

    /// Services one raw newline-terminated protocol frame.
    pub fn service_frame(
        &mut self,
        input: &[u8],
        profile_store: &mut impl usb2ble_platform_espidf::nvs_store::ProfileStore,
        bond_store: &mut impl usb2ble_platform_espidf::nvs_store::BondStore,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
    ) -> Result<usb2ble_proto::framing::EncodedFrame, FrameServiceError> {
        let command =
            usb2ble_proto::framing::decode_command(input).map_err(FrameServiceError::Decode)?;
        let response = self.handle_command(command, profile_store, bond_store, ble_state);

        usb2ble_proto::framing::encode_response(response).map_err(FrameServiceError::Encode)
    }

    /// Handles one USB ingress event through the pure-Rust core pipeline.
    pub fn handle_usb_event(
        &mut self,
        event: usb2ble_platform_espidf::usb_host::UsbEvent,
    ) -> Result<UsbServiceOutcome, UsbServiceError> {
        match event {
            usb2ble_platform_espidf::usb_host::UsbEvent::DeviceAttached(meta) => {
                self.active_device = Some(meta.device_id);
                self.active_descriptor = None;
                self.clear_input();

                Ok(UsbServiceOutcome::DeviceAttached {
                    device_id: meta.device_id,
                    vendor_id: meta.vendor_id,
                    product_id: meta.product_id,
                })
            }
            usb2ble_platform_espidf::usb_host::UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes,
                len,
            } => {
                if len > 64 {
                    return Err(UsbServiceError::InvalidBufferLength { len, max: 64 });
                }

                if Some(device_id) != self.active_device() {
                    return Ok(UsbServiceOutcome::Ignored);
                }

                let summary = usb2ble_core::hid_descriptor::parse_descriptor_summary(&bytes[..len])
                    .map_err(UsbServiceError::DescriptorParse)?;
                let field_count = summary.field_count();

                self.active_descriptor = Some(summary);

                Ok(UsbServiceOutcome::DescriptorStored {
                    device_id,
                    field_count,
                })
            }
            usb2ble_platform_espidf::usb_host::UsbEvent::InputReportReceived {
                device_id,
                report_id,
                bytes,
                len,
            } => {
                if len > 64 {
                    return Err(UsbServiceError::InvalidBufferLength { len, max: 64 });
                }

                if Some(device_id) != self.active_device() {
                    return Ok(UsbServiceOutcome::Ignored);
                }

                let Some(summary) = self.active_descriptor() else {
                    return Ok(UsbServiceOutcome::Ignored);
                };

                let decoded =
                    usb2ble_core::hid_decode::decode_report(&summary, report_id, &bytes[..len])
                        .map_err(UsbServiceError::ReportDecode)?;
                let normalized = usb2ble_core::normalize::normalize_decoded_report(&decoded)
                    .map_err(UsbServiceError::Normalize)?;

                self.update_input(normalized);

                Ok(UsbServiceOutcome::InputApplied(
                    self.runtime().current_report(),
                ))
            }
            usb2ble_platform_espidf::usb_host::UsbEvent::DeviceDetached(device_id) => {
                if Some(device_id) != self.active_device() {
                    return Ok(UsbServiceOutcome::Ignored);
                }

                self.active_device = None;
                self.active_descriptor = None;
                self.clear_input();

                Ok(UsbServiceOutcome::DeviceDetached(device_id))
            }
            _ => {
                // Ignore other events like internal signals
                Ok(UsbServiceOutcome::Ignored)
            }
        }
    }

    /// Services at most one USB ingress event and publishes applied input reports.
    pub fn service_usb_once(
        &mut self,
        usb_ingress: &mut impl usb2ble_platform_espidf::usb_host::UsbIngress,
        ble_output: &mut impl usb2ble_platform_espidf::ble_hid::BleOutput,
    ) -> Result<UsbPumpOutcome, UsbPumpError> {
        let event = match usb_ingress.poll_event() {
            Some(event) => event,
            None => return Ok(UsbPumpOutcome::Idle),
        };

        let outcome = self.handle_usb_event(event).map_err(UsbPumpError::Usb)?;

        match outcome {
            UsbServiceOutcome::InputApplied(report) => {
                self.publish_current_report(ble_output)
                    .map_err(UsbPumpError::Ble)?;
                Ok(UsbPumpOutcome::Published(report))
            }
            outcome => Ok(UsbPumpOutcome::Handled(outcome)),
        }
    }

    /// Services at most one USB ingress event and publishes persona-encoded applied input reports.
    pub fn service_usb_once_persona(
        &mut self,
        usb_ingress: &mut impl usb2ble_platform_espidf::usb_host::UsbIngress,
        ble_output: &mut impl usb2ble_platform_espidf::ble_hid::BlePersonaOutput,
    ) -> Result<UsbPersonaPumpOutcome, UsbPersonaPumpError> {
        let event = match usb_ingress.poll_event() {
            Some(event) => event,
            None => return Ok(UsbPersonaPumpOutcome::Idle),
        };

        let outcome = self
            .handle_usb_event(event)
            .map_err(UsbPersonaPumpError::Usb)?;

        match outcome {
            UsbServiceOutcome::InputApplied(report) => {
                let persona = self.current_output_persona();
                let encoded = self.current_encoded_ble_input_report();

                self.publish_current_persona_report(ble_output)
                    .map_err(UsbPersonaPumpError::Ble)?;

                Ok(UsbPersonaPumpOutcome::Published {
                    report,
                    persona,
                    encoded,
                })
            }
            outcome => Ok(UsbPersonaPumpOutcome::Handled(outcome)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// Services at most one buffered console or persona USB action with buffered console priority.
    pub fn service_once_with_console_buffer_persona(
        &mut self,
        buffer: &mut usb2ble_platform_espidf::console_uart::FramedConsoleBuffer,
        profile_store: &mut impl usb2ble_platform_espidf::nvs_store::ProfileStore,
        bond_store: &mut impl usb2ble_platform_espidf::nvs_store::BondStore,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
        usb_ingress: &mut impl usb2ble_platform_espidf::usb_host::UsbIngress,
        ble_output: &mut impl usb2ble_platform_espidf::ble_hid::BlePersonaOutput,
    ) -> Result<BufferedPersonaAppPumpOutcome, BufferedPersonaAppPumpError> {
        match self
            .service_console_buffer_once(buffer, profile_store, bond_store, ble_state)
            .map_err(BufferedPersonaAppPumpError::Console)?
        {
            BufferedConsoleOutcome::Responded(response) => {
                return Ok(BufferedPersonaAppPumpOutcome::Console(
                    BufferedConsoleOutcome::Responded(response),
                ));
            }
            BufferedConsoleOutcome::Idle => {}
        }

        self.service_usb_once_persona(usb_ingress, ble_output)
            .map(BufferedPersonaAppPumpOutcome::Usb)
            .map_err(BufferedPersonaAppPumpError::Usb)
    }

    #[allow(clippy::too_many_arguments)]
    /// Services at most one buffered console or USB action with buffered console priority.
    pub fn service_once_with_console_buffer(
        &mut self,
        buffer: &mut usb2ble_platform_espidf::console_uart::FramedConsoleBuffer,
        profile_store: &mut impl usb2ble_platform_espidf::nvs_store::ProfileStore,
        bond_store: &mut impl usb2ble_platform_espidf::nvs_store::BondStore,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
        usb_ingress: &mut impl usb2ble_platform_espidf::usb_host::UsbIngress,
        ble_output: &mut impl usb2ble_platform_espidf::ble_hid::BleOutput,
    ) -> Result<BufferedAppPumpOutcome, BufferedAppPumpError> {
        match self
            .service_console_buffer_once(buffer, profile_store, bond_store, ble_state)
            .map_err(BufferedAppPumpError::Console)?
        {
            BufferedConsoleOutcome::Responded(response) => {
                return Ok(BufferedAppPumpOutcome::Console(
                    BufferedConsoleOutcome::Responded(response),
                ));
            }
            BufferedConsoleOutcome::Idle => {}
        }

        self.service_usb_once(usb_ingress, ble_output)
            .map(BufferedAppPumpOutcome::Usb)
            .map_err(BufferedAppPumpError::Usb)
    }

    #[allow(clippy::too_many_arguments)]
    /// Services at most one console or USB action with console priority.
    pub fn service_once(
        &mut self,
        command_source: &mut impl usb2ble_platform_espidf::console_uart::CommandSource,
        response_sink: &mut impl usb2ble_platform_espidf::console_uart::ResponseSink,
        profile_store: &mut impl usb2ble_platform_espidf::nvs_store::ProfileStore,
        bond_store: &mut impl usb2ble_platform_espidf::nvs_store::BondStore,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
        usb_ingress: &mut impl usb2ble_platform_espidf::usb_host::UsbIngress,
        ble_output: &mut impl usb2ble_platform_espidf::ble_hid::BleOutput,
    ) -> Result<AppPumpOutcome, AppPumpError> {
        match self
            .service_console_once(
                command_source,
                response_sink,
                profile_store,
                bond_store,
                ble_state,
            )
            .map_err(AppPumpError::Console)?
        {
            ServiceOutcome::Responded(response) => {
                return Ok(AppPumpOutcome::Console(ServiceOutcome::Responded(response)));
            }
            ServiceOutcome::Idle => {}
        }

        self.service_usb_once(usb_ingress, ble_output)
            .map(AppPumpOutcome::Usb)
            .map_err(AppPumpError::Usb)
    }

    #[allow(clippy::too_many_arguments)]
    /// Services at most one console or persona-oriented USB action with console priority.
    pub fn service_once_persona(
        &mut self,
        command_source: &mut impl usb2ble_platform_espidf::console_uart::CommandSource,
        response_sink: &mut impl usb2ble_platform_espidf::console_uart::ResponseSink,
        profile_store: &mut impl usb2ble_platform_espidf::nvs_store::ProfileStore,
        bond_store: &mut impl usb2ble_platform_espidf::nvs_store::BondStore,
        ble_state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
        usb_ingress: &mut impl usb2ble_platform_espidf::usb_host::UsbIngress,
        ble_output: &mut impl usb2ble_platform_espidf::ble_hid::BlePersonaOutput,
    ) -> Result<PersonaAppPumpOutcome, PersonaAppPumpError> {
        match self
            .service_console_once(
                command_source,
                response_sink,
                profile_store,
                bond_store,
                ble_state,
            )
            .map_err(PersonaAppPumpError::Console)?
        {
            ServiceOutcome::Responded(response) => {
                return Ok(PersonaAppPumpOutcome::Console(ServiceOutcome::Responded(
                    response,
                )));
            }
            ServiceOutcome::Idle => {}
        }

        self.service_usb_once_persona(usb_ingress, ble_output)
            .map(PersonaAppPumpOutcome::Usb)
            .map_err(PersonaAppPumpError::Usb)
    }
}

fn map_ble_link_state(
    state: usb2ble_platform_espidf::ble_hid::BleConnectionState,
) -> usb2ble_proto::messages::BleLinkState {
    match state {
        usb2ble_platform_espidf::ble_hid::BleConnectionState::Idle => {
            usb2ble_proto::messages::BleLinkState::Idle
        }
        usb2ble_platform_espidf::ble_hid::BleConnectionState::Advertising => {
            usb2ble_proto::messages::BleLinkState::Advertising
        }
        usb2ble_platform_espidf::ble_hid::BleConnectionState::Connected => {
            usb2ble_proto::messages::BleLinkState::Connected
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bootstrap_default, App, AppPumpError, AppPumpOutcome, BootstrapError, BufferedAppPumpError,
        BufferedAppPumpOutcome, BufferedConsoleError, BufferedConsoleOutcome,
        BufferedPersonaAppPumpError, BufferedPersonaAppPumpOutcome, FrameServiceError,
        PersonaAppPumpError, PersonaAppPumpOutcome, ServiceError, ServiceOutcome,
        UsbPersonaPumpError, UsbPersonaPumpOutcome, UsbPumpError, UsbPumpOutcome, UsbServiceError,
        UsbServiceOutcome, APP_NAME,
    };
    use core::cell::Cell;
    use usb2ble_core::hid_decode::DecodeError;
    use usb2ble_core::hid_descriptor::{DescriptorParseError, ItemParseError};
    use usb2ble_core::normalize::NormalizeError;
    use usb2ble_core::normalize::{Axis, ButtonIndex, HatPosition, NormalizedJoystickState};
    use usb2ble_core::profile::{OutputPersona, V1_PROFILE_ID};
    use usb2ble_core::runtime::{GenericBleGamepad16Report, RuntimeState};
    use usb2ble_platform_espidf::ble_hid::{
        encode_generic_ble_gamepad16_report, BleConnectionState, BleOutput, BlePersonaOutput,
        BlePublishError, PersonaWireRecordingBleOutput, RecordingBleOutput, WireRecordingBleOutput,
    };
    use usb2ble_platform_espidf::console_uart::{
        CommandSource, ConsoleError, FrameBufferError, FramedConsoleBuffer, QueuedCommandSource,
        RecordingResponseSink, ResponseSink,
    };
    use usb2ble_platform_espidf::nvs_store::{
        BondStore, MemoryBondStore, MemoryProfileStore, ProfileStore, StoreError,
    };
    use usb2ble_platform_espidf::usb_host::{
        DeviceMeta, QueuedUsbIngress, UsbDeviceId, UsbEvent, UsbIngress,
    };
    use usb2ble_proto::framing::FrameError;

    use super::{EmbeddedDrainError, EmbeddedRuntimeState};
    use usb2ble_proto::messages::{BleLinkState, Command, Response};

    fn button_index(index: u8) -> ButtonIndex {
        match ButtonIndex::new(index) {
            Ok(index) => index,
            Err(error) => panic!("failed to create button index {index}: {error:?}"),
        }
    }

    fn xy_descriptor_bytes() -> [u8; 64] {
        let mut bytes = [0_u8; 64];
        bytes[..18].copy_from_slice(&[
            0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02,
            0x09, 0x31, 0x81, 0x02,
        ]);
        bytes
    }

    struct FakeProfileStore {
        active_profile: Option<usb2ble_core::profile::ProfileId>,
        load_calls: Cell<usize>,
        store_calls: usize,
    }

    impl ProfileStore for FakeProfileStore {
        fn load_active_profile(&self) -> Option<usb2ble_core::profile::ProfileId> {
            self.load_calls.set(self.load_calls.get() + 1);
            self.active_profile
        }

        fn store_active_profile(
            &mut self,
            profile: usb2ble_core::profile::ProfileId,
        ) -> Result<(), StoreError> {
            self.active_profile = Some(profile);
            self.store_calls += 1;
            Ok(())
        }
    }

    struct FakeBondStore {
        bonds_present: bool,
        clear_calls: usize,
    }

    impl BondStore for FakeBondStore {
        fn bonds_present(&self) -> bool {
            self.bonds_present
        }

        fn store_bonds_present(&mut self, bonds_present: bool) -> Result<(), StoreError> {
            self.bonds_present = bonds_present;
            Ok(())
        }

        fn clear_bonds(&mut self) -> Result<(), StoreError> {
            let _ = self.store_bonds_present(false);
            self.clear_calls += 1;
            Ok(())
        }
    }

    struct FakeBleOutput {
        state: BleConnectionState,
        last_report: Option<GenericBleGamepad16Report>,
        fail_with: Option<BlePublishError>,
    }

    impl BleOutput for FakeBleOutput {
        fn publish_report(
            &mut self,
            report: GenericBleGamepad16Report,
        ) -> Result<(), BlePublishError> {
            match self.fail_with {
                Some(error) => Err(error),
                None => {
                    self.last_report = Some(report);
                    Ok(())
                }
            }
        }

        fn connection_state(&self) -> BleConnectionState {
            self.state
        }
    }

    struct FakeUsbIngress {
        next_event: Option<UsbEvent>,
        poll_calls: usize,
    }

    impl UsbIngress for FakeUsbIngress {
        fn poll_event(&mut self) -> Option<UsbEvent> {
            self.poll_calls += 1;
            self.next_event.take()
        }
    }

    struct FakeCommandSource {
        next_command: Option<Command>,
        poll_calls: usize,
    }

    impl CommandSource for FakeCommandSource {
        fn poll_command(&mut self) -> Option<Command> {
            self.poll_calls += 1;
            self.next_command.take()
        }
    }

    struct FakeResponseSink {
        sent_response: Option<Response>,
        send_calls: usize,
        fail_with: Option<ConsoleError>,
    }

    impl ResponseSink for FakeResponseSink {
        fn send_response(&mut self, response: Response) -> Result<(), ConsoleError> {
            self.send_calls += 1;

            match self.fail_with {
                Some(error) => Err(error),
                None => {
                    self.sent_response = Some(response);
                    Ok(())
                }
            }
        }
    }

    fn service_frame_ok(
        app: &mut App,
        input: &[u8],
        profile_store: &mut FakeProfileStore,
        bond_store: &mut FakeBondStore,
        ble_state: BleConnectionState,
    ) -> usb2ble_proto::framing::EncodedFrame {
        match app.service_frame(input, profile_store, bond_store, ble_state) {
            Ok(frame) => frame,
            Err(error) => panic!("service_frame failed for {input:?}: {error:?}"),
        }
    }

    #[test]
    fn app_new_sets_runtime_active_profile() {
        let app = App::new(V1_PROFILE_ID);

        assert_eq!(app.runtime().active_profile(), V1_PROFILE_ID);
    }

    #[test]
    fn bootstrap_uses_persisted_profile_when_available() {
        let store = FakeProfileStore {
            active_profile: Some(V1_PROFILE_ID),
            load_calls: Cell::new(0),
            store_calls: 0,
        };

        let app = App::bootstrap(&store);

        assert_eq!(store.load_calls.get(), 1);
        assert_eq!(app.runtime().active_profile(), V1_PROFILE_ID);
    }

    #[test]
    fn bootstrap_falls_back_to_v1_profile_when_store_is_empty() {
        let store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };

        let app = App::bootstrap(&store);

        assert_eq!(store.load_calls.get(), 1);
        assert_eq!(app.runtime().active_profile(), V1_PROFILE_ID);
    }

    #[test]
    fn bootstrap_with_memory_profile_store_uses_persisted_profile() {
        let store = MemoryProfileStore::with_profile(V1_PROFILE_ID);

        let app = App::bootstrap(&store);

        assert_eq!(app.runtime().active_profile(), V1_PROFILE_ID);
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn bootstrap_default_returns_v1_profile_on_host() {
        let app = match bootstrap_default() {
            Ok(app) => app,
            Err(error) => panic!("bootstrap_default failed on host: {error:?}"),
        };

        assert_eq!(app.runtime().active_profile(), V1_PROFILE_ID);
    }

    #[test]
    fn bootstrap_error_unavailable_profile_store_compares_equal() {
        assert_eq!(
            BootstrapError::UnavailableProfileStore,
            BootstrapError::UnavailableProfileStore
        );
    }

    #[test]
    fn device_info_reports_current_contract() {
        let app = App::new(V1_PROFILE_ID);
        let info = app.device_info();

        assert_eq!(info.protocol_version.major, 1);
        assert_eq!(info.protocol_version.minor, 0);
        assert_eq!(info.firmware_name, APP_NAME);
        assert_eq!(info.active_profile, V1_PROFILE_ID);
        assert_eq!(info.output_persona, OutputPersona::GenericBleGamepad16);
    }

    #[test]
    fn device_status_maps_idle_ble_state() {
        let app = App::new(V1_PROFILE_ID);
        let bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        let status = app.device_status(BleConnectionState::Idle, &bond_store);

        assert_eq!(status.ble_link_state, BleLinkState::Idle);
    }

    #[test]
    fn device_status_maps_advertising_ble_state() {
        let app = App::new(V1_PROFILE_ID);
        let bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        let status = app.device_status(BleConnectionState::Advertising, &bond_store);

        assert_eq!(status.ble_link_state, BleLinkState::Advertising);
    }

    #[test]
    fn device_status_maps_connected_ble_state() {
        let app = App::new(V1_PROFILE_ID);
        let bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        let status = app.device_status(BleConnectionState::Connected, &bond_store);

        assert_eq!(status.ble_link_state, BleLinkState::Connected);
    }

    #[test]
    fn device_status_reflects_bond_presence() {
        let app = App::new(V1_PROFILE_ID);
        let bond_store = FakeBondStore {
            bonds_present: true,
            clear_calls: 0,
        };

        let status = app.device_status(BleConnectionState::Idle, &bond_store);

        assert!(status.bonds_present);
    }

    #[test]
    fn handle_get_info_returns_info_response() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        assert_eq!(
            app.handle_command(
                Command::GetInfo,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Response::Info(app.device_info())
        );
    }

    #[test]
    fn handle_get_status_returns_status_response() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: true,
            clear_calls: 0,
        };

        assert_eq!(
            app.handle_command(
                Command::GetStatus,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Connected
            ),
            Response::Status(app.device_status(BleConnectionState::Connected, &bond_store))
        );
    }

    #[test]
    fn handle_get_profile_returns_active_profile() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        assert_eq!(
            app.handle_command(
                Command::GetProfile,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Response::Profile {
                active_profile: V1_PROFILE_ID,
            }
        );
    }

    #[test]
    fn handle_set_profile_returns_ack_and_persists_profile() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        let response = app.handle_command(
            Command::SetProfile {
                profile: V1_PROFILE_ID,
            },
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Idle,
        );

        assert_eq!(response, Response::Ack);
        assert_eq!(profile_store.active_profile, Some(V1_PROFILE_ID));
        assert_eq!(profile_store.store_calls, 1);
        assert_eq!(app.runtime().active_profile(), V1_PROFILE_ID);
    }

    #[test]
    fn handle_forget_bonds_returns_ack_and_clears_bonds() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: true,
            clear_calls: 0,
        };

        let response = app.handle_command(
            Command::ForgetBonds,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Idle,
        );

        assert_eq!(response, Response::Ack);
        assert!(!bond_store.bonds_present);
        assert_eq!(bond_store.clear_calls, 1);
    }

    #[test]
    fn handle_reboot_returns_ack() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        assert_eq!(
            app.handle_command(
                Command::Reboot,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Response::Ack
        );
    }

    #[test]
    fn publish_current_report_forwards_exact_report_to_ble_output() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut state = NormalizedJoystickState::default();
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        state.set_axis(Axis::X, 111);
        state.set_axis(Axis::Y, -222);
        state.set_axis(Axis::Rz, 333);
        state.set_hat(HatPosition::DownLeft);
        state.set_button(button_index(0), true);
        state.set_button(button_index(5), true);
        state.set_button(button_index(15), true);
        app.update_input(state);

        assert_eq!(app.publish_current_report(&mut ble_output), Ok(()));
        assert_eq!(ble_output.connection_state(), BleConnectionState::Connected);
        assert_eq!(
            ble_output.last_report,
            Some(GenericBleGamepad16Report {
                x: 111,
                y: -222,
                rz: 333,
                hat: HatPosition::DownLeft,
                buttons: 1_u16 | (1_u16 << 5) | (1_u16 << 15),
            })
        );
    }

    #[test]
    fn publish_current_report_records_exact_ble_wire_bytes() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut state = NormalizedJoystickState::default();
        let mut ble_output = WireRecordingBleOutput::new(BleConnectionState::Connected);

        state.set_axis(Axis::X, 111);
        state.set_axis(Axis::Y, -222);
        state.set_axis(Axis::Rz, 333);
        state.set_hat(HatPosition::DownLeft);
        state.set_button(button_index(0), true);
        state.set_button(button_index(5), true);
        state.set_button(button_index(15), true);
        app.update_input(state);

        assert_eq!(app.publish_current_report(&mut ble_output), Ok(()));
        assert_eq!(
            ble_output.last_report(),
            Some(app.runtime().current_report())
        );
        assert_eq!(
            ble_output.last_wire(),
            Some(encode_generic_ble_gamepad16_report(
                app.runtime().current_report()
            ))
        );
    }

    #[test]
    fn publish_current_persona_report_records_persona_and_exact_wire_bytes() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut state = NormalizedJoystickState::default();
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        state.set_axis(Axis::X, 111);
        state.set_axis(Axis::Y, -222);
        state.set_axis(Axis::Rz, 333);
        state.set_hat(HatPosition::DownLeft);
        state.set_button(button_index(0), true);
        state.set_button(button_index(5), true);
        state.set_button(button_index(15), true);
        app.update_input(state);

        assert_eq!(
            BlePersonaOutput::connection_state(&ble_output),
            BleConnectionState::Connected
        );
        assert_eq!(app.publish_current_persona_report(&mut ble_output), Ok(()));
        assert_eq!(
            ble_output.last_persona(),
            Some(app.current_output_persona())
        );
        assert_eq!(
            ble_output.last_wire(),
            Some(app.current_encoded_ble_input_report())
        );
    }

    #[test]
    fn publish_current_persona_report_propagates_persona_output_failure() {
        let app = App::new(V1_PROFILE_ID);
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        ble_output.set_fail_with(BlePublishError::NotReady);

        assert_eq!(
            app.publish_current_persona_report(&mut ble_output),
            Err(BlePublishError::NotReady)
        );
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);
    }

    #[test]
    fn publish_current_persona_report_uses_same_encoding_as_app_helpers() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut state = NormalizedJoystickState::default();
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Advertising);

        state.set_axis(Axis::X, 5);
        state.set_axis(Axis::Y, -10);
        state.set_axis(Axis::Rz, 300);
        state.set_hat(HatPosition::DownRight);
        state.set_button(button_index(0), true);
        state.set_button(button_index(5), true);
        state.set_button(button_index(15), true);
        app.update_input(state);

        assert_eq!(app.publish_current_persona_report(&mut ble_output), Ok(()));
        assert_eq!(
            ble_output.last_persona(),
            Some(OutputPersona::GenericBleGamepad16)
        );
        assert_eq!(
            ble_output.last_wire(),
            Some(encode_generic_ble_gamepad16_report(
                app.runtime().current_report()
            ))
        );
    }

    #[test]
    fn current_output_persona_returns_active_profile_persona() {
        let app = App::new(V1_PROFILE_ID);

        assert_eq!(
            app.current_output_persona(),
            OutputPersona::GenericBleGamepad16
        );
    }

    #[test]
    fn current_ble_persona_descriptor_matches_platform_descriptor() {
        let app = App::new(V1_PROFILE_ID);

        assert_eq!(
            app.current_ble_persona_descriptor(),
            usb2ble_platform_espidf::ble_hid::output_persona_descriptor(
                OutputPersona::GenericBleGamepad16
            )
        );
    }

    #[test]
    fn current_encoded_ble_input_report_matches_platform_encoder_for_default_report() {
        let app = App::new(V1_PROFILE_ID);

        assert_eq!(
            app.current_encoded_ble_input_report(),
            usb2ble_platform_espidf::ble_hid::encode_generic_ble_gamepad16_report(
                app.runtime().current_report()
            )
        );
    }

    #[test]
    fn current_encoded_ble_input_report_matches_platform_encoder_for_non_default_report() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut state = NormalizedJoystickState::default();

        state.set_axis(Axis::X, 111);
        state.set_axis(Axis::Y, -222);
        state.set_axis(Axis::Rz, 333);
        state.set_hat(HatPosition::DownLeft);
        state.set_button(button_index(0), true);
        state.set_button(button_index(5), true);
        state.set_button(button_index(15), true);
        app.update_input(state);

        assert_eq!(
            app.current_encoded_ble_input_report(),
            usb2ble_platform_espidf::ble_hid::encode_generic_ble_gamepad16_report(
                app.runtime().current_report()
            )
        );
    }

    #[test]
    fn clear_input_resets_current_report_to_default() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut state = NormalizedJoystickState::default();

        state.set_axis(Axis::X, 5);
        state.set_axis(Axis::Y, -6);
        state.set_axis(Axis::Rz, 7);
        state.set_hat(HatPosition::Right);
        state.set_button(button_index(0), true);
        app.update_input(state);

        app.clear_input();

        assert_eq!(
            app.runtime().current_report(),
            GenericBleGamepad16Report::default()
        );
        assert_eq!(app.runtime(), RuntimeState::new(V1_PROFILE_ID));
    }

    #[test]
    fn service_console_once_returns_idle_when_no_command_is_available() {
        let mut app = App::new(V1_PROFILE_ID);
        let runtime_before = app.runtime();
        let mut command_source = FakeCommandSource {
            next_command: None,
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        assert_eq!(
            app.service_console_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Ok(ServiceOutcome::Idle)
        );
        assert_eq!(command_source.poll_calls, 1);
        assert_eq!(response_sink.send_calls, 0);
        assert_eq!(response_sink.sent_response, None);
        assert_eq!(app.runtime(), runtime_before);
    }

    #[test]
    fn service_console_once_get_info_returns_info_response_and_sends_it() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: Some(Command::GetInfo),
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let expected = Response::Info(app.device_info());

        assert_eq!(
            app.service_console_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Ok(ServiceOutcome::Responded(expected))
        );
        assert_eq!(response_sink.send_calls, 1);
        assert_eq!(response_sink.sent_response, Some(expected));
    }

    #[test]
    fn service_console_once_get_status_returns_status_response_and_sends_it() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: Some(Command::GetStatus),
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: true,
            clear_calls: 0,
        };
        let expected =
            Response::Status(app.device_status(BleConnectionState::Advertising, &bond_store));

        assert_eq!(
            app.service_console_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Advertising
            ),
            Ok(ServiceOutcome::Responded(expected))
        );
        assert_eq!(response_sink.send_calls, 1);
        assert_eq!(response_sink.sent_response, Some(expected));
    }

    #[test]
    fn service_console_once_get_profile_returns_profile_response() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: Some(Command::GetProfile),
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let expected = Response::Profile {
            active_profile: V1_PROFILE_ID,
        };

        assert_eq!(
            app.service_console_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Ok(ServiceOutcome::Responded(expected))
        );
        assert_eq!(response_sink.sent_response, Some(expected));
    }

    #[test]
    fn service_console_once_set_profile_returns_ack_and_persists_profile() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: Some(Command::SetProfile {
                profile: V1_PROFILE_ID,
            }),
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        assert_eq!(
            app.service_console_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Ok(ServiceOutcome::Responded(Response::Ack))
        );
        assert_eq!(response_sink.sent_response, Some(Response::Ack));
        assert_eq!(profile_store.active_profile, Some(V1_PROFILE_ID));
        assert_eq!(profile_store.store_calls, 1);
    }

    #[test]
    fn service_console_once_forget_bonds_returns_ack_and_clears_bonds() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: Some(Command::ForgetBonds),
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: true,
            clear_calls: 0,
        };

        assert_eq!(
            app.service_console_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Ok(ServiceOutcome::Responded(Response::Ack))
        );
        assert_eq!(response_sink.sent_response, Some(Response::Ack));
        assert!(!bond_store.bonds_present);
        assert_eq!(bond_store.clear_calls, 1);
    }

    #[test]
    fn service_console_once_returns_console_error_when_send_fails() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: Some(Command::GetInfo),
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: Some(ConsoleError::Transport),
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        assert_eq!(
            app.service_console_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Err(ServiceError::Console(ConsoleError::Transport))
        );
        assert_eq!(response_sink.send_calls, 1);
        assert_eq!(response_sink.sent_response, None);
    }

    #[test]
    fn service_console_buffer_once_returns_idle_for_partial_frame() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();

        assert_eq!(buffer.push_rx_bytes(b"GET_INFO"), Ok(()));
        assert_eq!(
            app.service_console_buffer_once(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
            ),
            Ok(BufferedConsoleOutcome::Idle)
        );
        assert_eq!(buffer.rx_len(), 8);
        assert!(buffer.tx_bytes().is_empty());
    }

    #[test]
    fn service_console_buffer_once_handles_get_info_and_queues_response() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let expected = Response::Info(app.device_info());

        assert_eq!(buffer.push_rx_bytes(b"GET_INFO\n"), Ok(()));
        assert_eq!(
            app.service_console_buffer_once(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
            ),
            Ok(BufferedConsoleOutcome::Responded(expected))
        );
        assert_eq!(
            buffer.tx_bytes(),
            b"INFO|usb2ble-fw|1|0|t16000m_v1|generic_ble_gamepad_16\n"
        );
        assert_eq!(buffer.rx_len(), 0);
    }

    #[test]
    fn service_console_buffer_once_handles_forget_bonds_and_updates_store() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::with_bonds_present(true);

        assert_eq!(buffer.push_rx_bytes(b"FORGET_BONDS\n"), Ok(()));
        assert_eq!(
            app.service_console_buffer_once(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
            ),
            Ok(BufferedConsoleOutcome::Responded(Response::Ack))
        );
        assert!(!bond_store.bonds_present());
        assert_eq!(buffer.tx_bytes(), b"ACK\n");
    }

    #[test]
    fn service_console_buffer_once_handles_set_profile_and_updates_store() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();

        assert_eq!(buffer.push_rx_bytes(b"SET_PROFILE|t16000m_v1\n"), Ok(()));
        assert_eq!(
            app.service_console_buffer_once(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
            ),
            Ok(BufferedConsoleOutcome::Responded(Response::Ack))
        );
        assert_eq!(profile_store.load_active_profile(), Some(V1_PROFILE_ID));
        assert_eq!(buffer.tx_bytes(), b"ACK\n");
    }

    #[test]
    fn service_console_buffer_once_returns_decode_error_for_malformed_frame() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();

        assert_eq!(buffer.push_rx_bytes(b"NOPE\n"), Ok(()));
        assert_eq!(
            app.service_console_buffer_once(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
            ),
            Err(BufferedConsoleError::Buffer(FrameBufferError::Decode(
                FrameError::UnknownCommand
            )))
        );
        assert_eq!(buffer.rx_len(), 0);
        assert!(buffer.tx_bytes().is_empty());
    }

    #[test]
    fn service_console_buffer_once_appends_multiple_responses_across_calls() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let first_expected = Response::Info(app.device_info());
        let second_expected = Response::Profile {
            active_profile: V1_PROFILE_ID,
        };

        assert_eq!(buffer.push_rx_bytes(b"GET_INFO\n"), Ok(()));
        assert_eq!(
            app.service_console_buffer_once(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
            ),
            Ok(BufferedConsoleOutcome::Responded(first_expected))
        );

        assert_eq!(buffer.push_rx_bytes(b"GET_PROFILE\n"), Ok(()));
        assert_eq!(
            app.service_console_buffer_once(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
            ),
            Ok(BufferedConsoleOutcome::Responded(second_expected))
        );
        assert_eq!(
            buffer.tx_bytes(),
            b"INFO|usb2ble-fw|1|0|t16000m_v1|generic_ble_gamepad_16\nPROFILE|t16000m_v1\n"
        );
    }

    #[test]
    fn service_frame_get_info_returns_exact_info_bytes() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        let frame = service_frame_ok(
            &mut app,
            b"GET_INFO\n",
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Idle,
        );

        assert_eq!(
            frame.as_bytes(),
            b"INFO|usb2ble-fw|1|0|t16000m_v1|generic_ble_gamepad_16\n"
        );
    }

    #[test]
    fn service_frame_get_status_returns_exact_status_bytes() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: true,
            clear_calls: 0,
        };

        let frame = service_frame_ok(
            &mut app,
            b"GET_STATUS\n",
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Advertising,
        );

        assert_eq!(
            frame.as_bytes(),
            b"STATUS|t16000m_v1|generic_ble_gamepad_16|advertising|1\n"
        );
    }

    #[test]
    fn service_frame_get_profile_returns_exact_profile_bytes() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        let frame = service_frame_ok(
            &mut app,
            b"GET_PROFILE\n",
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Idle,
        );

        assert_eq!(frame.as_bytes(), b"PROFILE|t16000m_v1\n");
    }

    #[test]
    fn service_frame_set_profile_returns_ack_and_persists_profile() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        let frame = service_frame_ok(
            &mut app,
            b"SET_PROFILE|t16000m_v1\n",
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Idle,
        );

        assert_eq!(frame.as_bytes(), b"ACK\n");
        assert_eq!(profile_store.active_profile, Some(V1_PROFILE_ID));
        assert_eq!(profile_store.store_calls, 1);
    }

    #[test]
    fn service_frame_forget_bonds_returns_ack_and_clears_bonds() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: true,
            clear_calls: 0,
        };

        let frame = service_frame_ok(
            &mut app,
            b"FORGET_BONDS\n",
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Idle,
        );

        assert_eq!(frame.as_bytes(), b"ACK\n");
        assert!(!bond_store.bonds_present);
        assert_eq!(bond_store.clear_calls, 1);
    }

    #[test]
    fn service_frame_reboot_returns_exact_ack_bytes() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        let frame = service_frame_ok(
            &mut app,
            b"REBOOT\n",
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Idle,
        );

        assert_eq!(frame.as_bytes(), b"ACK\n");
    }

    #[test]
    fn service_frame_empty_input_returns_decode_empty_error() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        assert_eq!(
            app.service_frame(
                b"",
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Err(FrameServiceError::Decode(FrameError::Empty))
        );
    }

    #[test]
    fn service_frame_unknown_command_returns_decode_unknown_command_error() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        assert_eq!(
            app.service_frame(
                b"NOPE\n",
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Err(FrameServiceError::Decode(FrameError::UnknownCommand))
        );
    }

    #[test]
    fn service_frame_unsupported_profile_returns_decode_unsupported_profile_error() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };

        assert_eq!(
            app.service_frame(
                b"SET_PROFILE|other\n",
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle
            ),
            Err(FrameServiceError::Decode(FrameError::UnsupportedProfile))
        );
    }

    #[test]
    fn device_attached_stores_active_device_id() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(7);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(app.active_device(), Some(device_id));
    }

    #[test]
    fn device_attached_clears_prior_descriptor_and_resets_input() {
        let mut app = App::new(V1_PROFILE_ID);
        let first_device = UsbDeviceId::new(1);
        let second_device = UsbDeviceId::new(2);
        let mut descriptor_bytes = [0_u8; 64];

        descriptor_bytes[..14].copy_from_slice(&[
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02,
        ]);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id: first_device,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id: first_device,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id: first_device,
                bytes: descriptor_bytes,
                len: 14,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id: first_device,
                field_count: 1,
            })
        );

        let mut state = NormalizedJoystickState::default();
        state.set_axis(Axis::X, 33);
        state.set_button(button_index(0), true);
        app.update_input(state);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id: second_device,
                vendor_id: 3,
                product_id: 4,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id: second_device,
                vendor_id: 3,
                product_id: 4
            })
        );
        assert_eq!(app.active_device(), Some(second_device));
        assert_eq!(app.active_descriptor(), None);
        assert_eq!(
            app.runtime().current_report(),
            GenericBleGamepad16Report::default()
        );
    }

    #[test]
    fn device_detached_for_active_device_clears_device_descriptor_and_input() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(4);
        let mut descriptor_bytes = [0_u8; 64];

        descriptor_bytes[..14].copy_from_slice(&[
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02,
        ]);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: descriptor_bytes,
                len: 14,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 1,
            })
        );

        let mut state = NormalizedJoystickState::default();
        state.set_axis(Axis::Y, -12);
        app.update_input(state);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceDetached(device_id)),
            Ok(UsbServiceOutcome::DeviceDetached(device_id))
        );
        assert_eq!(app.active_device(), None);
        assert_eq!(app.active_descriptor(), None);
        assert_eq!(
            app.runtime().current_report(),
            GenericBleGamepad16Report::default()
        );
    }

    #[test]
    fn device_detached_for_non_active_device_is_ignored() {
        let mut app = App::new(V1_PROFILE_ID);
        let active_device = UsbDeviceId::new(1);
        let other_device = UsbDeviceId::new(2);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id: active_device,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id: active_device,
                vendor_id: 1,
                product_id: 2
            })
        );

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceDetached(other_device)),
            Ok(UsbServiceOutcome::Ignored)
        );
        assert_eq!(app.active_device(), Some(active_device));
    }

    #[test]
    fn matching_report_descriptor_event_stores_descriptor_and_returns_field_count() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(9);
        let mut bytes = [0_u8; 64];

        bytes[..14].copy_from_slice(&[
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02,
        ]);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );

        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes,
                len: 14,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 1,
            })
        );
        assert_eq!(
            app.active_descriptor().map(|summary| summary.field_count()),
            Some(1)
        );
    }

    #[test]
    fn report_descriptor_event_for_non_active_device_is_ignored() {
        let mut app = App::new(V1_PROFILE_ID);
        let active_device = UsbDeviceId::new(1);
        let other_device = UsbDeviceId::new(2);
        let mut bytes = [0_u8; 64];

        bytes[..14].copy_from_slice(&[
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02,
        ]);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id: active_device,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id: active_device,
                vendor_id: 1,
                product_id: 2
            })
        );

        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id: other_device,
                bytes,
                len: 14,
            }),
            Ok(UsbServiceOutcome::Ignored)
        );
        assert_eq!(app.active_descriptor(), None);
    }

    #[test]
    fn report_descriptor_event_rejects_invalid_buffer_length() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(3);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );

        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: [0_u8; 64],
                len: 65,
            }),
            Err(UsbServiceError::InvalidBufferLength { len: 65, max: 64 })
        );
    }

    #[test]
    fn malformed_report_descriptor_event_returns_descriptor_parse_error() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(5);
        let mut bytes = [0_u8; 64];

        bytes[0] = 0xFE;
        bytes[1] = 0x00;
        bytes[2] = 0x00;

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );

        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes,
                len: 3,
            }),
            Err(UsbServiceError::DescriptorParse(
                DescriptorParseError::Item(ItemParseError::LongItemsUnsupported)
            ))
        );
    }

    #[test]
    fn input_event_for_non_active_device_is_ignored() {
        let mut app = App::new(V1_PROFILE_ID);
        let active_device = UsbDeviceId::new(1);
        let other_device = UsbDeviceId::new(2);
        let mut descriptor_bytes = [0_u8; 64];
        let mut report_bytes = [0_u8; 64];

        descriptor_bytes[..18].copy_from_slice(&[
            0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02,
            0x09, 0x31, 0x81, 0x02,
        ]);
        report_bytes[0] = 0x05;
        report_bytes[1] = 0xF6;

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id: active_device,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id: active_device,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id: active_device,
                bytes: descriptor_bytes,
                len: 18,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id: active_device,
                field_count: 2,
            })
        );

        assert_eq!(
            app.handle_usb_event(UsbEvent::InputReportReceived {
                device_id: other_device,
                report_id: 0,
                bytes: report_bytes,
                len: 2,
            }),
            Ok(UsbServiceOutcome::Ignored)
        );
    }

    #[test]
    fn input_event_before_descriptor_is_stored_is_ignored() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(6);
        let mut report_bytes = [0_u8; 64];

        report_bytes[0] = 0x05;

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );

        assert_eq!(
            app.handle_usb_event(UsbEvent::InputReportReceived {
                device_id,
                report_id: 0,
                bytes: report_bytes,
                len: 1,
            }),
            Ok(UsbServiceOutcome::Ignored)
        );
    }

    #[test]
    fn matching_input_event_decodes_normalizes_and_updates_runtime_state() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(8);
        let mut descriptor_bytes = [0_u8; 64];
        let mut report_bytes = [0_u8; 64];

        descriptor_bytes[..18].copy_from_slice(&[
            0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02,
            0x09, 0x31, 0x81, 0x02,
        ]);
        report_bytes[0] = 0x05;
        report_bytes[1] = 0xF6;

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: descriptor_bytes,
                len: 18,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 2,
            })
        );

        let expected_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };

        assert_eq!(
            app.handle_usb_event(UsbEvent::InputReportReceived {
                device_id,
                report_id: 0,
                bytes: report_bytes,
                len: 2,
            }),
            Ok(UsbServiceOutcome::InputApplied(expected_report))
        );
        assert_eq!(app.runtime().current_report(), expected_report);
    }

    #[test]
    fn input_event_rejects_invalid_buffer_length() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(10);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );

        assert_eq!(
            app.handle_usb_event(UsbEvent::InputReportReceived {
                device_id,
                report_id: 0,
                bytes: [0_u8; 64],
                len: 65,
            }),
            Err(UsbServiceError::InvalidBufferLength { len: 65, max: 64 })
        );
    }

    #[test]
    fn input_event_returns_report_decode_error_for_array_field_descriptor() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(11);
        let mut descriptor_bytes = [0_u8; 64];

        descriptor_bytes[..14].copy_from_slice(&[
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x81, 0x00,
        ]);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: descriptor_bytes,
                len: 14,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 1,
            })
        );

        assert_eq!(
            app.handle_usb_event(UsbEvent::InputReportReceived {
                device_id,
                report_id: 0,
                bytes: [0_u8; 64],
                len: 1,
            }),
            Err(UsbServiceError::ReportDecode(
                DecodeError::ArrayFieldsUnsupported
            ))
        );
    }

    #[test]
    fn input_event_returns_normalize_error_for_out_of_range_button_usage() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(12);
        let mut descriptor_bytes = [0_u8; 64];
        let mut report_bytes = [0_u8; 64];

        descriptor_bytes[..14].copy_from_slice(&[
            0x05, 0x09, 0x09, 0x11, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02,
        ]);
        report_bytes[0] = 0x01;

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: descriptor_bytes,
                len: 14,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 1,
            })
        );

        assert_eq!(
            app.handle_usb_event(UsbEvent::InputReportReceived {
                device_id,
                report_id: 0,
                bytes: report_bytes,
                len: 1,
            }),
            Err(UsbServiceError::Normalize(
                NormalizeError::ButtonOutOfRange { usage: 17 }
            ))
        );
    }

    #[test]
    fn service_usb_once_returns_idle_when_no_event_is_available() {
        let mut app = App::new(V1_PROFILE_ID);
        let runtime_before = app.runtime();
        let active_device_before = app.active_device();
        let active_descriptor_before = app.active_descriptor();
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Ok(UsbPumpOutcome::Idle)
        );
        assert_eq!(app.runtime(), runtime_before);
        assert_eq!(app.active_device(), active_device_before);
        assert_eq!(app.active_descriptor(), active_descriptor_before);
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn service_usb_once_returns_handled_for_device_attached_event() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(13);
        let mut usb_ingress = FakeUsbIngress {
            next_event: Some(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Ok(UsbPumpOutcome::Handled(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            }))
        );
        assert_eq!(app.active_device(), Some(device_id));
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn service_usb_once_returns_handled_for_descriptor_stored_event() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(14);
        let mut descriptor_bytes = [0_u8; 64];
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        descriptor_bytes[..14].copy_from_slice(&[
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02,
        ]);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );

        usb_ingress.next_event = Some(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: descriptor_bytes,
            len: 14,
        });

        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Ok(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DescriptorStored {
                    device_id,
                    field_count: 1,
                }
            ))
        );
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn service_usb_once_publishes_report_for_input_applied_event() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(15);
        let mut descriptor_bytes = [0_u8; 64];
        let mut report_bytes = [0_u8; 64];
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        descriptor_bytes[..18].copy_from_slice(&[
            0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02,
            0x09, 0x31, 0x81, 0x02,
        ]);
        report_bytes[0] = 0x05;
        report_bytes[1] = 0xF6;

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: descriptor_bytes,
                len: 18,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 2,
            })
        );

        let expected_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };

        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: report_bytes,
            len: 2,
        });

        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Ok(UsbPumpOutcome::Published(expected_report))
        );
        assert_eq!(ble_output.last_report, Some(expected_report));
    }

    #[test]
    fn service_usb_once_published_path_records_exact_ble_wire_bytes() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = WireRecordingBleOutput::new(BleConnectionState::Connected);
        let device_id = UsbDeviceId::new(41);
        let mut payload = [0_u8; 64];
        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };

        usb_ingress.next_event = Some(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Ok(UsbPumpOutcome::Handled(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            }))
        );

        usb_ingress.next_event = Some(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Ok(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DescriptorStored {
                    device_id,
                    field_count: 2,
                }
            ))
        );

        payload[0] = 0x05;
        payload[1] = 0xF6;
        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Ok(UsbPumpOutcome::Published(report))
        );
        assert_eq!(ble_output.last_report(), Some(report));
        assert_eq!(
            ble_output.last_wire(),
            Some(encode_generic_ble_gamepad16_report(report))
        );
    }

    #[test]
    fn service_usb_once_wraps_usb_errors() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(16);
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );

        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: [0_u8; 64],
            len: 65,
        });

        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Err(UsbPumpError::Usb(UsbServiceError::InvalidBufferLength {
                len: 65,
                max: 64,
            }))
        );
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn service_usb_once_persona_returns_idle_when_no_event_is_available() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(
            app.service_usb_once_persona(&mut usb_ingress, &mut ble_output),
            Ok(UsbPersonaPumpOutcome::Idle)
        );
        assert_eq!(usb_ingress.poll_calls, 1);
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);
    }

    #[test]
    fn service_usb_once_persona_handles_non_input_event_without_publish() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(51);
        let mut usb_ingress = FakeUsbIngress {
            next_event: Some(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(
            app.service_usb_once_persona(&mut usb_ingress, &mut ble_output),
            Ok(UsbPersonaPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id,
                    vendor_id: 1,
                    product_id: 2
                }
            ))
        );
        assert_eq!(app.active_device(), Some(device_id));
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);
    }

    #[test]
    fn service_usb_once_persona_publishes_persona_and_encoded_report_for_input_event() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(52);
        let mut payload = [0_u8; 64];
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: xy_descriptor_bytes(),
                len: 18,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 2,
            })
        );

        payload[0] = 0x05;
        payload[1] = 0xF6;
        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        let expected_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };
        let expected_persona = app.current_output_persona();

        assert_eq!(
            app.service_usb_once_persona(&mut usb_ingress, &mut ble_output),
            Ok(UsbPersonaPumpOutcome::Published {
                report: expected_report,
                persona: expected_persona,
                encoded: app.current_encoded_ble_input_report(),
            })
        );
        assert_eq!(ble_output.last_persona(), Some(expected_persona));
        assert_eq!(
            ble_output.last_wire(),
            Some(app.current_encoded_ble_input_report())
        );
    }

    #[test]
    fn service_usb_once_persona_propagates_persona_output_failure() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(53);
        let mut payload = [0_u8; 64];
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: xy_descriptor_bytes(),
                len: 18,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 2,
            })
        );

        ble_output.set_fail_with(BlePublishError::NotReady);

        payload[0] = 0x05;
        payload[1] = 0xF6;
        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        assert_eq!(
            app.service_usb_once_persona(&mut usb_ingress, &mut ble_output),
            Err(UsbPersonaPumpError::Ble(BlePublishError::NotReady))
        );
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);
    }

    #[test]
    fn service_usb_once_wraps_ble_publish_errors() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(17);
        let mut descriptor_bytes = [0_u8; 64];
        let mut report_bytes = [0_u8; 64];
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: Some(BlePublishError::NotReady),
        };

        descriptor_bytes[..18].copy_from_slice(&[
            0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02,
            0x09, 0x31, 0x81, 0x02,
        ]);
        report_bytes[0] = 0x05;
        report_bytes[1] = 0xF6;

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: descriptor_bytes,
                len: 18,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 2,
            })
        );

        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: report_bytes,
            len: 2,
        });

        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Err(UsbPumpError::Ble(BlePublishError::NotReady))
        );
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn wire_recording_ble_output_failure_is_propagated_through_usb_pump() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = WireRecordingBleOutput::new(BleConnectionState::Connected);
        let device_id = UsbDeviceId::new(43);
        let mut payload = [0_u8; 64];

        usb_ingress.next_event = Some(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Ok(UsbPumpOutcome::Handled(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            }))
        );

        usb_ingress.next_event = Some(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Ok(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DescriptorStored {
                    device_id,
                    field_count: 2,
                }
            ))
        );

        ble_output.set_fail_with(BlePublishError::NotReady);

        payload[0] = 0x05;
        payload[1] = 0xF6;
        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        assert_eq!(
            app.service_usb_once(&mut usb_ingress, &mut ble_output),
            Err(UsbPumpError::Ble(BlePublishError::NotReady))
        );
        assert_eq!(ble_output.last_report(), None);
        assert_eq!(ble_output.last_wire(), None);
    }

    #[test]
    fn service_once_prioritizes_console_over_usb_and_leaves_usb_pending() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(18);
        let expected = Response::Info(app.device_info());
        let mut command_source = FakeCommandSource {
            next_command: Some(Command::GetInfo),
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: Some(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(AppPumpOutcome::Console(ServiceOutcome::Responded(expected)))
        );
        assert_eq!(usb_ingress.poll_calls, 0);
        assert_eq!(app.active_device(), None);
        assert_eq!(ble_output.last_report, None);

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(AppPumpOutcome::Usb(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id,
                    vendor_id: 1,
                    product_id: 2
                }
            )))
        );
        assert_eq!(usb_ingress.poll_calls, 1);
        assert_eq!(app.active_device(), Some(device_id));
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn service_once_returns_usb_idle_when_console_and_usb_are_idle() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: None,
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(AppPumpOutcome::Usb(UsbPumpOutcome::Idle))
        );
        assert_eq!(command_source.poll_calls, 1);
        assert_eq!(usb_ingress.poll_calls, 1);
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn service_once_routes_console_idle_to_usb_handled_event() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(19);
        let mut command_source = FakeCommandSource {
            next_command: None,
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: Some(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(AppPumpOutcome::Usb(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id,
                    vendor_id: 1,
                    product_id: 2
                }
            )))
        );
        assert_eq!(app.active_device(), Some(device_id));
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn service_once_routes_console_idle_to_usb_published_input_event() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(20);
        let mut descriptor_bytes = [0_u8; 64];
        let mut report_bytes = [0_u8; 64];
        let mut command_source = FakeCommandSource {
            next_command: None,
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        descriptor_bytes[..18].copy_from_slice(&[
            0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02,
            0x09, 0x31, 0x81, 0x02,
        ]);
        report_bytes[0] = 0x05;
        report_bytes[1] = 0xF6;

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: descriptor_bytes,
                len: 18,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 2,
            })
        );

        let expected_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };

        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: report_bytes,
            len: 2,
        });

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(AppPumpOutcome::Usb(UsbPumpOutcome::Published(
                expected_report
            )))
        );
        assert_eq!(ble_output.last_report, Some(expected_report));
    }

    #[test]
    fn service_once_wraps_console_errors() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(21);
        let mut command_source = FakeCommandSource {
            next_command: Some(Command::GetInfo),
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: Some(ConsoleError::Transport),
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: Some(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Err(AppPumpError::Console(ServiceError::Console(
                ConsoleError::Transport
            )))
        );
        assert_eq!(usb_ingress.poll_calls, 0);
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn service_once_wraps_usb_errors_when_console_is_idle() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(22);
        let mut command_source = FakeCommandSource {
            next_command: None,
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: Some(UsbEvent::InputReportReceived {
                device_id,
                report_id: 0,
                bytes: [0_u8; 64],
                len: 65,
            }),
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: None,
        };

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Err(AppPumpError::Usb(UsbPumpError::Usb(
                UsbServiceError::InvalidBufferLength { len: 65, max: 64 }
            )))
        );
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn service_once_wraps_usb_publish_errors_when_console_is_idle() {
        let mut app = App::new(V1_PROFILE_ID);
        let device_id = UsbDeviceId::new(23);
        let mut descriptor_bytes = [0_u8; 64];
        let mut report_bytes = [0_u8; 64];
        let mut command_source = FakeCommandSource {
            next_command: None,
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = FakeBleOutput {
            state: BleConnectionState::Connected,
            last_report: None,
            fail_with: Some(BlePublishError::NotReady),
        };

        descriptor_bytes[..18].copy_from_slice(&[
            0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02,
            0x09, 0x31, 0x81, 0x02,
        ]);
        report_bytes[0] = 0x05;
        report_bytes[1] = 0xF6;

        assert_eq!(
            app.handle_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            })),
            Ok(UsbServiceOutcome::DeviceAttached {
                device_id,
                vendor_id: 1,
                product_id: 2
            })
        );
        assert_eq!(
            app.handle_usb_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: descriptor_bytes,
                len: 18,
            }),
            Ok(UsbServiceOutcome::DescriptorStored {
                device_id,
                field_count: 2,
            })
        );

        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: report_bytes,
            len: 2,
        });

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Err(AppPumpError::Usb(UsbPumpError::Ble(
                BlePublishError::NotReady
            )))
        );
        assert_eq!(ble_output.last_report, None);
    }

    #[test]
    fn service_once_with_platform_adapters_handles_console_get_info() {
        let mut app = App::new(V1_PROFILE_ID);
        let expected_info = app.device_info();
        let mut command_source = QueuedCommandSource::with_command(Command::GetInfo);
        let mut response_sink = RecordingResponseSink::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = QueuedUsbIngress::new();
        let mut ble_output = RecordingBleOutput::new(BleConnectionState::Idle);

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(AppPumpOutcome::Console(ServiceOutcome::Responded(
                Response::Info(expected_info)
            )))
        );
        assert_eq!(
            response_sink.last_response(),
            Some(Response::Info(expected_info))
        );
        assert_eq!(response_sink.send_calls(), 1);
        assert_eq!(command_source.poll_calls(), 1);
        assert_eq!(usb_ingress.poll_calls(), 0);
        assert_eq!(ble_output.last_report(), None);
    }

    #[test]
    fn service_once_with_platform_adapters_runs_usb_pipeline_end_to_end() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = QueuedCommandSource::new();
        let mut response_sink = RecordingResponseSink::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = QueuedUsbIngress::new();
        let mut ble_output = RecordingBleOutput::new(BleConnectionState::Connected);
        let device_id = UsbDeviceId::new(21);
        let mut report_bytes = [0_u8; 64];

        usb_ingress.queue_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(AppPumpOutcome::Usb(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id,
                    vendor_id: 1,
                    product_id: 2
                }
            )))
        );

        usb_ingress.queue_event(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(AppPumpOutcome::Usb(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DescriptorStored {
                    device_id,
                    field_count: 2,
                }
            )))
        );

        report_bytes[0] = 0x05;
        report_bytes[1] = 0xF6;
        usb_ingress.queue_event(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: report_bytes,
            len: 2,
        });

        let expected_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(AppPumpOutcome::Usb(UsbPumpOutcome::Published(
                expected_report
            )))
        );
        assert_eq!(ble_output.last_report(), Some(expected_report));
        assert_eq!(response_sink.last_response(), None);
        assert_eq!(command_source.poll_calls(), 3);
        assert_eq!(usb_ingress.poll_calls(), 3);
    }

    #[test]
    fn service_once_with_platform_adapters_clears_memory_bonds() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = QueuedCommandSource::with_command(Command::ForgetBonds);
        let mut response_sink = RecordingResponseSink::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::with_bonds_present(true);
        let mut usb_ingress = QueuedUsbIngress::new();
        let mut ble_output = RecordingBleOutput::new(BleConnectionState::Idle);

        assert_eq!(
            app.service_once(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(AppPumpOutcome::Console(ServiceOutcome::Responded(
                Response::Ack
            )))
        );
        assert!(!bond_store.bonds_present());
        assert_eq!(response_sink.last_response(), Some(Response::Ack));
        assert_eq!(usb_ingress.poll_calls(), 0);
    }

    #[test]
    fn service_once_with_console_buffer_prioritizes_console_over_usb() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let device_id = UsbDeviceId::new(31);
        let mut usb_ingress = QueuedUsbIngress::with_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        let mut ble_output = RecordingBleOutput::new(BleConnectionState::Idle);
        let expected = Response::Info(app.device_info());

        assert_eq!(buffer.push_rx_bytes(b"GET_INFO\n"), Ok(()));
        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedAppPumpOutcome::Console(
                BufferedConsoleOutcome::Responded(expected)
            ))
        );
        assert_eq!(
            buffer.tx_bytes(),
            b"INFO|usb2ble-fw|1|0|t16000m_v1|generic_ble_gamepad_16\n"
        );
        assert_eq!(app.active_device(), None);
        assert_eq!(usb_ingress.poll_calls(), 0);
        assert_eq!(ble_output.last_report(), None);

        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedAppPumpOutcome::Usb(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id,
                    vendor_id: 1,
                    product_id: 2
                }
            )))
        );
    }

    #[test]
    fn service_once_with_console_buffer_returns_usb_idle_when_both_sources_are_idle() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = QueuedUsbIngress::new();
        let mut ble_output = RecordingBleOutput::new(BleConnectionState::Idle);

        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedAppPumpOutcome::Usb(UsbPumpOutcome::Idle))
        );
    }

    #[test]
    fn service_once_with_console_buffer_handles_usb_publish_when_console_buffer_is_idle() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = QueuedUsbIngress::new();
        let mut ble_output = RecordingBleOutput::new(BleConnectionState::Connected);
        let device_id = UsbDeviceId::new(32);
        let mut report_bytes = [0_u8; 64];
        let expected_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };

        usb_ingress.queue_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedAppPumpOutcome::Usb(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id,
                    vendor_id: 1,
                    product_id: 2
                }
            )))
        );

        usb_ingress.queue_event(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedAppPumpOutcome::Usb(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DescriptorStored {
                    device_id,
                    field_count: 2,
                }
            )))
        );

        report_bytes[0] = 0x05;
        report_bytes[1] = 0xF6;
        usb_ingress.queue_event(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: report_bytes,
            len: 2,
        });
        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedAppPumpOutcome::Usb(UsbPumpOutcome::Published(
                expected_report
            )))
        );
        assert_eq!(ble_output.last_report(), Some(expected_report));
        assert!(buffer.tx_bytes().is_empty());
    }

    #[test]
    fn service_once_with_console_buffer_usb_publish_records_exact_ble_wire_bytes() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = QueuedUsbIngress::new();
        let mut ble_output = WireRecordingBleOutput::new(BleConnectionState::Connected);
        let device_id = UsbDeviceId::new(42);
        let mut payload = [0_u8; 64];
        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };

        usb_ingress.queue_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedAppPumpOutcome::Usb(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id,
                    vendor_id: 1,
                    product_id: 2
                }
            )))
        );

        usb_ingress.queue_event(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedAppPumpOutcome::Usb(UsbPumpOutcome::Handled(
                UsbServiceOutcome::DescriptorStored {
                    device_id,
                    field_count: 2,
                }
            )))
        );

        payload[0] = 0x05;
        payload[1] = 0xF6;
        usb_ingress.queue_event(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });
        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedAppPumpOutcome::Usb(UsbPumpOutcome::Published(
                report
            )))
        );
        assert_eq!(ble_output.last_report(), Some(report));
        assert_eq!(
            ble_output.last_wire(),
            Some(encode_generic_ble_gamepad16_report(report))
        );
        assert!(buffer.tx_bytes().is_empty());
    }

    #[test]
    fn service_once_with_console_buffer_wraps_buffer_decode_errors() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = QueuedUsbIngress::new();
        let mut ble_output = RecordingBleOutput::new(BleConnectionState::Idle);

        assert_eq!(buffer.push_rx_bytes(b"NOPE\n"), Ok(()));
        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Err(BufferedAppPumpError::Console(BufferedConsoleError::Buffer(
                FrameBufferError::Decode(FrameError::UnknownCommand)
            )))
        );
        assert_eq!(buffer.rx_len(), 0);
        assert!(buffer.tx_bytes().is_empty());
        assert_eq!(usb_ingress.poll_calls(), 0);
        assert_eq!(ble_output.last_report(), None);
    }

    #[test]
    fn service_once_with_console_buffer_wraps_usb_errors_when_console_is_idle() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = QueuedUsbIngress::with_event(UsbEvent::ReportDescriptorReceived {
            device_id: UsbDeviceId::new(33),
            bytes: [0_u8; 64],
            len: 65,
        });
        let mut ble_output = RecordingBleOutput::new(BleConnectionState::Idle);

        assert_eq!(
            app.service_once_with_console_buffer(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Err(BufferedAppPumpError::Usb(UsbPumpError::Usb(
                UsbServiceError::InvalidBufferLength { len: 65, max: 64 }
            )))
        );
    }

    #[test]
    fn service_once_with_console_buffer_persona_prioritizes_console_over_usb() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = FakeUsbIngress {
            next_event: Some(UsbEvent::DeviceAttached(DeviceMeta {
                device_id: UsbDeviceId::new(71),
                vendor_id: 1,
                product_id: 2,
            })),
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(buffer.push_rx_bytes(b"GET_INFO\n"), Ok(()));

        let outcome = match app.service_once_with_console_buffer_persona(
            &mut buffer,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        ) {
            Ok(outcome) => outcome,
            Err(error) => panic!("service_once_with_console_buffer_persona failed: {error:?}"),
        };

        match outcome {
            BufferedPersonaAppPumpOutcome::Console(BufferedConsoleOutcome::Responded(
                Response::Info(_),
            )) => {}
            _ => panic!("expected Info response, got {:?}", outcome),
        }

        assert!(buffer
            .tx_bytes()
            .windows(5)
            .any(|window| window == b"INFO|"));
        assert_eq!(app.active_device(), None);
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);

        let outcome2 = match app.service_once_with_console_buffer_persona(
            &mut buffer,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        ) {
            Ok(outcome) => outcome,
            Err(error) => panic!("service_once_with_console_buffer_persona failed: {error:?}"),
        };

        assert_eq!(
            outcome2,
            BufferedPersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id: UsbDeviceId::new(71),
                    vendor_id: 1,
                    product_id: 2
                }
            ))
        );
    }

    #[test]
    fn service_once_with_console_buffer_persona_returns_usb_idle_when_both_sources_are_idle() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        let outcome = match app.service_once_with_console_buffer_persona(
            &mut buffer,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        ) {
            Ok(outcome) => outcome,
            Err(error) => panic!("service_once_with_console_buffer_persona failed: {error:?}"),
        };

        assert_eq!(
            outcome,
            BufferedPersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Idle)
        );
    }

    #[test]
    fn service_once_with_console_buffer_persona_publishes_persona_usb_report_when_console_is_idle()
    {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);
        let device_id = UsbDeviceId::new(72);

        usb_ingress.next_event = Some(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        if let Err(error) = app.service_once_with_console_buffer_persona(
            &mut buffer,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        ) {
            panic!("service_once_with_console_buffer_persona failed: {error:?}");
        }

        usb_ingress.next_event = Some(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        if let Err(error) = app.service_once_with_console_buffer_persona(
            &mut buffer,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        ) {
            panic!("service_once_with_console_buffer_persona failed: {error:?}");
        }

        let mut payload = [0_u8; 64];
        payload[0] = 0x05;
        payload[1] = 0xF6;
        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        let outcome = match app.service_once_with_console_buffer_persona(
            &mut buffer,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        ) {
            Ok(outcome) => outcome,
            Err(error) => panic!("service_once_with_console_buffer_persona failed: {error:?}"),
        };

        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };
        let persona = OutputPersona::GenericBleGamepad16;
        let encoded = encode_generic_ble_gamepad16_report(report);

        assert_eq!(
            outcome,
            BufferedPersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Published {
                report,
                persona,
                encoded
            })
        );
        assert_eq!(ble_output.last_persona(), Some(persona));
        assert_eq!(ble_output.last_wire(), Some(encoded));
        assert!(buffer.tx_bytes().is_empty());
    }

    #[test]
    fn service_once_with_console_buffer_persona_wraps_console_buffer_errors() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(buffer.push_rx_bytes(b"NOPE\n"), Ok(()));

        let outcome = app.service_once_with_console_buffer_persona(
            &mut buffer,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        assert_eq!(
            outcome,
            Err(BufferedPersonaAppPumpError::Console(
                BufferedConsoleError::Buffer(FrameBufferError::Decode(FrameError::UnknownCommand))
            ))
        );
        assert_eq!(buffer.rx_len(), 0);
        assert!(buffer.tx_bytes().is_empty());
        assert_eq!(ble_output.last_persona(), None);
    }

    #[test]
    fn service_once_with_console_buffer_persona_wraps_persona_usb_publish_errors() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);
        let device_id = UsbDeviceId::new(72);

        usb_ingress.next_event = Some(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        if let Err(error) = app.service_once_with_console_buffer_persona(
            &mut buffer,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        ) {
            panic!("service_once_with_console_buffer_persona failed: {error:?}");
        }

        usb_ingress.next_event = Some(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        if let Err(error) = app.service_once_with_console_buffer_persona(
            &mut buffer,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        ) {
            panic!("service_once_with_console_buffer_persona failed: {error:?}");
        }

        ble_output.set_fail_with(BlePublishError::NotReady);

        let mut payload = [0_u8; 64];
        payload[0] = 0x05;
        payload[1] = 0xF6;
        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        let outcome = app.service_once_with_console_buffer_persona(
            &mut buffer,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        assert_eq!(
            outcome,
            Err(BufferedPersonaAppPumpError::Usb(UsbPersonaPumpError::Ble(
                BlePublishError::NotReady
            )))
        );
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);
    }

    #[test]
    fn service_once_persona_prioritizes_console_over_usb() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: Some(Command::GetInfo),
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: Some(UsbEvent::DeviceAttached(DeviceMeta {
                device_id: UsbDeviceId::new(81),
                vendor_id: 1,
                product_id: 2,
            })),
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        let result = app.service_once_persona(
            &mut command_source,
            &mut response_sink,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        let expected_info = Response::Info(app.device_info());
        assert_eq!(
            result,
            Ok(PersonaAppPumpOutcome::Console(ServiceOutcome::Responded(
                expected_info
            )))
        );
        assert_eq!(response_sink.sent_response, Some(expected_info));
        assert_eq!(app.active_device(), None);
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);

        let result2 = app.service_once_persona(
            &mut command_source,
            &mut response_sink,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        assert_eq!(
            result2,
            Ok(PersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id: UsbDeviceId::new(81),
                    vendor_id: 1,
                    product_id: 2
                }
            )))
        );
    }

    #[test]
    fn service_once_persona_returns_usb_idle_when_both_sources_are_idle() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: None,
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        let result = app.service_once_persona(
            &mut command_source,
            &mut response_sink,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        assert_eq!(
            result,
            Ok(PersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Idle))
        );
    }

    #[test]
    fn service_once_persona_publishes_persona_usb_report_when_console_is_idle() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: None,
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);
        let device_id = UsbDeviceId::new(82);

        usb_ingress.next_event = Some(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        let _ = app.service_once_persona(
            &mut command_source,
            &mut response_sink,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        usb_ingress.next_event = Some(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        let _ = app.service_once_persona(
            &mut command_source,
            &mut response_sink,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        let mut payload = [0_u8; 64];
        payload[0] = 0x05;
        payload[1] = 0xF6;
        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        let result = app.service_once_persona(
            &mut command_source,
            &mut response_sink,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };
        let persona = OutputPersona::GenericBleGamepad16;
        let encoded = encode_generic_ble_gamepad16_report(report);

        assert_eq!(
            result,
            Ok(PersonaAppPumpOutcome::Usb(
                UsbPersonaPumpOutcome::Published {
                    report,
                    persona,
                    encoded
                }
            ))
        );
        assert_eq!(ble_output.last_persona(), Some(persona));
        assert_eq!(ble_output.last_wire(), Some(encoded));
    }

    #[test]
    fn service_once_persona_wraps_console_errors() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: Some(Command::GetInfo),
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: Some(ConsoleError::Transport),
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        let result = app.service_once_persona(
            &mut command_source,
            &mut response_sink,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        assert_eq!(
            result,
            Err(PersonaAppPumpError::Console(ServiceError::Console(
                ConsoleError::Transport
            )))
        );
        assert_eq!(ble_output.last_persona(), None);
    }

    #[test]
    fn service_once_persona_wraps_persona_usb_publish_errors() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = FakeCommandSource {
            next_command: None,
            poll_calls: 0,
        };
        let mut response_sink = FakeResponseSink {
            sent_response: None,
            send_calls: 0,
            fail_with: None,
        };
        let mut profile_store = FakeProfileStore {
            active_profile: None,
            load_calls: Cell::new(0),
            store_calls: 0,
        };
        let mut bond_store = FakeBondStore {
            bonds_present: false,
            clear_calls: 0,
        };
        let mut usb_ingress = FakeUsbIngress {
            next_event: None,
            poll_calls: 0,
        };
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);
        let device_id = UsbDeviceId::new(82);

        usb_ingress.next_event = Some(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        let _ = app.service_once_persona(
            &mut command_source,
            &mut response_sink,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        usb_ingress.next_event = Some(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        let _ = app.service_once_persona(
            &mut command_source,
            &mut response_sink,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        ble_output.set_fail_with(BlePublishError::NotReady);

        let mut payload = [0_u8; 64];
        payload[0] = 0x05;
        payload[1] = 0xF6;
        usb_ingress.next_event = Some(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        let result = app.service_once_persona(
            &mut command_source,
            &mut response_sink,
            &mut profile_store,
            &mut bond_store,
            BleConnectionState::Connected,
            &mut usb_ingress,
            &mut ble_output,
        );

        assert_eq!(
            result,
            Err(PersonaAppPumpError::Usb(UsbPersonaPumpError::Ble(
                BlePublishError::NotReady
            )))
        );
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);
    }

    #[test]
    fn service_once_persona_with_platform_adapters_handles_console_get_info() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = QueuedCommandSource::with_command(Command::GetInfo);
        let mut response_sink = RecordingResponseSink::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = QueuedUsbIngress::new();
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Idle);

        let expected_info = app.device_info();

        assert_eq!(
            app.service_once_persona(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(PersonaAppPumpOutcome::Console(ServiceOutcome::Responded(
                Response::Info(expected_info)
            )))
        );

        assert_eq!(
            response_sink.last_response(),
            Some(Response::Info(expected_info))
        );
        assert_eq!(response_sink.send_calls(), 1);
        assert_eq!(command_source.poll_calls(), 1);
        assert_eq!(usb_ingress.poll_calls(), 0);
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);
    }

    #[test]
    fn service_once_persona_with_platform_adapters_runs_usb_pipeline_end_to_end() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut command_source = QueuedCommandSource::new();
        let mut response_sink = RecordingResponseSink::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = QueuedUsbIngress::new();
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        let device_id = UsbDeviceId::new(91);

        // Stage 1: Attach
        usb_ingress.queue_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));

        assert_eq!(
            app.service_once_persona(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Connected,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(PersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id,
                    vendor_id: 1,
                    product_id: 2
                }
            )))
        );

        // Stage 2: Descriptor
        usb_ingress.queue_event(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });

        assert_eq!(
            app.service_once_persona(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Connected,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(PersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Handled(
                UsbServiceOutcome::DescriptorStored {
                    device_id,
                    field_count: 2,
                }
            )))
        );

        // Stage 3: Input
        let mut payload = [0_u8; 64];
        payload[0] = 0x05;
        payload[1] = 0xF6;
        usb_ingress.queue_event(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        let expected_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };
        let expected_persona = OutputPersona::GenericBleGamepad16;
        let expected_encoded = encode_generic_ble_gamepad16_report(expected_report);

        assert_eq!(
            app.service_once_persona(
                &mut command_source,
                &mut response_sink,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Connected,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(PersonaAppPumpOutcome::Usb(
                UsbPersonaPumpOutcome::Published {
                    report: expected_report,
                    persona: expected_persona,
                    encoded: expected_encoded,
                }
            ))
        );

        assert_eq!(ble_output.last_persona(), Some(expected_persona));
        assert_eq!(ble_output.last_wire(), Some(expected_encoded));
        assert_eq!(response_sink.last_response(), None);
    }

    #[test]
    fn service_once_with_console_buffer_persona_with_platform_adapters_prioritizes_console_then_usb(
    ) {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::new();
        let mut usb_ingress = QueuedUsbIngress::with_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id: UsbDeviceId::new(92),
            vendor_id: 1,
            product_id: 2,
        }));
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        let expected_info = app.device_info();

        // Push RX bytes
        assert_eq!(buffer.push_rx_bytes(b"GET_INFO\n"), Ok(()));

        // First call: Console priority
        assert_eq!(
            app.service_once_with_console_buffer_persona(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Connected,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedPersonaAppPumpOutcome::Console(
                BufferedConsoleOutcome::Responded(Response::Info(expected_info))
            ))
        );

        assert!(buffer
            .tx_bytes()
            .windows(5)
            .any(|window| window == b"INFO|"));
        assert_eq!(app.active_device(), None);
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);

        // Second call: Handle queued USB event
        assert_eq!(
            app.service_once_with_console_buffer_persona(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Connected,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedPersonaAppPumpOutcome::Usb(
                UsbPersonaPumpOutcome::Handled(UsbServiceOutcome::DeviceAttached {
                    device_id: UsbDeviceId::new(92),
                    vendor_id: 1,
                    product_id: 2
                })
            ))
        );

        assert_eq!(app.active_device(), Some(UsbDeviceId::new(92)));
    }

    #[test]
    fn service_once_with_console_buffer_persona_with_platform_adapters_forget_bonds_roundtrip() {
        let mut app = App::new(V1_PROFILE_ID);
        let mut buffer = FramedConsoleBuffer::new();
        let mut profile_store = MemoryProfileStore::new();
        let mut bond_store = MemoryBondStore::with_bonds_present(true);
        let mut usb_ingress = QueuedUsbIngress::new();
        let mut ble_output = PersonaWireRecordingBleOutput::new(BleConnectionState::Idle);

        // Push RX bytes
        assert_eq!(buffer.push_rx_bytes(b"FORGET_BONDS\n"), Ok(()));

        // Call: Handle forget bonds
        assert_eq!(
            app.service_once_with_console_buffer_persona(
                &mut buffer,
                &mut profile_store,
                &mut bond_store,
                BleConnectionState::Idle,
                &mut usb_ingress,
                &mut ble_output,
            ),
            Ok(BufferedPersonaAppPumpOutcome::Console(
                BufferedConsoleOutcome::Responded(Response::Ack)
            ))
        );

        assert!(!bond_store.bonds_present());
        assert_eq!(buffer.tx_bytes(), b"ACK\n");
        assert_eq!(usb_ingress.poll_calls(), 0);
        assert_eq!(ble_output.last_persona(), None);
        assert_eq!(ble_output.last_wire(), None);
    }

    #[test]
    fn embedded_runtime_state_new_for_host_bootstraps_v1_profile() {
        let runtime = EmbeddedRuntimeState::new_for_host();
        assert_eq!(runtime.app.runtime().active_profile(), V1_PROFILE_ID);
        assert_eq!(runtime.ble_state(), BleConnectionState::Idle);
        assert!(!runtime.bonds_present());
    }

    #[test]
    fn embedded_runtime_state_boot_info_matches_app_contract() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        runtime.set_ble_state(BleConnectionState::Connected);
        assert!(runtime.store_bonds_present(true).is_ok());

        let info = runtime.boot_info();

        assert_eq!(info.active_profile, V1_PROFILE_ID);
        assert_eq!(info.output_persona, OutputPersona::GenericBleGamepad16);
        assert_eq!(
            info.ble_descriptor,
            usb2ble_platform_espidf::ble_hid::output_persona_descriptor(
                OutputPersona::GenericBleGamepad16
            )
        );
        assert_eq!(
            info.initial_encoded_report,
            usb2ble_platform_espidf::ble_hid::encode_generic_ble_gamepad16_report(
                GenericBleGamepad16Report::default()
            )
        );
        assert_eq!(info.ble_state, BleConnectionState::Connected);
        assert!(info.bonds_present);
    }

    #[test]
    fn embedded_runtime_state_step_persona_services_console_roundtrip() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        assert!(runtime.console_buffer.push_rx_bytes(b"GET_INFO\n").is_ok());

        let result = runtime.step_persona(BleConnectionState::Idle);

        match result {
            Ok(BufferedPersonaAppPumpOutcome::Console(BufferedConsoleOutcome::Responded(
                Response::Info(_),
            ))) => {}
            _ => panic!("expected Info response, got {:?}", result),
        }

        assert!(!runtime.console_buffer.tx_bytes().is_empty());
        assert!(runtime.ble_output.last_persona().is_none());
    }

    #[test]
    fn embedded_runtime_state_step_persona_services_usb_input_to_persona_wire() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        let device_id = UsbDeviceId::new(1);

        // Queue attach
        runtime
            .usb_ingress
            .queue_event(UsbEvent::DeviceAttached(DeviceMeta {
                device_id,
                vendor_id: 1,
                product_id: 2,
            }));
        assert!(runtime.step_persona(BleConnectionState::Connected).is_ok());

        // Queue descriptor
        runtime
            .usb_ingress
            .queue_event(UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes: xy_descriptor_bytes(),
                len: 18,
            });
        assert!(runtime.step_persona(BleConnectionState::Connected).is_ok());

        // Queue input [0x05, 0xF6]
        let mut payload = [0_u8; 64];
        payload[0] = 0x05;
        payload[1] = 0xF6;
        runtime
            .usb_ingress
            .queue_event(UsbEvent::InputReportReceived {
                device_id,
                report_id: 0,
                bytes: payload,
                len: 2,
            });

        let result = runtime.step_persona(BleConnectionState::Connected);

        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };
        let persona = OutputPersona::GenericBleGamepad16;
        let encoded = encode_generic_ble_gamepad16_report(report);

        assert_eq!(
            result,
            Ok(BufferedPersonaAppPumpOutcome::Usb(
                UsbPersonaPumpOutcome::Published {
                    report,
                    persona,
                    encoded
                }
            ))
        );

        assert_eq!(runtime.ble_output.last_persona(), Some(persona));
        assert_eq!(runtime.ble_output.last_wire(), Some(encoded));
    }

    #[test]
    fn embedded_runtime_state_queue_usb_event_exposes_event_to_step_persona() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        let device_id = UsbDeviceId::new(10);

        runtime.queue_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));

        let outcome = runtime.step_persona(BleConnectionState::Idle);
        assert_eq!(
            outcome,
            Ok(BufferedPersonaAppPumpOutcome::Usb(
                UsbPersonaPumpOutcome::Handled(UsbServiceOutcome::DeviceAttached {
                    device_id,
                    vendor_id: 1,
                    product_id: 2
                })
            ))
        );
    }

    #[test]
    fn embedded_runtime_state_push_console_bytes_allows_console_roundtrip() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        assert!(runtime.push_console_bytes(b"GET_INFO\n").is_ok());

        let result = runtime.step_persona(BleConnectionState::Idle);
        match result {
            Ok(BufferedPersonaAppPumpOutcome::Console(BufferedConsoleOutcome::Responded(
                Response::Info(_),
            ))) => {}
            _ => panic!("expected Info response, got {:?}", result),
        }
        assert!(!runtime.console_buffer.tx_bytes().is_empty());
    }

    #[test]
    fn embedded_runtime_state_last_persona_and_last_wire_reflect_publish() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        let device_id = UsbDeviceId::new(11);

        runtime.queue_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        let _ = runtime.step_persona(BleConnectionState::Connected);

        runtime.queue_usb_event(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        let _ = runtime.step_persona(BleConnectionState::Connected);

        let mut payload = [0_u8; 64];
        payload[0] = 0x05;
        payload[1] = 0xF6;
        runtime.queue_usb_event(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        let _ = runtime.step_persona(BleConnectionState::Connected);

        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };
        let persona = OutputPersona::GenericBleGamepad16;
        let encoded = encode_generic_ble_gamepad16_report(report);

        assert_eq!(runtime.current_report(), report);
        assert_eq!(runtime.last_persona(), Some(persona));
        assert_eq!(runtime.last_wire(), Some(encoded));
    }

    #[test]
    fn embedded_runtime_snapshot_matches_default_boot_state() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        runtime.set_ble_state(BleConnectionState::Advertising);
        assert!(runtime.store_bonds_present(true).is_ok());

        let snapshot = runtime.snapshot();

        assert_eq!(snapshot.active_profile, V1_PROFILE_ID);
        assert_eq!(snapshot.output_persona, OutputPersona::GenericBleGamepad16);
        assert_eq!(
            snapshot.current_report,
            GenericBleGamepad16Report::default()
        );
        assert_eq!(
            snapshot.current_encoded_report,
            usb2ble_platform_espidf::ble_hid::encode_generic_ble_gamepad16_report(
                GenericBleGamepad16Report::default()
            )
        );
        assert!(snapshot.last_persona.is_none());
        assert!(snapshot.last_wire.is_none());
        assert_eq!(snapshot.ble_state, BleConnectionState::Advertising);
        assert!(snapshot.bonds_present);
    }

    #[test]
    fn embedded_runtime_state_device_status_reflects_owned_state() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        runtime.set_ble_state(BleConnectionState::Connected);
        assert!(runtime.store_bonds_present(true).is_ok());

        let status = runtime.device_status();
        assert_eq!(
            status.ble_link_state,
            usb2ble_proto::messages::BleLinkState::Connected
        );
        assert!(status.bonds_present);
    }

    #[test]
    fn embedded_runtime_step_snapshot_includes_outcome_and_updated_state() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        let device_id = UsbDeviceId::new(12);

        // Queue attach
        runtime.queue_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        let _ = runtime.step_persona_snapshot(BleConnectionState::Connected);

        // Queue descriptor
        runtime.queue_usb_event(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        let _ = runtime.step_persona_snapshot(BleConnectionState::Connected);

        // Queue input [0x05, 0xF6]
        let mut payload = [0_u8; 64];
        payload[0] = 0x05;
        payload[1] = 0xF6;
        runtime.queue_usb_event(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        let snapshot = runtime.step_persona_snapshot(BleConnectionState::Connected);

        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };
        let persona = OutputPersona::GenericBleGamepad16;
        let encoded = encode_generic_ble_gamepad16_report(report);

        assert_eq!(
            snapshot.outcome,
            Ok(BufferedPersonaAppPumpOutcome::Usb(
                UsbPersonaPumpOutcome::Published {
                    report,
                    persona,
                    encoded
                }
            ))
        );

        assert_eq!(snapshot.runtime.current_report, report);
        assert_eq!(snapshot.runtime.output_persona, persona);
        assert_eq!(snapshot.runtime.current_encoded_report, encoded);
        assert_eq!(snapshot.runtime.last_persona, Some(persona));
        assert_eq!(snapshot.runtime.last_wire, Some(encoded));
    }

    #[test]
    fn embedded_runtime_state_drain_persona_until_idle_returns_zero_actions_for_idle_runtime() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        let result = runtime.drain_persona_until_idle(BleConnectionState::Idle, 4);

        let summary = match result {
            Ok(summary) => summary,
            Err(error) => panic!("drain failed: {:?}", error),
        };

        assert_eq!(summary.actions_processed, 0);
        assert_eq!(summary.last_non_idle_outcome, None);
        assert_eq!(summary.final_snapshot, runtime.snapshot());
        assert_eq!(
            summary.final_snapshot.current_report,
            GenericBleGamepad16Report::default()
        );
    }

    #[test]
    fn embedded_runtime_state_drain_persona_until_idle_processes_console_then_stops() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        assert!(runtime.push_console_bytes(b"GET_INFO\n").is_ok());

        let summary = match runtime.drain_persona_until_idle(BleConnectionState::Idle, 4) {
            Ok(summary) => summary,
            Err(error) => panic!("drain failed: {:?}", error),
        };

        assert_eq!(summary.actions_processed, 1);
        match summary.last_non_idle_outcome {
            Some(BufferedPersonaAppPumpOutcome::Console(BufferedConsoleOutcome::Responded(
                Response::Info(_),
            ))) => {}
            _ => {
                panic!(
                    "expected Info outcome, got {:?}",
                    summary.last_non_idle_outcome
                )
            }
        }
        assert!(!runtime.console_tx_bytes().is_empty());
        assert!(summary.final_snapshot.last_persona.is_none());
        assert!(runtime.ble_output.last_persona().is_none());
    }

    #[test]
    fn embedded_runtime_state_drain_persona_until_idle_processes_usb_attach_descriptor_input_in_one_drain(
    ) {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        let device_id = UsbDeviceId::new(13);

        // Queue ALL THREE before one drain call
        runtime.queue_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));
        runtime.queue_usb_event(UsbEvent::ReportDescriptorReceived {
            device_id,
            bytes: xy_descriptor_bytes(),
            len: 18,
        });
        let mut payload = [0_u8; 64];
        payload[0] = 0x05;
        payload[1] = 0xF6;
        runtime.queue_usb_event(UsbEvent::InputReportReceived {
            device_id,
            report_id: 0,
            bytes: payload,
            len: 2,
        });

        let summary = match runtime.drain_persona_until_idle(BleConnectionState::Connected, 8) {
            Ok(summary) => summary,
            Err(error) => panic!("drain failed: {:?}", error),
        };

        assert_eq!(summary.actions_processed, 3);
        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        };
        let persona = OutputPersona::GenericBleGamepad16;
        let encoded = encode_generic_ble_gamepad16_report(report);

        match summary.last_non_idle_outcome {
            Some(BufferedPersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Published {
                report: r,
                persona: p,
                encoded: e,
            })) => {
                assert_eq!(r, report);
                assert_eq!(p, persona);
                assert_eq!(e, encoded);
            }
            _ => {
                panic!(
                    "expected Published outcome, got {:?}",
                    summary.last_non_idle_outcome
                )
            }
        }

        assert_eq!(summary.final_snapshot.current_report, report);
        assert_eq!(summary.final_snapshot.output_persona, persona);
        assert_eq!(summary.final_snapshot.current_encoded_report, encoded);
        assert_eq!(summary.final_snapshot.last_persona, Some(persona));
        assert_eq!(summary.final_snapshot.last_wire, Some(encoded));
    }

    #[test]
    fn embedded_runtime_state_drain_persona_until_idle_respects_console_priority_before_usb() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        let device_id = UsbDeviceId::new(14);

        // Queue console command AND USB event
        assert!(runtime.push_console_bytes(b"GET_INFO\n").is_ok());
        runtime.queue_usb_event(UsbEvent::DeviceAttached(DeviceMeta {
            device_id,
            vendor_id: 1,
            product_id: 2,
        }));

        let summary = match runtime.drain_persona_until_idle(BleConnectionState::Connected, 8) {
            Ok(summary) => summary,
            Err(error) => panic!("drain failed: {:?}", error),
        };

        // Console has priority, so it processes both: Console then USB Attach
        assert_eq!(summary.actions_processed, 2);
        match summary.last_non_idle_outcome {
            Some(BufferedPersonaAppPumpOutcome::Usb(UsbPersonaPumpOutcome::Handled(
                UsbServiceOutcome::DeviceAttached {
                    device_id: id,
                    vendor_id: 1,
                    product_id: 2,
                },
            ))) => {
                assert_eq!(id, device_id);
            }
            _ => {
                panic!(
                    "expected USB Attach outcome, got {:?}",
                    summary.last_non_idle_outcome
                )
            }
        }
        assert_eq!(runtime.active_device(), Some(device_id));
        assert!(!runtime.console_tx_bytes().is_empty());
    }

    #[test]
    fn embedded_runtime_state_drain_persona_until_idle_returns_step_limit_reached() {
        let mut runtime = EmbeddedRuntimeState::new_for_host();
        assert!(runtime.push_console_bytes(b"GET_INFO\n").is_ok());

        let result = runtime.drain_persona_until_idle(BleConnectionState::Idle, 0);

        match result {
            Err(EmbeddedDrainError::StepLimitReached { max_steps, .. }) => {
                assert_eq!(max_steps, 0);
            }
            other => panic!("expected StepLimitReached, got {:?}", other),
        }
    }
}
