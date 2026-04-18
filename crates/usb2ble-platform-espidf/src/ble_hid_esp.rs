//! Real ESP-IDF BLE HID backend using Bluedroid.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::ble_hid::{
    BleConnectionState, BleInitError, BlePersonaOutput, BlePublishError, EncodedBleInputReport,
    GENERIC_BLE_GAMEPAD16_REPORT_MAP,
};
use usb2ble_core::profile::OutputPersona;

/// Internal atomic state for BLE connection tracking.
static CONNECTION_STATE: AtomicU8 = AtomicU8::new(STATE_IDLE);

const STATE_IDLE: u8 = 0;
const STATE_ADVERTISING: u8 = 1;
const STATE_CONNECTED: u8 = 2;

fn set_state(state: BleConnectionState) {
    let val = match state {
        BleConnectionState::Idle => STATE_IDLE,
        BleConnectionState::Advertising => STATE_ADVERTISING,
        BleConnectionState::Connected => STATE_CONNECTED,
    };
    CONNECTION_STATE.store(val, Ordering::SeqCst);
}

fn get_state() -> BleConnectionState {
    match CONNECTION_STATE.load(Ordering::SeqCst) {
        STATE_ADVERTISING => BleConnectionState::Advertising,
        STATE_CONNECTED => BleConnectionState::Connected,
        _ => BleConnectionState::Idle,
    }
}

/// Structural ESP-IDF BLE persona output using Bluedroid HID Device API.
pub struct EspBlePersonaOutput {
    // We keep this to satisfy the trait and potential future per-instance state.
}

impl EspBlePersonaOutput {
    /// Attempts to initialize the BLE stack and register the generic gamepad v1 persona.
    pub fn new_generic_gamepad_v1() -> Result<Self, BleInitError> {
        unsafe {
            // 1. Initialize Bluetooth Controller
            // Best-effort initialization for ESP32-S3 avoiding mem::zeroed if possible.
            // Since we don't have the macro, we populate the known required fields for S3.
            let mut bt_cfg = esp_idf_sys::esp_bt_controller_config_t {
                magic: 0x20220615, // ESP_BT_CTRL_CONFIG_MAGIC_VAL
                version: 0x01,     // ESP_BT_CTRL_CONFIG_VERSION
                controller_task_stack_size: 4096,
                controller_task_prio: 20,
                controller_task_run_cpu: 0,
                bluetooth_mode: 0x01, // ESP_BT_MODE_BLE
                ble_max_act: 10,
                sleep_mode: 0x01,
                sleep_clock: 0x00,
                ble_st_acl_tx_buf_nb: 0,
                ble_hw_cca_check: 0,
                ble_adv_dup_filt_max: 30,
                coex_param_en: false,
                ce_len_type: 0,
                coex_use_hooks: false,
                hci_tl_type: 1,
                hci_tl_funcs: std::ptr::null_mut(),
                txant_dft: 0,
                rxant_dft: 0,
                txpwr_dft: 7,
                cfg_mask: 1,
                scan_duplicate_mode: 0,
                scan_duplicate_type: 0,
                normal_adv_size: 20,
                mesh_adv_size: 0,
                coex_phy_coded_tx_rx_time_limit: 0,
                hw_target_code: 0x01, // BLE_HW_TARGET_CODE_CHIP_ECO0
                slave_ce_len_min: 5,
                hw_recorrect_en: 0,
                cca_thresh: 20,
                ..std::mem::zeroed() // Safety for any unknown trailing fields
            };

            let res = esp_idf_sys::esp_bt_controller_init(&mut bt_cfg);
            if res != esp_idf_sys::ESP_OK {
                return Err(BleInitError::Controller);
            }

            let res = esp_idf_sys::esp_bt_controller_enable(esp_idf_sys::esp_bt_mode_t_ESP_BT_MODE_BLE);
            if res != esp_idf_sys::ESP_OK {
                return Err(BleInitError::Controller);
            }

            // 2. Initialize Bluedroid
            let res = esp_idf_sys::esp_bluedroid_init();
            if res != esp_idf_sys::ESP_OK {
                return Err(BleInitError::Bluedroid);
            }

            let res = esp_idf_sys::esp_bluedroid_enable();
            if res != esp_idf_sys::ESP_OK {
                return Err(BleInitError::Bluedroid);
            }

            // 3. Register HID Callbacks
            let res = esp_idf_sys::esp_hidd_register_callbacks(Some(hidd_event_callback));
            if res != esp_idf_sys::ESP_OK {
                return Err(BleInitError::HidDevice);
            }

            // 4. Initialize HID Device Profile
            let res = esp_idf_sys::esp_hidd_profile_init();
            if res != esp_idf_sys::ESP_OK {
                return Err(BleInitError::HidDevice);
            }

            // 5. Register GAP callback
            let res = esp_idf_sys::esp_ble_gap_register_callback(Some(gap_event_callback));
            if res != esp_idf_sys::ESP_OK {
                return Err(BleInitError::Advertising);
            }

            // 6. Set Device Name
            let name = b"USBLERST Gamepad\0";
            esp_idf_sys::esp_ble_gap_set_device_name(name.as_ptr() as *const i8);

            // 7. Configure HID Device (Descriptor etc)
            // Best-effort: In some ESP-IDF versions, this is done via a config struct.
            // We assume esp_hidd_dev_config_t exists and has a report_map field.
            let mut hid_config = esp_idf_sys::esp_hidd_dev_config_t {
                vendor_id: 0xdead,
                product_id: 0xbeef,
                version: 0x0100,
                appearance: 0x03C4, // Generic Gamepad
                protocol_mode: 0x01, // Report Protocol
                report_map: GENERIC_BLE_GAMEPAD16_REPORT_MAP.as_ptr() as *mut u8,
                report_map_len: GENERIC_BLE_GAMEPAD16_REPORT_MAP.len() as u16,
                ..std::mem::zeroed()
            };

            // We'll try to set this config. If this function doesn't exist, cargo check will tell us.
            // In many examples, it's esp_hidd_dev_config_set.
            esp_idf_sys::esp_hidd_dev_config_set(&mut hid_config);

            Ok(Self {})
        }
    }
}

impl BlePersonaOutput for EspBlePersonaOutput {
    fn publish_encoded_report(
        &mut self,
        persona: OutputPersona,
        report: EncodedBleInputReport,
    ) -> Result<(), BlePublishError> {
        // Explicitly reject unsupported personas as requested.
        if persona != OutputPersona::GenericBleGamepad16 {
            return Err(BlePublishError::NotReady);
        }

        if self.connection_state() != BleConnectionState::Connected {
            return Err(BlePublishError::NotReady);
        }

        unsafe {
            let bytes = report.as_bytes();
            let report_id = bytes[0];
            let data = &bytes[1..];

            let res = esp_idf_sys::esp_hidd_dev_input_report_send(
                0,
                report_id as i32,
                data.as_ptr() as *mut u8,
                data.len() as i32,
            );

            if res != esp_idf_sys::ESP_OK {
                return Err(BlePublishError::Transport);
            }
        }

        Ok(())
    }

    fn connection_state(&self) -> BleConnectionState {
        get_state()
    }
}

unsafe extern "C" fn gap_event_callback(
    event: esp_idf_sys::esp_gap_ble_cb_event_t,
    _param: *mut esp_idf_sys::esp_ble_gap_cb_param_t,
) {
    match event {
        esp_idf_sys::esp_gap_ble_cb_event_t_ESP_GAP_BLE_ADV_START_COMPLETE_EVT => {
            set_state(BleConnectionState::Advertising);
        }
        _ => {}
    }
}

unsafe extern "C" fn hidd_event_callback(
    event: esp_idf_sys::esp_hidd_cb_event_t,
    _param: *mut esp_idf_sys::esp_hidd_cb_param_t,
) {
    match event {
        esp_idf_sys::esp_hidd_cb_event_t_ESP_HIDD_EVENT_REG_FINISH => {
            let mut adv_params: esp_idf_sys::esp_ble_adv_params_t = std::mem::zeroed();
            adv_params.adv_int_min = 0x20;
            adv_params.adv_int_max = 0x40;
            adv_params.adv_type = esp_idf_sys::esp_ble_adv_type_t_ADV_TYPE_IND;
            adv_params.own_addr_type = esp_idf_sys::esp_ble_addr_type_t_BLE_ADDR_TYPE_PUBLIC;
            adv_params.channel_map = esp_idf_sys::esp_ble_adv_channel_t_ADV_CHNL_ALL;
            adv_params.adv_filter_policy = esp_idf_sys::esp_ble_adv_filter_t_ADV_FILTER_ALLOW_SCAN_ANY_CON_ANY;

            esp_idf_sys::esp_ble_gap_start_advertising(&mut adv_params);
        }
        esp_idf_sys::esp_hidd_cb_event_t_ESP_HIDD_EVENT_BLE_CONNECT => {
            set_state(BleConnectionState::Connected);
        }
        esp_idf_sys::esp_hidd_cb_event_t_ESP_HIDD_EVENT_BLE_DISCONNECT => {
            set_state(BleConnectionState::Idle);
        }
        _ => {}
    }
}
