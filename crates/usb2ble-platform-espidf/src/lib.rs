//! ESP-IDF platform stubs for the USB-to-BLE bridge workspace.
//! All `unsafe` code in the project must eventually live in this crate.

/// BLE HID seam contracts for lean v1.
pub mod ble_hid;
/// UART console seam contracts for lean v1.
pub mod console_uart;
/// Profile and bond storage seam contracts for lean v1.
pub mod nvs_store;
/// USB host seam contracts for lean v1.
pub mod usb_host;

/// Crate identity used by bootstrap verification.
pub const PLATFORM_CRATE_NAME: &str = "usb2ble-platform-espidf";

#[cfg(target_os = "espidf")]
/// Links required ESP-IDF runtime patches during embedded startup.
pub fn link_patches_if_needed() {
    // SAFETY: ESP-IDF requires calling this bootstrap hook exactly during startup
    // to ensure required runtime patches are linked into the final firmware image.
    unsafe {
        esp_idf_sys::link_patches();
    }
}

#[cfg(not(target_os = "espidf"))]
/// Host-side no-op stub for the embedded patch-link bootstrap hook.
pub fn link_patches_if_needed() {}

#[cfg(test)]
mod tests {
    use super::ble_hid::{BleConnectionState, BleOutput, BlePublishError, RecordingBleOutput};
    use super::console_uart::{
        CommandSource, ConsoleError, QueuedCommandSource, RecordingResponseSink, ResponseSink,
    };
    use super::nvs_store::{
        BondStore, MemoryBondStore, MemoryProfileStore, ProfileStore, StoreError,
    };
    use super::usb_host::{DeviceMeta, QueuedUsbIngress, UsbDeviceId, UsbEvent, UsbIngress};
    use super::PLATFORM_CRATE_NAME;
    use usb2ble_core::normalize::HatPosition;
    use usb2ble_core::profile::V1_PROFILE_ID;
    use usb2ble_core::runtime::GenericBleGamepad16Report;
    use usb2ble_proto::messages::{Command, ErrorCode, Response};

    fn concrete_report() -> GenericBleGamepad16Report {
        GenericBleGamepad16Report {
            x: 11,
            y: -22,
            rz: 33,
            hat: HatPosition::DownRight,
            buttons: (1_u16 << 0) | (1_u16 << 5),
        }
    }

    #[test]
    fn platform_crate_name_matches_expected() {
        assert_eq!(PLATFORM_CRATE_NAME, "usb2ble-platform-espidf");
    }

    #[test]
    fn usb_device_id_round_trips_raw_value() {
        assert_eq!(UsbDeviceId::new(7).raw(), 7);
    }

    #[test]
    fn device_detached_event_preserves_device_id() {
        let event = UsbEvent::DeviceDetached(UsbDeviceId::new(3));

        match event {
            UsbEvent::DeviceDetached(device_id) => assert_eq!(device_id.raw(), 3),
            _ => panic!("unexpected USB event variant"),
        }
    }

    #[test]
    fn report_descriptor_event_preserves_length_and_prefix_bytes() {
        let mut bytes = [0_u8; 64];
        bytes[0] = 1;
        bytes[1] = 2;
        bytes[2] = 3;
        bytes[3] = 4;

        let event = UsbEvent::ReportDescriptorReceived {
            device_id: UsbDeviceId::new(9),
            bytes,
            len: 4,
        };

        match event {
            UsbEvent::ReportDescriptorReceived {
                device_id,
                bytes,
                len,
            } => {
                assert_eq!(device_id.raw(), 9);
                assert_eq!(len, 4);
                assert_eq!(bytes[0], 1);
                assert_eq!(bytes[1], 2);
                assert_eq!(bytes[2], 3);
                assert_eq!(bytes[3], 4);
            }
            _ => panic!("unexpected USB event variant"),
        }
    }

    #[test]
    fn ble_connection_state_connected_compares_equal() {
        assert_eq!(BleConnectionState::Connected, BleConnectionState::Connected);
    }

    #[test]
    fn ble_publish_error_not_ready_compares_equal() {
        assert_eq!(BlePublishError::NotReady, BlePublishError::NotReady);
    }

    #[test]
    fn store_error_backend_failure_compares_equal() {
        assert_eq!(StoreError::BackendFailure, StoreError::BackendFailure);
    }

    #[test]
    fn console_error_transport_compares_equal() {
        assert_eq!(ConsoleError::Transport, ConsoleError::Transport);
    }

    #[test]
    fn queued_usb_ingress_new_starts_empty() {
        let mut ingress = QueuedUsbIngress::new();

        assert_eq!(ingress.poll_calls(), 0);
        assert_eq!(ingress.poll_event(), None);
        assert_eq!(ingress.poll_calls(), 1);
    }

    #[test]
    fn queued_usb_ingress_with_event_returns_event_once_then_none() {
        let event = UsbEvent::DeviceAttached(DeviceMeta {
            device_id: UsbDeviceId::new(1),
            vendor_id: 0x1234,
            product_id: 0x5678,
        });
        let mut ingress = QueuedUsbIngress::with_event(event);

        assert_eq!(ingress.poll_event(), Some(event));
        assert_eq!(ingress.poll_event(), None);
    }

    #[test]
    fn queued_usb_ingress_queue_event_appends_to_queue() {
        let first = UsbEvent::DeviceDetached(UsbDeviceId::new(1));
        let second = UsbEvent::DeviceDetached(UsbDeviceId::new(2));
        let mut ingress = QueuedUsbIngress::with_event(first);

        ingress.queue_event(second);

        assert_eq!(ingress.poll_event(), Some(first));
        assert_eq!(ingress.poll_event(), Some(second));
        assert_eq!(ingress.poll_event(), None);
    }

    #[test]
    fn queued_usb_ingress_set_event_replaces_queue() {
        let first = UsbEvent::DeviceDetached(UsbDeviceId::new(1));
        let second = UsbEvent::DeviceDetached(UsbDeviceId::new(2));
        let mut ingress = QueuedUsbIngress::with_event(first);

        ingress.set_event(second);

        assert_eq!(ingress.poll_event(), Some(second));
        assert_eq!(ingress.poll_event(), None);
    }

    #[test]
    fn queued_usb_ingress_poll_calls_increment_on_every_poll() {
        let mut ingress = QueuedUsbIngress::new();

        assert_eq!(ingress.poll_calls(), 0);
        assert_eq!(ingress.poll_event(), None);
        assert_eq!(ingress.poll_calls(), 1);
        assert_eq!(ingress.poll_event(), None);
        assert_eq!(ingress.poll_calls(), 2);
    }

    #[test]
    fn recording_ble_output_new_stores_connection_state() {
        let output = RecordingBleOutput::new(BleConnectionState::Advertising);

        assert_eq!(output.connection_state(), BleConnectionState::Advertising);
        assert_eq!(output.last_report(), None);
    }

    #[test]
    fn recording_ble_output_successful_publish_stores_report() {
        let report = concrete_report();
        let mut output = RecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(output.publish_report(report), Ok(()));
        assert_eq!(output.last_report(), Some(report));
    }

    #[test]
    fn recording_ble_output_forced_failure_preserves_prior_report() {
        let first_report = concrete_report();
        let second_report = GenericBleGamepad16Report {
            x: -44,
            y: 55,
            rz: -66,
            hat: HatPosition::Left,
            buttons: 1_u16 << 3,
        };
        let mut output = RecordingBleOutput::new(BleConnectionState::Connected);

        assert_eq!(output.publish_report(first_report), Ok(()));
        output.set_fail_with(BlePublishError::NotReady);

        assert_eq!(
            output.publish_report(second_report),
            Err(BlePublishError::NotReady)
        );
        assert_eq!(output.last_report(), Some(first_report));
    }

    #[test]
    fn recording_ble_output_clear_failure_removes_forced_failure() {
        let report = concrete_report();
        let mut output = RecordingBleOutput::new(BleConnectionState::Connected);

        output.set_fail_with(BlePublishError::NotReady);
        output.clear_failure();

        assert_eq!(output.publish_report(report), Ok(()));
        assert_eq!(output.last_report(), Some(report));
    }

    #[test]
    fn recording_ble_output_clear_last_report_resets_stored_report() {
        let report = concrete_report();
        let mut output = RecordingBleOutput::new(BleConnectionState::Idle);

        assert_eq!(output.publish_report(report), Ok(()));
        output.clear_last_report();

        assert_eq!(output.last_report(), None);
    }

    #[test]
    fn memory_profile_store_new_starts_with_none() {
        let store = MemoryProfileStore::new();

        assert_eq!(store.active_profile(), None);
        assert_eq!(store.load_active_profile(), None);
    }

    #[test]
    fn memory_profile_store_with_profile_loads_that_profile() {
        let store = MemoryProfileStore::with_profile(V1_PROFILE_ID);

        assert_eq!(store.active_profile(), Some(V1_PROFILE_ID));
        assert_eq!(store.load_active_profile(), Some(V1_PROFILE_ID));
    }

    #[test]
    fn memory_profile_store_store_active_profile_persists_profile() {
        let mut store = MemoryProfileStore::new();

        assert_eq!(store.store_active_profile(V1_PROFILE_ID), Ok(()));
        assert_eq!(store.active_profile(), Some(V1_PROFILE_ID));
        assert_eq!(store.load_active_profile(), Some(V1_PROFILE_ID));
    }

    #[test]
    fn memory_bond_store_new_starts_without_bonds() {
        let store = MemoryBondStore::new();

        assert!(!store.bonds_present());
    }

    #[test]
    fn memory_bond_store_with_bonds_present_true_reports_true() {
        let store = MemoryBondStore::with_bonds_present(true);

        assert!(store.bonds_present());
    }

    #[test]
    fn memory_bond_store_clear_bonds_resets_to_false() {
        let mut store = MemoryBondStore::with_bonds_present(true);

        assert_eq!(store.clear_bonds(), Ok(()));
        assert!(!store.bonds_present());
    }

    #[test]
    fn queued_command_source_new_starts_empty() {
        let mut source = QueuedCommandSource::new();

        assert_eq!(source.poll_calls(), 0);
        assert_eq!(source.poll_command(), None);
        assert_eq!(source.poll_calls(), 1);
    }

    #[test]
    fn queued_command_source_with_command_returns_it_once_then_none() {
        let mut source = QueuedCommandSource::with_command(Command::GetInfo);

        assert_eq!(source.poll_command(), Some(Command::GetInfo));
        assert_eq!(source.poll_command(), None);
    }

    #[test]
    fn queued_command_source_queue_command_replaces_queued_command() {
        let mut source = QueuedCommandSource::with_command(Command::GetStatus);

        source.queue_command(Command::GetInfo);

        assert_eq!(source.poll_command(), Some(Command::GetInfo));
        assert_eq!(source.poll_command(), None);
    }

    #[test]
    fn queued_command_source_poll_calls_increment_on_every_poll() {
        let mut source = QueuedCommandSource::new();

        assert_eq!(source.poll_calls(), 0);
        assert_eq!(source.poll_command(), None);
        assert_eq!(source.poll_calls(), 1);
        assert_eq!(source.poll_command(), None);
        assert_eq!(source.poll_calls(), 2);
    }

    #[test]
    fn recording_response_sink_success_records_response_and_increments_send_calls() {
        let mut sink = RecordingResponseSink::new();

        assert_eq!(sink.send_response(Response::Ack), Ok(()));
        assert_eq!(sink.last_response(), Some(Response::Ack));
        assert_eq!(sink.send_calls(), 1);
    }

    #[test]
    fn recording_response_sink_forced_failure_preserves_prior_response() {
        let prior = Response::Ack;
        let next = Response::Error(ErrorCode::InvalidRequest);
        let mut sink = RecordingResponseSink::new();

        assert_eq!(sink.send_response(prior), Ok(()));
        sink.set_fail_with(ConsoleError::Transport);

        assert_eq!(sink.send_response(next), Err(ConsoleError::Transport));
        assert_eq!(sink.send_calls(), 2);
        assert_eq!(sink.last_response(), Some(prior));
    }

    #[test]
    fn recording_response_sink_clear_failure_removes_forced_failure() {
        let mut sink = RecordingResponseSink::new();

        sink.set_fail_with(ConsoleError::Transport);
        sink.clear_failure();

        assert_eq!(sink.send_response(Response::Ack), Ok(()));
        assert_eq!(sink.last_response(), Some(Response::Ack));
    }

    #[test]
    fn recording_response_sink_clear_last_response_resets_stored_response() {
        let mut sink = RecordingResponseSink::new();

        assert_eq!(sink.send_response(Response::Ack), Ok(()));
        sink.clear_last_response();

        assert_eq!(sink.last_response(), None);
    }
}
