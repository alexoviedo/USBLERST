/// The fixed report ID used for the lean v1 generic BLE gamepad input report.
pub const GENERIC_BLE_GAMEPAD16_REPORT_ID: u8 = 1;

/// The fixed payload length, excluding the report ID byte.
pub const GENERIC_BLE_GAMEPAD16_PAYLOAD_LEN: usize = 9;

/// The full fixed wire length, including the report ID byte.
pub const GENERIC_BLE_GAMEPAD16_WIRE_LEN: usize = 10;

/// The stable persona name for the lean v1 generic BLE gamepad persona.
pub const GENERIC_BLE_GAMEPAD16_PERSONA_NAME: &str = "generic_ble_gamepad_16";

/// The fixed HID report map length for the lean v1 generic BLE gamepad persona.
pub const GENERIC_BLE_GAMEPAD16_REPORT_MAP_LEN: usize = 66;

/// The fixed HID report descriptor for the lean v1 generic BLE gamepad persona.
pub const GENERIC_BLE_GAMEPAD16_REPORT_MAP: [u8; GENERIC_BLE_GAMEPAD16_REPORT_MAP_LEN] = [
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x05, // Usage (Game Pad)
    0xA1, 0x01, // Collection (Application)
    0x85, 0x01, // Report ID (1)
    0x09, 0x30, // Usage (X)
    0x09, 0x31, // Usage (Y)
    0x09, 0x35, // Usage (Rz)
    0x16, 0x00, 0x80, // Logical Minimum (-32768)
    0x26, 0xFF, 0x7F, // Logical Maximum (32767)
    0x75, 0x10, // Report Size (16)
    0x95, 0x03, // Report Count (3)
    0x81, 0x02, // Input (Data,Var,Abs)
    0x09, 0x39, // Usage (Hat switch)
    0x15, 0x00, // Logical Minimum (0)
    0x25, 0x08, // Logical Maximum (8)
    0x35, 0x00, // Physical Minimum (0)
    0x46, 0x3B, 0x01, // Physical Maximum (315)
    0x75, 0x04, // Report Size (4)
    0x95, 0x01, // Report Count (1)
    0x81, 0x42, // Input (Data,Var,Abs,Null State)
    0x75, 0x04, // Report Size (4)
    0x95, 0x01, // Report Count (1)
    0x81, 0x01, // Input (Const,Array,Abs)
    0x05, 0x09, // Usage Page (Button)
    0x19, 0x01, // Usage Minimum (1)
    0x29, 0x10, // Usage Maximum (16)
    0x15, 0x00, // Logical Minimum (0)
    0x25, 0x01, // Logical Maximum (1)
    0x75, 0x01, // Report Size (1)
    0x95, 0x10, // Report Count (16)
    0x81, 0x02, // Input (Data,Var,Abs)
    0xC0, // End Collection
];

/// The current BLE connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BleConnectionState {
    /// No BLE activity is active.
    Idle,
    /// The BLE persona is advertising.
    Advertising,
    /// The BLE persona is connected.
    Connected,
    /// The BLE persona failed to initialize (async failure).
    InitializationFailed,
}

/// Errors that can occur when publishing a BLE report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlePublishError {
    /// The BLE output path is not ready to accept reports.
    NotReady,
    /// A transport-level failure occurred.
    Transport,
}

/// Errors that can occur during BLE stack initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BleInitError {
    /// Controller initialization failed.
    Controller,
    /// Bluedroid initialization or enablement failed.
    Bluedroid,
    /// HID device profile initialization or registration failed.
    HidDevice,
    /// Advertising configuration or startup failed.
    Advertising,
    /// The requested persona is not supported by this backend.
    UnsupportedPersona,
}

impl std::fmt::Display for BleInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Controller => write!(f, "controller init failed"),
            Self::Bluedroid => write!(f, "bluedroid init failed"),
            Self::HidDevice => write!(f, "hid device init failed"),
            Self::Advertising => write!(f, "advertising init failed"),
            Self::UnsupportedPersona => write!(f, "unsupported persona"),
        }
    }
}

/// A fixed-width encoded BLE input report ready for future transport glue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodedBleInputReport {
    bytes: [u8; GENERIC_BLE_GAMEPAD16_WIRE_LEN],
}

impl EncodedBleInputReport {
    /// Returns the full fixed wire encoding as a slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the full fixed wire encoding as an owned array.
    pub fn into_bytes(self) -> [u8; GENERIC_BLE_GAMEPAD16_WIRE_LEN] {
        self.bytes
    }
}

/// Static BLE persona metadata describing a fixed report contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlePersonaDescriptor {
    /// The output persona this BLE descriptor serves.
    pub persona: usb2ble_core::profile::OutputPersona,
    /// The stable human-readable persona name.
    pub name: &'static str,
    /// The fixed report ID used by this persona.
    pub report_id: u8,
    /// The fixed payload length, excluding the report ID byte.
    pub payload_len: usize,
    /// The full fixed wire length, including the report ID byte.
    pub wire_len: usize,
    /// The fixed HID report map bytes for the persona.
    pub report_map: &'static [u8],
}

/// Typed BLE input reports keyed by output persona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BleInputReport {
    /// The lean v1 generic BLE gamepad report.
    GenericBleGamepad16(usb2ble_core::runtime::GenericBleGamepad16Report),
}

/// Returns the stable persona name for the provided output persona.
pub fn output_persona_name(persona: usb2ble_core::profile::OutputPersona) -> &'static str {
    match persona {
        usb2ble_core::profile::OutputPersona::GenericBleGamepad16 => {
            GENERIC_BLE_GAMEPAD16_PERSONA_NAME
        }
    }
}

/// Returns the fixed BLE descriptor metadata for the provided output persona.
pub fn output_persona_descriptor(
    persona: usb2ble_core::profile::OutputPersona,
) -> BlePersonaDescriptor {
    match persona {
        usb2ble_core::profile::OutputPersona::GenericBleGamepad16 => BlePersonaDescriptor {
            persona: usb2ble_core::profile::OutputPersona::GenericBleGamepad16,
            name: GENERIC_BLE_GAMEPAD16_PERSONA_NAME,
            report_id: GENERIC_BLE_GAMEPAD16_REPORT_ID,
            payload_len: GENERIC_BLE_GAMEPAD16_PAYLOAD_LEN,
            wire_len: GENERIC_BLE_GAMEPAD16_WIRE_LEN,
            report_map: &GENERIC_BLE_GAMEPAD16_REPORT_MAP,
        },
    }
}

/// Returns the fixed HID report map bytes for the provided output persona.
pub fn report_map_for_output_persona(
    persona: usb2ble_core::profile::OutputPersona,
) -> &'static [u8] {
    output_persona_descriptor(persona).report_map
}

/// Encodes the normalized hat position into the fixed lean v1 wire value.
pub fn hat_position_to_wire(hat: usb2ble_core::normalize::HatPosition) -> u8 {
    match hat {
        usb2ble_core::normalize::HatPosition::Up => 0,
        usb2ble_core::normalize::HatPosition::UpRight => 1,
        usb2ble_core::normalize::HatPosition::Right => 2,
        usb2ble_core::normalize::HatPosition::DownRight => 3,
        usb2ble_core::normalize::HatPosition::Down => 4,
        usb2ble_core::normalize::HatPosition::DownLeft => 5,
        usb2ble_core::normalize::HatPosition::Left => 6,
        usb2ble_core::normalize::HatPosition::UpLeft => 7,
        usb2ble_core::normalize::HatPosition::Centered => 8,
    }
}

/// Encodes the fixed lean v1 generic BLE gamepad report into its wire format.
pub fn encode_generic_ble_gamepad16_report(
    report: usb2ble_core::runtime::GenericBleGamepad16Report,
) -> EncodedBleInputReport {
    let x = report.x.to_le_bytes();
    let y = report.y.to_le_bytes();
    let rz = report.rz.to_le_bytes();
    let buttons = report.buttons.to_le_bytes();

    EncodedBleInputReport {
        bytes: [
            GENERIC_BLE_GAMEPAD16_REPORT_ID,
            x[0],
            x[1],
            y[0],
            y[1],
            rz[0],
            rz[1],
            hat_position_to_wire(report.hat),
            buttons[0],
            buttons[1],
        ],
    }
}

/// Encodes a typed BLE input report for the provided output persona.
pub fn encode_input_report_for_output_persona(
    persona: usb2ble_core::profile::OutputPersona,
    report: BleInputReport,
) -> EncodedBleInputReport {
    match (persona, report) {
        (
            usb2ble_core::profile::OutputPersona::GenericBleGamepad16,
            BleInputReport::GenericBleGamepad16(report),
        ) => encode_generic_ble_gamepad16_report(report),
    }
}

/// Returns the fixed HID report map bytes for the lean v1 generic BLE gamepad persona.
pub fn generic_ble_gamepad16_report_map() -> &'static [u8] {
    &GENERIC_BLE_GAMEPAD16_REPORT_MAP
}

/// BLE output boundary for the future ESP-IDF glue.
pub trait BleOutput {
    /// Publishes the latest fixed lean v1 BLE gamepad report.
    fn publish_report(
        &mut self,
        report: usb2ble_core::runtime::GenericBleGamepad16Report,
    ) -> Result<(), BlePublishError>;

    /// Returns the current BLE connection state.
    fn connection_state(&self) -> BleConnectionState;
}

/// Persona-oriented BLE output boundary for future encoded-report transport glue.
pub trait BlePersonaOutput {
    /// Publishes one already-encoded BLE report for the provided output persona.
    fn publish_encoded_report(
        &mut self,
        persona: usb2ble_core::profile::OutputPersona,
        report: EncodedBleInputReport,
    ) -> Result<(), BlePublishError>;

    /// Returns the current BLE connection state.
    fn connection_state(&self) -> BleConnectionState;
}

/// In-memory BLE output adapter that records the last published report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordingBleOutput {
    state: BleConnectionState,
    last_report: Option<usb2ble_core::runtime::GenericBleGamepad16Report>,
    fail_with: Option<BlePublishError>,
}

impl RecordingBleOutput {
    /// Creates a recording output with the requested connection state.
    pub fn new(state: BleConnectionState) -> Self {
        Self {
            state,
            last_report: None,
            fail_with: None,
        }
    }

    /// Returns the most recently published report, if any.
    pub fn last_report(&self) -> Option<usb2ble_core::runtime::GenericBleGamepad16Report> {
        self.last_report
    }

    /// Forces future publishes to fail with the provided error.
    pub fn set_fail_with(&mut self, error: BlePublishError) {
        self.fail_with = Some(error);
    }

    /// Clears any forced publish failure.
    pub fn clear_failure(&mut self) {
        self.fail_with = None;
    }

    /// Clears the last recorded report.
    pub fn clear_last_report(&mut self) {
        self.last_report = None;
    }
}

impl BleOutput for RecordingBleOutput {
    fn publish_report(
        &mut self,
        report: usb2ble_core::runtime::GenericBleGamepad16Report,
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

/// In-memory BLE output adapter that records both typed reports and encoded wire bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireRecordingBleOutput {
    state: BleConnectionState,
    last_report: Option<usb2ble_core::runtime::GenericBleGamepad16Report>,
    last_wire: Option<EncodedBleInputReport>,
    fail_with: Option<BlePublishError>,
}

impl WireRecordingBleOutput {
    /// Creates a wire-recording output with the requested connection state.
    pub fn new(state: BleConnectionState) -> Self {
        Self {
            state,
            last_report: None,
            last_wire: None,
            fail_with: None,
        }
    }

    /// Returns the most recently published typed report, if any.
    pub fn last_report(&self) -> Option<usb2ble_core::runtime::GenericBleGamepad16Report> {
        self.last_report
    }

    /// Returns the most recently encoded wire report, if any.
    pub fn last_wire(&self) -> Option<EncodedBleInputReport> {
        self.last_wire
    }

    /// Forces future publishes to fail with the provided error.
    pub fn set_fail_with(&mut self, error: BlePublishError) {
        self.fail_with = Some(error);
    }

    /// Clears any forced publish failure.
    pub fn clear_failure(&mut self) {
        self.fail_with = None;
    }

    /// Clears the last recorded typed report.
    pub fn clear_last_report(&mut self) {
        self.last_report = None;
    }

    /// Clears the last recorded wire report.
    pub fn clear_last_wire(&mut self) {
        self.last_wire = None;
    }
}

impl BleOutput for WireRecordingBleOutput {
    fn publish_report(
        &mut self,
        report: usb2ble_core::runtime::GenericBleGamepad16Report,
    ) -> Result<(), BlePublishError> {
        match self.fail_with {
            Some(error) => Err(error),
            None => {
                self.last_report = Some(report);
                self.last_wire = Some(encode_generic_ble_gamepad16_report(report));
                Ok(())
            }
        }
    }

    fn connection_state(&self) -> BleConnectionState {
        self.state
    }
}

/// In-memory BLE persona output adapter that records the last persona and encoded wire bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersonaWireRecordingBleOutput {
    state: BleConnectionState,
    last_persona: Option<usb2ble_core::profile::OutputPersona>,
    last_wire: Option<EncodedBleInputReport>,
    fail_with: Option<BlePublishError>,
}

impl PersonaWireRecordingBleOutput {
    /// Creates a persona wire-recording output with the requested connection state.
    pub fn new(state: BleConnectionState) -> Self {
        Self {
            state,
            last_persona: None,
            last_wire: None,
            fail_with: None,
        }
    }

    /// Returns the most recently published output persona, if any.
    pub fn last_persona(&self) -> Option<usb2ble_core::profile::OutputPersona> {
        self.last_persona
    }

    /// Returns the most recently published encoded wire report, if any.
    pub fn last_wire(&self) -> Option<EncodedBleInputReport> {
        self.last_wire
    }

    /// Forces future publishes to fail with the provided error.
    pub fn set_fail_with(&mut self, error: BlePublishError) {
        self.fail_with = Some(error);
    }

    /// Clears any forced publish failure.
    pub fn clear_failure(&mut self) {
        self.fail_with = None;
    }

    /// Clears the last recorded output persona.
    pub fn clear_last_persona(&mut self) {
        self.last_persona = None;
    }

    /// Clears the last recorded wire report.
    pub fn clear_last_wire(&mut self) {
        self.last_wire = None;
    }
}

impl BlePersonaOutput for PersonaWireRecordingBleOutput {
    fn publish_encoded_report(
        &mut self,
        persona: usb2ble_core::profile::OutputPersona,
        report: EncodedBleInputReport,
    ) -> Result<(), BlePublishError> {
        match self.fail_with {
            Some(error) => Err(error),
            None => {
                self.last_persona = Some(persona);
                self.last_wire = Some(report);
                Ok(())
            }
        }
    }

    fn connection_state(&self) -> BleConnectionState {
        self.state
    }
}

#[cfg(target_os = "espidf")]
pub use crate::ble_hid_esp::EspBlePersonaOutput;

#[cfg(not(target_os = "espidf"))]
/// Stub BLE HID backend for host-side testing.
pub struct EspBlePersonaOutput;

#[cfg(not(target_os = "espidf"))]
impl EspBlePersonaOutput {
    /// Returns an error on host targets as real BLE is unavailable.
    pub fn new_generic_gamepad_v1() -> Result<Self, BleInitError> {
        Err(BleInitError::UnsupportedPersona)
    }
}

#[cfg(not(target_os = "espidf"))]
impl BlePersonaOutput for EspBlePersonaOutput {
    fn publish_encoded_report(
        &mut self,
        _persona: usb2ble_core::profile::OutputPersona,
        _report: EncodedBleInputReport,
    ) -> Result<(), BlePublishError> {
        Err(BlePublishError::NotReady)
    }

    fn connection_state(&self) -> BleConnectionState {
        BleConnectionState::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::{
        encode_generic_ble_gamepad16_report, encode_input_report_for_output_persona,
        generic_ble_gamepad16_report_map, hat_position_to_wire, output_persona_descriptor,
        output_persona_name, report_map_for_output_persona, BleConnectionState, BleInputReport,
        BleOutput, BlePersonaOutput, BlePublishError, PersonaWireRecordingBleOutput,
        WireRecordingBleOutput, GENERIC_BLE_GAMEPAD16_PAYLOAD_LEN,
        GENERIC_BLE_GAMEPAD16_PERSONA_NAME, GENERIC_BLE_GAMEPAD16_REPORT_ID,
        GENERIC_BLE_GAMEPAD16_REPORT_MAP, GENERIC_BLE_GAMEPAD16_REPORT_MAP_LEN,
        GENERIC_BLE_GAMEPAD16_WIRE_LEN,
    };
    use usb2ble_core::normalize::HatPosition;
    use usb2ble_core::profile::OutputPersona;
    use usb2ble_core::runtime::GenericBleGamepad16Report;

    fn contains_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }

        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn generic_ble_gamepad16_constants_match_expected_values() {
        assert_eq!(GENERIC_BLE_GAMEPAD16_REPORT_ID, 1);
        assert_eq!(GENERIC_BLE_GAMEPAD16_PAYLOAD_LEN, 9);
        assert_eq!(GENERIC_BLE_GAMEPAD16_WIRE_LEN, 10);
        assert_eq!(GENERIC_BLE_GAMEPAD16_REPORT_MAP_LEN, 66);
    }

    #[test]
    fn output_persona_name_returns_generic_ble_gamepad16_name() {
        assert_eq!(
            output_persona_name(OutputPersona::GenericBleGamepad16),
            GENERIC_BLE_GAMEPAD16_PERSONA_NAME
        );
    }

    #[test]
    fn output_persona_descriptor_returns_generic_ble_gamepad16_descriptor() {
        let descriptor = output_persona_descriptor(OutputPersona::GenericBleGamepad16);

        assert_eq!(descriptor.persona, OutputPersona::GenericBleGamepad16);
        assert_eq!(descriptor.name, GENERIC_BLE_GAMEPAD16_PERSONA_NAME);
        assert_eq!(descriptor.report_id, GENERIC_BLE_GAMEPAD16_REPORT_ID);
        assert_eq!(descriptor.payload_len, GENERIC_BLE_GAMEPAD16_PAYLOAD_LEN);
        assert_eq!(descriptor.wire_len, GENERIC_BLE_GAMEPAD16_WIRE_LEN);
        assert_eq!(
            descriptor.report_map,
            GENERIC_BLE_GAMEPAD16_REPORT_MAP.as_slice()
        );
    }

    #[test]
    fn report_map_for_output_persona_returns_generic_ble_gamepad16_map() {
        assert_eq!(
            report_map_for_output_persona(OutputPersona::GenericBleGamepad16),
            GENERIC_BLE_GAMEPAD16_REPORT_MAP.as_slice()
        );
    }

    #[test]
    fn generic_ble_gamepad16_report_map_helper_returns_constant_bytes() {
        assert_eq!(
            generic_ble_gamepad16_report_map(),
            GENERIC_BLE_GAMEPAD16_REPORT_MAP.as_slice()
        );
    }

    #[test]
    fn generic_ble_gamepad16_report_map_has_expected_prefix() {
        assert_eq!(
            &GENERIC_BLE_GAMEPAD16_REPORT_MAP[..8],
            &[0x05, 0x01, 0x09, 0x05, 0xA1, 0x01, 0x85, 0x01]
        );
    }

    #[test]
    fn generic_ble_gamepad16_report_map_bytes_are_exact() {
        assert_eq!(
            GENERIC_BLE_GAMEPAD16_REPORT_MAP,
            [
                0x05, 0x01, 0x09, 0x05, 0xA1, 0x01, 0x85, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x35,
                0x16, 0x00, 0x80, 0x26, 0xFF, 0x7F, 0x75, 0x10, 0x95, 0x03, 0x81, 0x02, 0x09, 0x39,
                0x15, 0x00, 0x25, 0x08, 0x35, 0x00, 0x46, 0x3B, 0x01, 0x75, 0x04, 0x95, 0x01, 0x81,
                0x42, 0x75, 0x04, 0x95, 0x01, 0x81, 0x01, 0x05, 0x09, 0x19, 0x01, 0x29, 0x10, 0x15,
                0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x10, 0x81, 0x02, 0xC0,
            ]
        );
    }

    #[test]
    fn generic_ble_gamepad16_report_map_contains_axes_block() {
        assert!(contains_subsequence(
            &GENERIC_BLE_GAMEPAD16_REPORT_MAP,
            &[
                0x09, 0x30, 0x09, 0x31, 0x09, 0x35, 0x16, 0x00, 0x80, 0x26, 0xFF, 0x7F, 0x75, 0x10,
                0x95, 0x03, 0x81, 0x02,
            ]
        ));
    }

    #[test]
    fn generic_ble_gamepad16_report_map_contains_hat_block() {
        assert!(contains_subsequence(
            &GENERIC_BLE_GAMEPAD16_REPORT_MAP,
            &[
                0x09, 0x39, 0x15, 0x00, 0x25, 0x08, 0x35, 0x00, 0x46, 0x3B, 0x01, 0x75, 0x04, 0x95,
                0x01, 0x81, 0x42,
            ]
        ));
    }

    #[test]
    fn generic_ble_gamepad16_report_map_contains_padding_block() {
        assert!(contains_subsequence(
            &GENERIC_BLE_GAMEPAD16_REPORT_MAP,
            &[0x75, 0x04, 0x95, 0x01, 0x81, 0x01]
        ));
    }

    #[test]
    fn generic_ble_gamepad16_report_map_contains_button_block() {
        assert!(contains_subsequence(
            &GENERIC_BLE_GAMEPAD16_REPORT_MAP,
            &[
                0x05, 0x09, 0x19, 0x01, 0x29, 0x10, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x10,
                0x81, 0x02,
            ]
        ));
    }

    #[test]
    fn generic_ble_gamepad16_report_map_ends_with_end_collection() {
        assert_eq!(GENERIC_BLE_GAMEPAD16_REPORT_MAP.last(), Some(&0xC0));
    }

    #[test]
    fn hat_position_to_wire_maps_expected_values() {
        assert_eq!(hat_position_to_wire(HatPosition::Up), 0);
        assert_eq!(hat_position_to_wire(HatPosition::Right), 2);
        assert_eq!(hat_position_to_wire(HatPosition::DownLeft), 5);
        assert_eq!(hat_position_to_wire(HatPosition::Centered), 8);
    }

    #[test]
    fn encode_generic_ble_gamepad16_report_defaultish_bytes_are_exact() {
        let encoded = encode_generic_ble_gamepad16_report(GenericBleGamepad16Report {
            x: 0,
            y: 0,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        });

        assert_eq!(encoded.into_bytes(), [1, 0, 0, 0, 0, 0, 0, 8, 0, 0]);
    }

    #[test]
    fn encode_generic_ble_gamepad16_report_non_trivial_bytes_are_exact() {
        let buttons = (1_u16 << 0) | (1_u16 << 5) | (1_u16 << 15);
        let encoded = encode_generic_ble_gamepad16_report(GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 300,
            hat: HatPosition::DownRight,
            buttons,
        });

        assert_eq!(
            encoded.into_bytes(),
            [
                GENERIC_BLE_GAMEPAD16_REPORT_ID,
                5_i16.to_le_bytes()[0],
                5_i16.to_le_bytes()[1],
                (-10_i16).to_le_bytes()[0],
                (-10_i16).to_le_bytes()[1],
                300_i16.to_le_bytes()[0],
                300_i16.to_le_bytes()[1],
                3,
                buttons.to_le_bytes()[0],
                buttons.to_le_bytes()[1],
            ]
        );
    }

    #[test]
    fn encoded_ble_input_report_helpers_return_full_wire_bytes() {
        let encoded = encode_generic_ble_gamepad16_report(GenericBleGamepad16Report {
            x: 0,
            y: 0,
            rz: 0,
            hat: HatPosition::Centered,
            buttons: 0,
        });
        let expected = [1, 0, 0, 0, 0, 0, 0, 8, 0, 0];

        assert_eq!(encoded.as_bytes(), expected.as_slice());
        assert_eq!(encoded.into_bytes(), expected);
    }

    #[test]
    fn encode_input_report_for_output_persona_matches_generic_ble_gamepad16_encoder() {
        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 300,
            hat: HatPosition::DownRight,
            buttons: (1_u16 << 0) | (1_u16 << 5) | (1_u16 << 15),
        };

        assert_eq!(
            encode_input_report_for_output_persona(
                OutputPersona::GenericBleGamepad16,
                BleInputReport::GenericBleGamepad16(report)
            ),
            encode_generic_ble_gamepad16_report(report)
        );
    }

    #[test]
    fn wire_recording_ble_output_new_starts_empty_with_requested_state() {
        let output = WireRecordingBleOutput::new(BleConnectionState::Advertising);

        assert_eq!(output.connection_state(), BleConnectionState::Advertising);
        assert_eq!(output.last_report(), None);
        assert_eq!(output.last_wire(), None);
    }

    #[test]
    fn wire_recording_ble_output_successful_publish_records_typed_and_wire_reports() {
        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 300,
            hat: HatPosition::DownRight,
            buttons: (1_u16 << 0) | (1_u16 << 5) | (1_u16 << 15),
        };
        let mut output = WireRecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(output.publish_report(report), Ok(()));
        assert_eq!(output.last_report(), Some(report));
        assert_eq!(
            output.last_wire(),
            Some(encode_generic_ble_gamepad16_report(report))
        );
    }

    #[test]
    fn wire_recording_ble_output_forced_failure_preserves_previous_state() {
        let first_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 300,
            hat: HatPosition::DownRight,
            buttons: (1_u16 << 0) | (1_u16 << 5) | (1_u16 << 15),
        };
        let second_report = GenericBleGamepad16Report {
            x: -1,
            y: 2,
            rz: -3,
            hat: HatPosition::Left,
            buttons: 0x0003,
        };
        let mut output = WireRecordingBleOutput::new(BleConnectionState::Connected);
        let first_wire = Some(encode_generic_ble_gamepad16_report(first_report));

        assert_eq!(output.publish_report(first_report), Ok(()));
        output.set_fail_with(BlePublishError::NotReady);

        assert_eq!(
            output.publish_report(second_report),
            Err(BlePublishError::NotReady)
        );
        assert_eq!(output.last_report(), Some(first_report));
        assert_eq!(output.last_wire(), first_wire);
    }

    #[test]
    fn wire_recording_ble_output_clear_helpers_and_failure_reset_work_as_expected() {
        let first_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 300,
            hat: HatPosition::DownRight,
            buttons: (1_u16 << 0) | (1_u16 << 5) | (1_u16 << 15),
        };
        let second_report = GenericBleGamepad16Report {
            x: 1,
            y: 2,
            rz: 3,
            hat: HatPosition::Up,
            buttons: 0x0001,
        };
        let mut output = WireRecordingBleOutput::new(BleConnectionState::Idle);

        assert_eq!(output.publish_report(first_report), Ok(()));
        output.clear_last_report();
        assert_eq!(output.last_report(), None);
        assert_eq!(
            output.last_wire(),
            Some(encode_generic_ble_gamepad16_report(first_report))
        );

        output.clear_last_wire();
        assert_eq!(output.last_wire(), None);

        output.set_fail_with(BlePublishError::NotReady);
        output.clear_failure();

        assert_eq!(output.publish_report(second_report), Ok(()));
        assert_eq!(output.last_report(), Some(second_report));
        assert_eq!(
            output.last_wire(),
            Some(encode_generic_ble_gamepad16_report(second_report))
        );
    }

    #[test]
    fn persona_wire_recording_ble_output_new_starts_empty_with_requested_state() {
        let output = PersonaWireRecordingBleOutput::new(BleConnectionState::Advertising);

        assert_eq!(output.connection_state(), BleConnectionState::Advertising);
        assert_eq!(output.last_persona(), None);
        assert_eq!(output.last_wire(), None);
    }

    #[test]
    fn persona_wire_recording_ble_output_successful_publish_records_persona_and_wire() {
        let persona = OutputPersona::GenericBleGamepad16;
        let report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 300,
            hat: HatPosition::DownRight,
            buttons: (1_u16 << 0) | (1_u16 << 5) | (1_u16 << 15),
        };
        let encoded = encode_generic_ble_gamepad16_report(report);
        let mut output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(output.publish_encoded_report(persona, encoded), Ok(()));
        assert_eq!(output.last_persona(), Some(persona));
        assert_eq!(output.last_wire(), Some(encoded));
    }

    #[test]
    fn persona_wire_recording_ble_output_forced_failure_preserves_previous_state() {
        let persona = OutputPersona::GenericBleGamepad16;
        let first_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 300,
            hat: HatPosition::DownRight,
            buttons: (1_u16 << 0) | (1_u16 << 5) | (1_u16 << 15),
        };
        let second_report = GenericBleGamepad16Report {
            x: -1,
            y: 2,
            rz: -3,
            hat: HatPosition::Left,
            buttons: 0x0003,
        };
        let first_encoded = encode_generic_ble_gamepad16_report(first_report);
        let second_encoded = encode_generic_ble_gamepad16_report(second_report);
        let mut output = PersonaWireRecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(
            output.publish_encoded_report(persona, first_encoded),
            Ok(())
        );
        output.set_fail_with(BlePublishError::NotReady);

        assert_eq!(
            output.publish_encoded_report(persona, second_encoded),
            Err(BlePublishError::NotReady)
        );
        assert_eq!(output.last_persona(), Some(persona));
        assert_eq!(output.last_wire(), Some(first_encoded));
    }

    #[test]
    fn persona_wire_recording_ble_output_clear_helpers_and_failure_reset_work_as_expected() {
        let persona = OutputPersona::GenericBleGamepad16;
        let first_report = GenericBleGamepad16Report {
            x: 5,
            y: -10,
            rz: 300,
            hat: HatPosition::DownRight,
            buttons: (1_u16 << 0) | (1_u16 << 5) | (1_u16 << 15),
        };
        let second_report = GenericBleGamepad16Report {
            x: 1,
            y: 2,
            rz: 3,
            hat: HatPosition::Up,
            buttons: 0x0001,
        };
        let first_encoded = encode_generic_ble_gamepad16_report(first_report);
        let second_encoded = encode_generic_ble_gamepad16_report(second_report);
        let mut output = PersonaWireRecordingBleOutput::new(BleConnectionState::Idle);

        assert_eq!(
            output.publish_encoded_report(persona, first_encoded),
            Ok(())
        );
        output.clear_last_persona();
        assert_eq!(output.last_persona(), None);
        assert_eq!(output.last_wire(), Some(first_encoded));

        output.clear_last_wire();
        assert_eq!(output.last_wire(), None);

        output.set_fail_with(BlePublishError::NotReady);
        output.clear_failure();

        assert_eq!(
            output.publish_encoded_report(persona, second_encoded),
            Ok(())
        );
        assert_eq!(output.last_persona(), Some(persona));
        assert_eq!(output.last_wire(), Some(second_encoded));
    }
}
