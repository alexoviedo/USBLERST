# USBLERST

USBLERST is a lean v1 Rust workspace for an ESP32-S3 USB-HID-to-BLE-gamepad bridge. This milestone provides an on-device bridge demo with real BLE HID transport.

## Workspace Layout

- `crates/usb2ble-fw`: host-compilable firmware binary that wires the workspace crates together.
- `crates/usb2ble-core`: core logic surface for HID descriptor handling, HID decode, normalization, and bridge routing.
- `crates/usb2ble-proto`: internal protocol for framing, messages, and command/response handling.
- `crates/usb2ble-platform-espidf`: ESP-IDF-facing platform layer for USB host, BLE HID, NVS, and UART console integration.

## V1 Scope

- one direct-attached USB HID joystick-class device
- one fixed generic BLE HID gamepad persona
- persisted active profile
- no multi-device merge in v1

## Status

**Milestone: Real BLE HID Transport**
- Full USB HID path exercised on-device (attach, descriptor fetch, live input).
- Rust core pipeline parses, normalizes, and maps reports to the BLE contract.
- Real BLE HID transport active on ESP-IDF targets using Bluedroid.
- Advertising as "USBLERST Gamepad" with Generic Gamepad appearance.

## Host Demo

Run the host-side deterministic vertical slice demo:
```bash
cargo run -p usb2ble-fw -- --demo-host
```
This demonstrates the end-to-end app pipeline from boot through console commands and USB ingress reports to persona-encoded BLE wire bytes.

## Hardware Bridge Demo Loop

For ESP-IDF targets, the firmware defaults to an on-device bridge demo loop. This milestone processes real USB HID events on-device and transmits them over BLE HID to a connected host.

### How to test on hardware

1.  **Flash and Open Console**: Flash the firmware to an ESP32-S3 and open the serial console (e.g., `espflash monitor`).
2.  **Pair with Host**: Search for Bluetooth devices on your PC or phone. Connect to "USBLERST Gamepad".
3.  **Connect USB HID**: Plug a supported USB HID joystick or gamepad into the ESP32-S3 USB host port.
4.  **Observe Logs**:
    -   `usb attach`: Confirms the device was detected.
    -   `bridge publish [REAL]`: Confirms reports are being transmitted over the real BLE radio.
5.  **Interactive Commands**: Send protocol commands (e.g., `GET_STATUS`, `GET_INFO`) over UART (newline-terminated).

### Current Milestone Scope & Limitations

-   **Bridge Pipeline**: Full parsing, decoding, and normalization of hardware USB HID reports.
-   **BLE Transport**: Real HID-over-GATT (HoG) transmission using the Bluedroid stack.
-   **Security**: Minimal security for v1. No active encryption/bonding integration in this step.
-   **Bond Management**: Commands like `FORGET_BONDS` and persisted bond state are structural placeholders in v1 and do not yet clear the internal Bluedroid bond table.
-   **Output Persona**: Fixed to `generic_ble_gamepad_16`.

### Example Commands

- `GET_INFO`: Returns firmware identity and contract info.
- `GET_STATUS`: Returns current BLE state and active profile.
- `GET_PROFILE`: Returns the active profile ID.
- `SET_PROFILE|t16000m_v1`: Persists a new active profile to NVS.

## Host Replay Mode

Iterate quickly by replaying scripted USB events:
```bash
cargo run -p usb2ble-fw -- --replay-host path/to/script.txt
```

## Development / CI

CI validates both host compilation and the embedded code path using target-aware checks.

See `docs/CLOUD_AGENT_DEVELOPMENT.md` for more details.
