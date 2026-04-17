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

The project is moving toward a v1 release. The core app pipeline is strong and on-device bridge demo is now functional.

## Host Demo

Run the host-side deterministic vertical slice demo:
```bash
cargo run -p usb2ble-fw -- --demo-host
```
This demonstrates the end-to-end app pipeline from boot through console commands and USB ingress reports to persona-encoded BLE wire bytes.

## Hardware Demo Loop

For ESP-IDF targets (ESP32-S3), the firmware enters an end-to-end bridge demo loop. This proves that the Rust core can handle real hardware events on-device.

After flashing, the firmware:
- accepts UART console commands
- detects USB HID attach/detach (with VID/PID)
- fetches the first HID report descriptor
- receives live HID input reports
- routes reports through the pure-Rust bridge logic
- logs the resulting normalized report and the simulated BLE output wire contract

Note: Real BLE transport is not yet implemented. The BLE output is currently routed to a recording sink that logs the wire bytes to the console.

### What to expect on hardware

When you attach a supported joystick, you should see logs like:
```text
usb attach: id=1 vid=0x044F pid=0xB10A
usb descriptor stored: id=1 fields=12
bridge publish: persona=generic_ble_gamepad_16 x=5 y=-10 rz=0 hat=Centered buttons=0x0000 wire=01 05 00 F6 FF 00 00 08 00 00
```

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
