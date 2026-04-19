//! Real ESP-IDF BLE HID backend using Bluedroid.

use std::sync::atomic::{AtomicU16, AtomicU8, Ordering};

use crate::ble_hid::{
    BleConnectionState, BleInitError, BlePersonaOutput, BlePublishError, EncodedBleInputReport,
    GENERIC_BLE_GAMEPAD16_REPORT_MAP,
};
use usb2ble_core::profile::OutputPersona;

/// Internal atomic state for BLE connection tracking and initialization status.
static CONNECTION_STATE: AtomicU8 = AtomicU8::new(STATE_IDLE);

const STATE_IDLE: u8 = 0;
const STATE_ADVERTISING: u8 = 1;
const STATE_CONNECTED: u8 = 2;
const STATE_INIT_FAILED: u8 = 3;

/// Tracks the active BLE connection handle.
static CONNECTION_HANDLE: AtomicU16 = AtomicU16::new(0);

fn set_state(state: BleConnectionState) {
    let val = match state {
        BleConnectionState::Idle => STATE_IDLE,
        BleConnectionState::Advertising => STATE_ADVERTISING,
        BleConnectionState::Connected => STATE_CONNECTED,
    };
    CONNECTION_STATE.store(val, Ordering::SeqCst);
}

fn set_init_failed() {
    CONNECTION_STATE.store(STATE_INIT_FAILED, Ordering::SeqCst);
    CONNECTION_HANDLE.store(0, Ordering::SeqCst);
}

fn get_state() -> BleConnectionState {
    match CONNECTION_STATE.load(Ordering::SeqCst) {
        STATE_ADVERTISING => BleConnectionState::Advertising,
        STATE_CONNECTED => BleConnectionState::Connected,
        _ => BleConnectionState::Idle,
    }
}

fn is_init_failed() -> bool {
    CONNECTION_STATE.load(Ordering::SeqCst) == STATE_INIT_FAILED
}

/// Helper to start BLE advertising with fixed parameters for USBLERST.
///
/// # Safety
/// This function calls ESP-IDF GAP APIs and must only be called when the BLE stack
/// is initialized and ready.
unsafe fn start_advertising() {
    // SAFETY: Initializing a C struct with zeroed memory is required by ESP-IDF APIs
    // before setting specific fields.
    let mut adv_params: esp_idf_sys::esp_ble_adv_params_t = unsafe { std::mem::zeroed() };
    adv_params.adv_int_min = 0x20;
    adv_params.adv_int_max = 0x40;
    adv_params.adv_type = esp_idf_sys::esp_ble_adv_type_t_ADV_TYPE_IND;
    adv_params.own_addr_type = esp_idf_sys::esp_ble_addr_type_t_BLE_ADDR_TYPE_PUBLIC;
    adv_params.channel_map = esp_idf_sys::esp_ble_adv_channel_t_ADV_CHNL_ALL;
    adv_params.adv_filter_policy =
        esp_idf_sys::esp_ble_adv_filter_t_ADV_FILTER_ALLOW_SCAN_ANY_CON_ANY;

    // SAFETY: esp_ble_gap_start_advertising is a FFI call to the ESP-IDF SDK.
    let res = unsafe { esp_idf_sys::esp_ble_gap_start_advertising(&mut adv_params) };
    if res != esp_idf_sys::ESP_OK {
        set_init_failed();
    }
}

/// Structural ESP-IDF BLE persona output using Bluedroid HID Device API.
pub struct EspBlePersonaOutput {
    // We keep this to satisfy the trait and potential future per-instance state.
}

impl EspBlePersonaOutput {
    /// Attempts to initialize the BLE stack and register the generic gamepad v1 persona.
    pub fn new_generic_gamepad_v1() -> Result<Self, BleInitError> {
        CONNECTION_HANDLE.store(0, Ordering::SeqCst);

        // 1. Initialize Bluetooth Controller
        // SAFETY: esp_bt_controller_config_t must be initialized before use.
        let mut bt_cfg: esp_idf_sys::esp_bt_controller_config_t = unsafe { std::mem::zeroed() };

        // Population using SDK-supported magic values and constants.
        // These match the defaults for ESP32-S3 in ESP-IDF v5.x.
        bt_cfg.magic = esp_idf_sys::ESP_BT_CTRL_CONFIG_MAGIC_VAL;
        bt_cfg.version = esp_idf_sys::ESP_BT_CTRL_CONFIG_VERSION;
        bt_cfg.controller_task_stack_size = esp_idf_sys::ESP_TASK_BT_CONTROLLER_STACK as u16;
        bt_cfg.controller_task_prio = esp_idf_sys::ESP_TASK_BT_CONTROLLER_PRIO as u8;
        bt_cfg.controller_task_run_cpu = esp_idf_sys::CONFIG_BT_CTRL_PINNED_TO_CORE as u8;
        bt_cfg.bluetooth_mode = esp_idf_sys::esp_bt_mode_t_ESP_BT_MODE_BLE as u8;
        bt_cfg.ble_max_act = esp_idf_sys::CONFIG_BT_CTRL_BLE_MAX_ACT_EFF as u8;
        bt_cfg.sleep_mode = esp_idf_sys::CONFIG_BT_CTRL_SLEEP_MODE_EFF as u8;
        bt_cfg.sleep_clock = esp_idf_sys::CONFIG_BT_CTRL_SLEEP_CLOCK_EFF as u8;
        bt_cfg.ble_st_acl_tx_buf_nb = esp_idf_sys::CONFIG_BT_CTRL_BLE_STATIC_ACL_TX_BUF_NB as u8;
        bt_cfg.ble_hw_cca_check = esp_idf_sys::CONFIG_BT_CTRL_HW_CCA_EFF as u8;
        bt_cfg.ble_adv_dup_filt_max = esp_idf_sys::CONFIG_BT_CTRL_ADV_DUP_FILT_MAX as u16;
        bt_cfg.ce_len_type = esp_idf_sys::CONFIG_BT_CTRL_CE_LENGTH_TYPE_EFF as u8;
        bt_cfg.hci_tl_type = esp_idf_sys::CONFIG_BT_CTRL_HCI_TL_EFF as u8;
        bt_cfg.hci_tl_funcs = std::ptr::null_mut();
        bt_cfg.txant_dft = esp_idf_sys::CONFIG_BT_CTRL_TX_ANTENNA_INDEX_EFF as u8;
        bt_cfg.rxant_dft = esp_idf_sys::CONFIG_BT_CTRL_RX_ANTENNA_INDEX_EFF as u8;
        bt_cfg.txpwr_dft = esp_idf_sys::CONFIG_BT_CTRL_DFT_TX_POWER_LEVEL_EFF as u8;
        bt_cfg.cfg_mask = esp_idf_sys::CFG_MASK as u32;
        bt_cfg.scan_duplicate_mode = esp_idf_sys::SCAN_DUPLICATE_MODE as u8;
        bt_cfg.scan_duplicate_type = esp_idf_sys::SCAN_DUPLICATE_TYPE_VALUE as u8;
        bt_cfg.normal_adv_size = esp_idf_sys::NORMAL_SCAN_DUPLICATE_CACHE_SIZE as u16;
        bt_cfg.mesh_adv_size = esp_idf_sys::MESH_DUPLICATE_SCAN_CACHE_SIZE as u16;
        bt_cfg.coex_phy_coded_tx_rx_time_limit =
            esp_idf_sys::CONFIG_BT_CTRL_COEX_PHY_CODED_TX_RX_TLIM_EFF as u8;
        bt_cfg.hw_target_code = esp_idf_sys::BLE_HW_TARGET_CODE_CHIP_ECO0 as u32;
        bt_cfg.slave_ce_len_min = esp_idf_sys::SLAVE_CE_LEN_MIN_DEFAULT as u16;
        bt_cfg.hw_recorrect_en = esp_idf_sys::AGC_RECORRECT_EN as u8;
        bt_cfg.cca_thresh = esp_idf_sys::CONFIG_BT_CTRL_HW_CCA_VAL as u8;

        // SAFETY: Initializing the BT controller via FFI.
        let res = unsafe { esp_idf_sys::esp_bt_controller_init(&mut bt_cfg) };
        if res != esp_idf_sys::ESP_OK {
            return Err(BleInitError::Controller);
        }

        // SAFETY: Enabling the BT controller for BLE mode.
        let res = unsafe {
            esp_idf_sys::esp_bt_controller_enable(esp_idf_sys::esp_bt_mode_t_ESP_BT_MODE_BLE)
        };
        if res != esp_idf_sys::ESP_OK {
            unsafe {
                let _ = esp_idf_sys::esp_bt_controller_deinit();
            }
            CONNECTION_HANDLE.store(0, Ordering::SeqCst);
            return Err(BleInitError::Controller);
        }

        // 2. Initialize Bluedroid
        // SAFETY: Initializing the Bluedroid stack.
        let res = unsafe { esp_idf_sys::esp_bluedroid_init() };
        if res != esp_idf_sys::ESP_OK {
            unsafe {
                let _ = esp_idf_sys::esp_bt_controller_disable();
                let _ = esp_idf_sys::esp_bt_controller_deinit();
            }
            return Err(BleInitError::Bluedroid);
        }

        // SAFETY: Enabling the Bluedroid stack.
        let res = unsafe { esp_idf_sys::esp_bluedroid_enable() };
        if res != esp_idf_sys::ESP_OK {
            unsafe {
                let _ = esp_idf_sys::esp_bluedroid_deinit();
                let _ = esp_idf_sys::esp_bt_controller_disable();
                let _ = esp_idf_sys::esp_bt_controller_deinit();
            }
            return Err(BleInitError::Bluedroid);
        }

        // 3. Register GAP callback
        // SAFETY: Registering FFI callbacks for GAP events.
        let res = unsafe { esp_idf_sys::esp_ble_gap_register_callback(Some(gap_event_callback)) };
        if res != esp_idf_sys::ESP_OK {
            Self::deinit_stack();
            return Err(BleInitError::Advertising);
        }

        // 4. Register HID Callbacks
        // SAFETY: Registering FFI callbacks for HID events.
        let res = unsafe { esp_idf_sys::esp_hidd_register_callbacks(Some(hidd_event_callback)) };
        if res != esp_idf_sys::ESP_OK {
            Self::deinit_stack();
            return Err(BleInitError::HidDevice);
        }

        // 5. Initialize HID Device Profile
        // SAFETY: Initializing the HID profile.
        let res = unsafe { esp_idf_sys::esp_hidd_profile_init() };
        if res != esp_idf_sys::ESP_OK {
            Self::deinit_stack();
            return Err(BleInitError::HidDevice);
        }

        // 6. Set Device Name
        let name = b"USBLERST Gamepad\0";
        // SAFETY: Setting the BLE device name.
        let res = unsafe { esp_idf_sys::esp_ble_gap_set_device_name(name.as_ptr() as *const i8) };
        if res != esp_idf_sys::ESP_OK {
            Self::deinit_stack();
            return Err(BleInitError::Advertising);
        }

        // 7. Configure HID Device
        // SAFETY: Initializing a C struct for HID configuration.
        let mut hid_config: esp_idf_sys::esp_hidd_dev_config_t = unsafe { std::mem::zeroed() };
        hid_config.vendor_id = 0xdead;
        hid_config.product_id = 0xbeef;
        hid_config.version = 0x0100;
        hid_config.appearance = 0x03C4; // Generic Gamepad
        hid_config.protocol_mode = 0x01; // Report Protocol
        hid_config.report_map = GENERIC_BLE_GAMEPAD16_REPORT_MAP.as_ptr() as *mut u8;
        hid_config.report_map_len = GENERIC_BLE_GAMEPAD16_REPORT_MAP.len() as u16;

        // SAFETY: Setting the HID device configuration.
        let res = unsafe { esp_idf_sys::esp_hidd_dev_config_set(&mut hid_config) };
        if res != esp_idf_sys::ESP_OK {
            Self::deinit_stack();
            return Err(BleInitError::HidDevice);
        }

        Ok(Self {})
    }

    /// Best-effort cleanup of the BLE stack on initialization failure.
    fn deinit_stack() {
        // SAFETY: These are FFI calls to deinitialize the BLE stack.
        // We ignore return values during best-effort cleanup.
        unsafe {
            let _ = esp_idf_sys::esp_bluedroid_disable();
            let _ = esp_idf_sys::esp_bluedroid_deinit();
            let _ = esp_idf_sys::esp_bt_controller_disable();
            let _ = esp_idf_sys::esp_bt_controller_deinit();
        }
        CONNECTION_HANDLE.store(0, Ordering::SeqCst);
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

        let bytes = report.as_bytes();
        let report_id = bytes[0];
        let data = &bytes[1..];
        let handle = CONNECTION_HANDLE.load(Ordering::SeqCst);

        // SAFETY: Sending a HID report over the active connection handle.
        // We use the handle captured during the BLE_CONNECT event.
        let res = unsafe {
            esp_idf_sys::esp_hidd_dev_input_report_send(
                handle,
                report_id as i32,
                data.as_ptr() as *mut u8,
                data.len() as i32,
            )
        };

        if res != esp_idf_sys::ESP_OK {
            return Err(BlePublishError::Transport);
        }

        Ok(())
    }

    fn connection_state(&self) -> BleConnectionState {
        if self.is_failed() {
            return BleConnectionState::Idle;
        }
        get_state()
    }
}

impl EspBlePersonaOutput {
    /// Returns true if the backend has entered a terminal failure state.
    pub fn is_failed(&self) -> bool {
        is_init_failed()
    }
}

/// Configure advertising data for discoverability.
///
/// # Safety
/// This function calls ESP-IDF GAP APIs and should be called after name is set.
unsafe fn config_adv_data() {
    // SAFETY: Initializing C structs for advertising data.
    let mut adv_data: esp_idf_sys::esp_ble_adv_data_t = unsafe { std::mem::zeroed() };
    adv_data.set_scan_rsp = false;
    adv_data.include_name = true;
    adv_data.include_txpower = true;
    adv_data.min_interval = 0x0006;
    adv_data.max_interval = 0x0010;
    adv_data.appearance = 0x03C4; // Gamepad
    adv_data.service_uuid_len = 0;

    // SAFETY: esp_ble_gap_config_adv_data is a FFI call to the ESP-IDF SDK.
    let res = unsafe { esp_idf_sys::esp_ble_gap_config_adv_data(&mut adv_data) };
    if res != esp_idf_sys::ESP_OK {
        set_init_failed();
    }
}

/// GAP event callback to handle advertising status.
unsafe extern "C" fn gap_event_callback(
    event: esp_idf_sys::esp_gap_ble_cb_event_t,
    param: *mut esp_idf_sys::esp_ble_gap_cb_param_t,
) {
    match event {
        esp_idf_sys::esp_gap_ble_cb_event_t_ESP_GAP_BLE_ADV_DATA_SET_COMPLETE_EVT => {
            // Advertising data configured, now we can start advertising.
            // SAFETY: Checking status before starting.
            if !param.is_null() {
                let status = unsafe { (*param).adv_data_cmpl.status };
                if status == esp_idf_sys::ESP_BT_STATUS_SUCCESS {
                    unsafe {
                        start_advertising();
                    }
                } else {
                    set_init_failed();
                }
            } else {
                set_init_failed();
            }
        }
        esp_idf_sys::esp_gap_ble_cb_event_t_ESP_GAP_BLE_ADV_START_COMPLETE_EVT => {
            // Check advertising start status from the parameters.
            // SAFETY: param is guaranteed non-null for this event in ESP-IDF.
            if !param.is_null() {
                let status = unsafe { (*param).adv_start_cmpl.status };
                if status == esp_idf_sys::ESP_BT_STATUS_SUCCESS {
                    set_state(BleConnectionState::Advertising);
                } else {
                    set_init_failed();
                }
            }
        }
        _ => {}
    }
}

/// HID Device event callback to handle connection status and initialization.
unsafe extern "C" fn hidd_event_callback(
    event: esp_idf_sys::esp_hidd_cb_event_t,
    param: *mut esp_idf_sys::esp_hidd_cb_param_t,
) {
    match event {
        esp_idf_sys::esp_hidd_cb_event_t_ESP_HIDD_EVENT_REG_FINISH => {
            // Check HID init result before treating backend as ready.
            // SAFETY: Dereferencing param after null check.
            if !param.is_null() {
                let status = unsafe { (*param).reg_finish.state };
                if status == esp_idf_sys::esp_hidd_init_state_t_ESP_HIDD_INIT_OK {
                    // SAFETY: Configure advertising data once the HID profile is registered.
                    unsafe {
                        config_adv_data();
                    }
                } else {
                    set_init_failed();
                }
            } else {
                set_init_failed();
            }
        }
        esp_idf_sys::esp_hidd_cb_event_t_ESP_HIDD_EVENT_BLE_CONNECT => {
            // SAFETY: Dereferencing param after null check to get connection handle.
            if !param.is_null() {
                let handle = unsafe { (*param).connect.conn_id };
                CONNECTION_HANDLE.store(handle, Ordering::SeqCst);
            }
            set_state(BleConnectionState::Connected);
        }
        esp_idf_sys::esp_hidd_cb_event_t_ESP_HIDD_EVENT_BLE_DISCONNECT => {
            CONNECTION_HANDLE.store(0, Ordering::SeqCst);
            set_state(BleConnectionState::Idle);
            // Restart advertising upon disconnect to maintain discoverability.
            // SAFETY: Calling start_advertising via FFI.
            unsafe {
                start_advertising();
            }
        }
        _ => {}
    }
}
