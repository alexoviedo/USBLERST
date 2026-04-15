# Replay Fixtures

These fixtures are used for regression testing the USB-to-BLE bridge app logic.
They are text-based scripts that simulate USB events.

## Running Fixtures

You can run these fixtures manually using the `--replay-host` flag:

```bash
cargo run -p usb2ble-fw -- --replay-host fixtures/replay/xy_input.txt
cargo run -p usb2ble-fw -- --replay-host fixtures/replay/xy_input_detach.txt
```

## Adding New Fixtures

Future real device captures can be added here as text scripts following the `ATTACH`, `DESCRIPTOR`, `INPUT`, `DETACH` format.
Refer to the main `README.md` for more details on the script format.
