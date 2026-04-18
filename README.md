# USBLERST

USBLERST is a lean v1 Rust workspace for an ESP32-S3 USB-HID-to-BLE-gamepad bridge. The immediate goal is a compilable bootstrap foundation with strict linting, placeholder crates, and verification gates in place before any USB host, BLE HID, or NVS behavior is introduced.

## Workspace Layout

- `crates/usb2ble-fw`: host-compilable firmware bootstrap binary that wires the workspace crates together.
- `crates/usb2ble-core`: core logic surface for HID descriptor handling, HID decode, normalization, runtime flow, profiles, and quirks.
- `crates/usb2ble-proto`: internal protocol stubs for framing, messages, and bundle handling.
- `crates/usb2ble-platform-espidf`: ESP-IDF-facing platform layer for USB host, BLE HID, NVS, and UART console integration.

## V1 Scope

- one direct-attached USB HID joystick-class device
- one fixed generic BLE HID gamepad persona
- persisted bond + active profile
- no multi-device merge in v1

## Status

This repository is in the bootstrap phase.

## Host Demo

Run the host-side deterministic vertical slice demo:
```bash
cargo run -p usb2ble-fw -- --demo-host
```
This demonstrates the end-to-end app pipeline from boot through console commands and USB ingress reports to persona-encoded BLE wire bytes.

## Hardware Bridge Demo Loop

For ESP-IDF targets, the firmware defaults to an on-device bridge demo loop. This milestone processes real USB HID events on-device and logs the resulting BLE output contract, proving the end-to-end Rust core pipeline on real hardware.

### How to test on hardware right now

1.  **Flash and Open Console**: Flash the firmware to an ESP32-S3 and open the serial console (e.g., `espflash monitor`).
2.  **Verify Startup**: Look for the startup banner displaying the firmware name, active profile, and backend status. Note that the BLE backend is currently in **recording-fallback** mode as the hardware send path is not yet wired.
3.  **Connect USB HID**: Plug a supported USB HID joystick or gamepad into the ESP32-S3 USB host port.
4.  **Observe Logs**:
    -   `usb attach`: Confirms the device was detected (includes VID/PID).
    -   `usb descriptor stored`: Confirms the HID descriptor was successfully parsed on-device.
    -   `bridge publish`: Observe real-time logs as you move sticks or press buttons. These lines show the output persona, typed normalized report fields, and the exact encoded BLE wire contract.
5.  **Interactive Commands**: Send protocol commands (e.g., `GET_STATUS`, `GET_INFO`) over UART (newline-terminated).

### Current Milestone Scope

-   **Bridge Pipeline**: Full parsing, decoding, and normalization of hardware USB HID reports.
-   **BLE Output Contract**: Deterministic encoding of BLE reports for the active persona.
-   **BLE Transport**: Currently using a structural **recording-fallback** sink. Radio transmission is not part of this milestone.
-   **Output Persona**: Fixed to `generic_ble_gamepad_16`.

After flashing, you can interact with the firmware over the default serial console using the internal protocol commands.

### Example Commands

- `GET_INFO`: Returns firmware identity and contract info.
- `GET_STATUS`: Returns current BLE state, bond presence, and active profile.
- `GET_PROFILE`: Returns the active profile ID.
- `SET_PROFILE|t16000m_v1`: Persists a new active profile to NVS.
- `FORGET_BONDS`: Clears all persisted BLE bonds from NVS.

## Host Replay Mode

Iterate quickly by replaying scripted USB events:
```bash
cargo run -p usb2ble-fw -- --replay-host path/to/script.txt
```

### Replay Fixtures

Committed regression fixtures are available in `fixtures/replay/`:

- `xy_input.txt`: Attaches a device, provides a descriptor, and sends a known X/Y report.
- `xy_input_detach.txt`: Same as above, but adds a detach command to verify state reset.

Run them with:
```bash
cargo run -p usb2ble-fw -- --replay-host fixtures/replay/xy_input.txt
cargo run -p usb2ble-fw -- --replay-host fixtures/replay/xy_input_detach.txt
```

### Script Format

- `ATTACH <device_id> <vendor_id> <product_id>`
- `DESCRIPTOR <hex_bytes>`
- `INPUT <report_id> <hex_bytes>`
- `DETACH <device_id>`

Example `script.txt`:
```text
# Attach a device
ATTACH 101 1 2

# Provide a report descriptor (Generic Desktop Gamepad)
DESCRIPTOR 05 01 09 05 A1 01 05 01 09 30 09 31 15 81 25 7F 75 08 95 02 81 02 C0

# Send an input report (X=5, Y=-10)
INPUT 00 05 F6

# Detach the device
DETACH 101
```

## Development / CI

In constrained cloud-agent environments where local Rust tooling is unavailable, GitHub Actions is the verification source of truth for this repository.

See `docs/CLOUD_AGENT_DEVELOPMENT.md` for the required development flow and verification gate.
