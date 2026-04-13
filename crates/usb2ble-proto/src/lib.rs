//! Protocol-domain contracts for the USB-to-BLE bridge workspace.

/// UART-friendly newline-terminated ASCII framing for the lean v1 control plane.
pub mod framing {
    use core::str;

    /// The fixed frame terminator byte.
    pub const FRAME_TERMINATOR: u8 = b'\n';

    /// The maximum supported frame length in bytes.
    pub const MAX_FRAME_LEN: usize = 96;

    /// A fixed-capacity encoded wire frame.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EncodedFrame {
        bytes: [u8; MAX_FRAME_LEN],
        len: usize,
    }

    impl EncodedFrame {
        /// Returns the encoded frame as an exact slice without trailing zeroes.
        pub fn as_bytes(&self) -> &[u8] {
            &self.bytes[..self.len]
        }

        /// Returns the encoded frame length.
        pub fn len(&self) -> usize {
            self.len
        }

        /// Returns whether the encoded frame is empty.
        pub fn is_empty(&self) -> bool {
            self.len == 0
        }
    }

    /// Errors that can occur while decoding or encoding wire frames.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FrameError {
        /// The input frame is empty.
        Empty,
        /// The input frame is missing the required terminator.
        MissingTerminator,
        /// The input or output frame exceeds the fixed maximum length.
        TooLong {
            /// The full input or output length that was attempted.
            attempted: usize,
            /// The maximum supported frame length.
            max: usize,
        },
        /// The input frame body is not valid UTF-8.
        InvalidUtf8,
        /// The command token is not recognized.
        UnknownCommand,
        /// The general shape of the frame is invalid for the command.
        InvalidFormat,
        /// The requested profile is not supported by lean v1.
        UnsupportedProfile,
    }

    struct FrameWriter {
        bytes: [u8; MAX_FRAME_LEN],
        len: usize,
        attempted: usize,
    }

    impl FrameWriter {
        fn new() -> Self {
            Self {
                bytes: [0_u8; MAX_FRAME_LEN],
                len: 0,
                attempted: 0,
            }
        }

        fn push_bytes(&mut self, bytes: &[u8]) {
            self.attempted += bytes.len();

            if self.len < MAX_FRAME_LEN {
                let remaining = MAX_FRAME_LEN - self.len;
                let copy_len = remaining.min(bytes.len());
                self.bytes[self.len..self.len + copy_len].copy_from_slice(&bytes[..copy_len]);
                self.len += copy_len;
            }
        }

        fn push_byte(&mut self, byte: u8) {
            self.push_bytes(&[byte]);
        }

        fn push_u8_decimal(&mut self, value: u8) {
            let hundreds = value / 100;
            let tens = (value / 10) % 10;
            let ones = value % 10;

            if hundreds != 0 {
                self.push_byte(b'0' + hundreds);
            }

            if hundreds != 0 || tens != 0 {
                self.push_byte(b'0' + tens);
            }

            self.push_byte(b'0' + ones);
        }

        fn finish(self) -> Result<EncodedFrame, FrameError> {
            if self.attempted > MAX_FRAME_LEN {
                Err(FrameError::TooLong {
                    attempted: self.attempted,
                    max: MAX_FRAME_LEN,
                })
            } else {
                Ok(EncodedFrame {
                    bytes: self.bytes,
                    len: self.len,
                })
            }
        }
    }

    fn profile_str(profile: usb2ble_core::profile::ProfileId) -> &'static str {
        profile.as_str()
    }

    fn persona_str(persona: usb2ble_core::profile::OutputPersona) -> &'static str {
        match persona {
            usb2ble_core::profile::OutputPersona::GenericBleGamepad16 => "generic_ble_gamepad_16",
        }
    }

    fn ble_link_state_str(state: crate::messages::BleLinkState) -> &'static str {
        match state {
            crate::messages::BleLinkState::Idle => "idle",
            crate::messages::BleLinkState::Advertising => "advertising",
            crate::messages::BleLinkState::Connected => "connected",
        }
    }

    fn bool_str(value: bool) -> &'static str {
        if value {
            "1"
        } else {
            "0"
        }
    }

    fn error_code_str(code: crate::messages::ErrorCode) -> &'static str {
        match code {
            crate::messages::ErrorCode::UnsupportedProfile => "unsupported_profile",
            crate::messages::ErrorCode::InvalidRequest => "invalid_request",
            crate::messages::ErrorCode::Internal => "internal",
        }
    }

    /// Decodes a typed command from a newline-terminated ASCII wire frame.
    pub fn decode_command(input: &[u8]) -> Result<crate::messages::Command, FrameError> {
        if input.is_empty() {
            return Err(FrameError::Empty);
        }

        if input.len() > MAX_FRAME_LEN {
            return Err(FrameError::TooLong {
                attempted: input.len(),
                max: MAX_FRAME_LEN,
            });
        }

        if input[input.len() - 1] != FRAME_TERMINATOR {
            return Err(FrameError::MissingTerminator);
        }

        let body = &input[..input.len() - 1];
        let text = str::from_utf8(body).map_err(|_| FrameError::InvalidUtf8)?;

        match text {
            "GET_INFO" => Ok(crate::messages::Command::GetInfo),
            "GET_STATUS" => Ok(crate::messages::Command::GetStatus),
            "GET_PROFILE" => Ok(crate::messages::Command::GetProfile),
            "REBOOT" => Ok(crate::messages::Command::Reboot),
            "FORGET_BONDS" => Ok(crate::messages::Command::ForgetBonds),
            _ => {
                if let Some((token, remainder)) = text.split_once('|') {
                    match token {
                        "GET_INFO" | "GET_STATUS" | "GET_PROFILE" | "REBOOT" | "FORGET_BONDS" => {
                            Err(FrameError::InvalidFormat)
                        }
                        "SET_PROFILE" => {
                            if remainder.is_empty() || remainder.contains('|') {
                                Err(FrameError::InvalidFormat)
                            } else if remainder == usb2ble_core::profile::V1_PROFILE_NAME {
                                Ok(crate::messages::Command::SetProfile {
                                    profile: usb2ble_core::profile::V1_PROFILE_ID,
                                })
                            } else {
                                Err(FrameError::UnsupportedProfile)
                            }
                        }
                        _ => Err(FrameError::UnknownCommand),
                    }
                } else if text == "SET_PROFILE" {
                    Err(FrameError::InvalidFormat)
                } else {
                    Err(FrameError::UnknownCommand)
                }
            }
        }
    }

    /// Encodes a typed response into a newline-terminated ASCII wire frame.
    pub fn encode_response(
        response: crate::messages::Response,
    ) -> Result<EncodedFrame, FrameError> {
        let mut writer = FrameWriter::new();

        match response {
            crate::messages::Response::Info(info) => {
                writer.push_bytes(b"INFO|");
                writer.push_bytes(info.firmware_name.as_bytes());
                writer.push_byte(b'|');
                writer.push_u8_decimal(info.protocol_version.major);
                writer.push_byte(b'|');
                writer.push_u8_decimal(info.protocol_version.minor);
                writer.push_byte(b'|');
                writer.push_bytes(profile_str(info.active_profile).as_bytes());
                writer.push_byte(b'|');
                writer.push_bytes(persona_str(info.output_persona).as_bytes());
                writer.push_byte(FRAME_TERMINATOR);
            }
            crate::messages::Response::Status(status) => {
                writer.push_bytes(b"STATUS|");
                writer.push_bytes(profile_str(status.active_profile).as_bytes());
                writer.push_byte(b'|');
                writer.push_bytes(persona_str(status.output_persona).as_bytes());
                writer.push_byte(b'|');
                writer.push_bytes(ble_link_state_str(status.ble_link_state).as_bytes());
                writer.push_byte(b'|');
                writer.push_bytes(bool_str(status.bonds_present).as_bytes());
                writer.push_byte(FRAME_TERMINATOR);
            }
            crate::messages::Response::Profile { active_profile } => {
                writer.push_bytes(b"PROFILE|");
                writer.push_bytes(profile_str(active_profile).as_bytes());
                writer.push_byte(FRAME_TERMINATOR);
            }
            crate::messages::Response::Ack => {
                writer.push_bytes(b"ACK\n");
            }
            crate::messages::Response::Error(error) => {
                writer.push_bytes(b"ERROR|");
                writer.push_bytes(error_code_str(error).as_bytes());
                writer.push_byte(FRAME_TERMINATOR);
            }
        }

        writer.finish()
    }
}

/// Typed request and response contracts for the lean v1 control plane.
pub mod messages {
    /// The stable protocol name for the lean v1 control plane.
    pub const PROTOCOL_NAME: &str = "usb2ble-proto";

    /// The current major protocol version.
    pub const PROTOCOL_VERSION_MAJOR: u8 = 1;

    /// The current minor protocol version.
    pub const PROTOCOL_VERSION_MINOR: u8 = 0;

    /// Identifies a protocol version.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ProtocolVersion {
        /// The major protocol version.
        pub major: u8,
        /// The minor protocol version.
        pub minor: u8,
    }

    impl ProtocolVersion {
        /// Returns the current protocol version.
        pub const fn current() -> Self {
            Self {
                major: PROTOCOL_VERSION_MAJOR,
                minor: PROTOCOL_VERSION_MINOR,
            }
        }
    }

    /// Represents the current BLE link state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BleLinkState {
        /// No BLE activity is active.
        Idle,
        /// The device is advertising.
        Advertising,
        /// The device has an active BLE connection.
        Connected,
    }

    /// Static information about the device and protocol contract.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DeviceInfo {
        /// The protocol version supported by the device.
        pub protocol_version: ProtocolVersion,
        /// The stable firmware identity string.
        pub firmware_name: &'static str,
        /// The active normalized input profile.
        pub active_profile: usb2ble_core::profile::ProfileId,
        /// The downstream output persona targeted by the active profile.
        pub output_persona: usb2ble_core::profile::OutputPersona,
    }

    /// Dynamic device status exposed by the control plane.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DeviceStatus {
        /// The active normalized input profile.
        pub active_profile: usb2ble_core::profile::ProfileId,
        /// The downstream output persona targeted by the active profile.
        pub output_persona: usb2ble_core::profile::OutputPersona,
        /// The current BLE link state.
        pub ble_link_state: BleLinkState,
        /// Whether any persisted bonds are present.
        pub bonds_present: bool,
    }

    /// Commands accepted by the lean v1 control plane.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Command {
        /// Requests static protocol and firmware information.
        GetInfo,
        /// Requests dynamic status information.
        GetStatus,
        /// Requests the active profile only.
        GetProfile,
        /// Requests that the active profile be changed.
        SetProfile {
            /// The profile to activate.
            profile: usb2ble_core::profile::ProfileId,
        },
        /// Requests a device reboot.
        Reboot,
        /// Requests removal of stored bonds.
        ForgetBonds,
    }

    /// Error codes returned by the lean v1 control plane.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ErrorCode {
        /// The requested profile is not supported.
        UnsupportedProfile,
        /// The request is malformed or unsupported in its current shape.
        InvalidRequest,
        /// An internal error occurred while processing the request.
        Internal,
    }

    /// Responses returned by the lean v1 control plane.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Response {
        /// Static device and protocol information.
        Info(DeviceInfo),
        /// Dynamic device status.
        Status(DeviceStatus),
        /// The active profile value.
        Profile {
            /// The currently active profile.
            active_profile: usb2ble_core::profile::ProfileId,
        },
        /// A generic success acknowledgment.
        Ack,
        /// A protocol-level error.
        Error(ErrorCode),
    }

    /// Returns the fixed default device information for lean v1.
    pub fn default_device_info() -> DeviceInfo {
        DeviceInfo {
            protocol_version: ProtocolVersion::current(),
            firmware_name: "usb2ble-fw",
            active_profile: usb2ble_core::profile::V1_PROFILE_ID,
            output_persona: usb2ble_core::profile::V1_PROFILE_ID.output_persona(),
        }
    }

    /// Returns the fixed default dynamic device status for lean v1.
    pub fn default_device_status() -> DeviceStatus {
        DeviceStatus {
            active_profile: usb2ble_core::profile::V1_PROFILE_ID,
            output_persona: usb2ble_core::profile::V1_PROFILE_ID.output_persona(),
            ble_link_state: BleLinkState::Idle,
            bonds_present: false,
        }
    }
}

/// Minimal profile bundle contracts for the lean v1 control plane.
pub mod bundle {
    /// The minimal persisted profile bundle for lean v1.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ProfileBundle {
        /// The active normalized input profile.
        pub active_profile: usb2ble_core::profile::ProfileId,
    }

    impl ProfileBundle {
        /// Returns the fixed lean v1 default profile bundle.
        pub const fn v1_default() -> Self {
            Self {
                active_profile: usb2ble_core::profile::V1_PROFILE_ID,
            }
        }
    }
}

/// Crate identity used by bootstrap verification.
pub const PROTO_CRATE_NAME: &str = "usb2ble-proto";

#[cfg(test)]
mod tests {
    use super::bundle::ProfileBundle;
    use super::framing::{decode_command, encode_response, EncodedFrame, FrameError};
    use super::messages::{
        default_device_info, default_device_status, BleLinkState, Command, DeviceStatus, ErrorCode,
        ProtocolVersion, Response,
    };
    use super::PROTO_CRATE_NAME;
    use usb2ble_core::profile::{OutputPersona, V1_PROFILE_ID};

    fn encoded_frame(response: Response) -> EncodedFrame {
        match encode_response(response) {
            Ok(frame) => frame,
            Err(error) => panic!("failed to encode response: {error:?}"),
        }
    }

    #[test]
    fn proto_crate_name_matches_expected() {
        assert_eq!(PROTO_CRATE_NAME, "usb2ble-proto");
    }

    #[test]
    fn current_protocol_version_matches_v1() {
        let version = ProtocolVersion::current();

        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
    }

    #[test]
    fn default_device_info_uses_expected_firmware_name() {
        let info = default_device_info();

        assert_eq!(info.firmware_name, "usb2ble-fw");
    }

    #[test]
    fn default_device_info_uses_v1_profile() {
        let info = default_device_info();

        assert_eq!(info.active_profile, V1_PROFILE_ID);
    }

    #[test]
    fn default_device_info_uses_generic_ble_gamepad_persona() {
        let info = default_device_info();

        assert_eq!(info.output_persona, OutputPersona::GenericBleGamepad16);
    }

    #[test]
    fn default_device_status_starts_idle() {
        let status = default_device_status();

        assert_eq!(status.ble_link_state, BleLinkState::Idle);
    }

    #[test]
    fn default_device_status_starts_without_bonds() {
        let status = default_device_status();

        assert!(!status.bonds_present);
    }

    #[test]
    fn set_profile_command_preserves_profile_value() {
        let command = Command::SetProfile {
            profile: V1_PROFILE_ID,
        };

        match command {
            Command::SetProfile { profile } => assert_eq!(profile, V1_PROFILE_ID),
            _ => panic!("set profile command did not preserve the profile value"),
        }
    }

    #[test]
    fn profile_bundle_v1_default_uses_v1_profile() {
        let bundle = ProfileBundle::v1_default();

        assert_eq!(bundle.active_profile, V1_PROFILE_ID);
    }

    #[test]
    fn decode_get_info_command() {
        assert_eq!(decode_command(b"GET_INFO\n"), Ok(Command::GetInfo));
    }

    #[test]
    fn decode_get_status_command() {
        assert_eq!(decode_command(b"GET_STATUS\n"), Ok(Command::GetStatus));
    }

    #[test]
    fn decode_get_profile_command() {
        assert_eq!(decode_command(b"GET_PROFILE\n"), Ok(Command::GetProfile));
    }

    #[test]
    fn decode_set_profile_command() {
        assert_eq!(
            decode_command(b"SET_PROFILE|t16000m_v1\n"),
            Ok(Command::SetProfile {
                profile: V1_PROFILE_ID,
            })
        );
    }

    #[test]
    fn decode_reboot_command() {
        assert_eq!(decode_command(b"REBOOT\n"), Ok(Command::Reboot));
    }

    #[test]
    fn decode_forget_bonds_command() {
        assert_eq!(decode_command(b"FORGET_BONDS\n"), Ok(Command::ForgetBonds));
    }

    #[test]
    fn decode_empty_input_returns_empty_error() {
        assert_eq!(decode_command(b""), Err(FrameError::Empty));
    }

    #[test]
    fn decode_missing_terminator_returns_error() {
        assert_eq!(
            decode_command(b"GET_INFO"),
            Err(FrameError::MissingTerminator)
        );
    }

    #[test]
    fn decode_unknown_command_returns_error() {
        assert_eq!(decode_command(b"NOPE\n"), Err(FrameError::UnknownCommand));
    }

    #[test]
    fn decode_malformed_set_profile_returns_invalid_format() {
        assert_eq!(
            decode_command(b"SET_PROFILE\n"),
            Err(FrameError::InvalidFormat)
        );
    }

    #[test]
    fn decode_unsupported_profile_returns_error() {
        assert_eq!(
            decode_command(b"SET_PROFILE|other\n"),
            Err(FrameError::UnsupportedProfile)
        );
    }

    #[test]
    fn encode_ack_response_matches_exact_bytes() {
        let frame = encoded_frame(Response::Ack);

        assert_eq!(frame.as_bytes(), b"ACK\n");
    }

    #[test]
    fn encode_internal_error_response_matches_exact_bytes() {
        let frame = encoded_frame(Response::Error(ErrorCode::Internal));

        assert_eq!(frame.as_bytes(), b"ERROR|internal\n");
    }

    #[test]
    fn encode_profile_response_matches_exact_bytes() {
        let frame = encoded_frame(Response::Profile {
            active_profile: V1_PROFILE_ID,
        });

        assert_eq!(frame.as_bytes(), b"PROFILE|t16000m_v1\n");
    }

    #[test]
    fn encode_info_response_matches_exact_bytes() {
        let frame = encoded_frame(Response::Info(default_device_info()));

        assert_eq!(
            frame.as_bytes(),
            b"INFO|usb2ble-fw|1|0|t16000m_v1|generic_ble_gamepad_16\n"
        );
    }

    #[test]
    fn encode_status_response_matches_exact_bytes() {
        let frame = encoded_frame(Response::Status(DeviceStatus {
            active_profile: V1_PROFILE_ID,
            output_persona: OutputPersona::GenericBleGamepad16,
            ble_link_state: BleLinkState::Advertising,
            bonds_present: true,
        }));

        assert_eq!(
            frame.as_bytes(),
            b"STATUS|t16000m_v1|generic_ble_gamepad_16|advertising|1\n"
        );
    }

    #[test]
    fn encoded_frame_helpers_return_exact_slice_metadata() {
        let frame = encoded_frame(Response::Ack);

        assert_eq!(frame.len(), 4);
        assert!(!frame.is_empty());
        assert_eq!(frame.as_bytes(), b"ACK\n");
    }
}
