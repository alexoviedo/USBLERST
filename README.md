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

## Development / CI

In constrained cloud-agent environments where local Rust tooling is unavailable, GitHub Actions is the verification source of truth for this repository.

See `docs/CLOUD_AGENT_DEVELOPMENT.md` for the required development flow and verification gate.
