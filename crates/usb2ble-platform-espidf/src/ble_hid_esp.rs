//! Structural ESP-IDF BLE HID backend using Bluedroid.

use crate::ble_hid::{
    BleConnectionState, BleInitError, BlePersonaOutput, BlePublishError, EncodedBleInputReport,
};
use usb2ble_core::profile::OutputPersona;

/// Structural ESP-IDF BLE persona output using Bluedroid HID Device API.
pub struct EspBlePersonaOutput {
    state: BleConnectionState,
}

impl EspBlePersonaOutput {
    /// Attempts to initialize the BLE stack and register the generic gamepad v1 persona.
    pub fn new_generic_gamepad_v1() -> Result<Self, BleInitError> {
        // Strategic guidance: This backend is structural for this step.
        // It honestly returns an error because the real hardware send path is not yet wired.
        // Returning Err(BleInitError::HidDevice) ensures the firmware reports 'RECORDING-FALLBACK'
        // and does not fake success.
        Err(BleInitError::HidDevice)
    }
}

impl BlePersonaOutput for EspBlePersonaOutput {
    fn publish_encoded_report(
        &mut self,
        _persona: OutputPersona,
        _report: EncodedBleInputReport,
    ) -> Result<(), BlePublishError> {
        // Strategic guidance: Reject not-connected state.
        if self.state != BleConnectionState::Connected {
            return Err(BlePublishError::NotReady);
        }

        // Real hardware send would go here in the next step.
        // For now, since init fails, this code path is not reached in the demo loop.
        Err(BlePublishError::Transport)
    }

    fn connection_state(&self) -> BleConnectionState {
        self.state
    }
}
