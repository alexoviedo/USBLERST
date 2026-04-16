/// Identifies a USB device within the platform layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbDeviceId(u8);

impl UsbDeviceId {
    /// Creates a new platform-local USB device identifier.
    pub fn new(raw: u8) -> Self {
        Self(raw)
    }

    /// Returns the raw platform-local USB device identifier value.
    pub fn raw(self) -> u8 {
        self.0
    }
}

/// Minimal metadata about an attached USB device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceMeta {
    /// The platform-local USB device identifier.
    pub device_id: UsbDeviceId,
    /// The USB vendor identifier.
    pub vendor_id: u16,
    /// The USB product identifier.
    pub product_id: u16,
}

/// USB-side ingress events exposed by the platform seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbEvent {
    /// A USB device has been attached.
    DeviceAttached(DeviceMeta),
    /// A report descriptor chunk has been received.
    ReportDescriptorReceived {
        /// The source device identifier.
        device_id: UsbDeviceId,
        /// The fixed descriptor buffer.
        bytes: [u8; 64],
        /// The number of valid bytes in `bytes`.
        len: usize,
    },
    /// An input report has been received.
    InputReportReceived {
        /// The source device identifier.
        device_id: UsbDeviceId,
        /// The HID report identifier.
        report_id: u8,
        /// The fixed report buffer.
        bytes: [u8; 64],
        /// The number of valid bytes in `bytes`.
        len: usize,
    },
    /// A USB device has been detached.
    DeviceDetached(UsbDeviceId),
}

/// Poll-based USB ingress boundary for the future ESP-IDF host glue.
pub trait UsbIngress {
    /// Returns the next available USB event, if any.
    fn poll_event(&mut self) -> Option<UsbEvent>;
}

/// In-memory multi-slot USB ingress adapter for host-side use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedUsbIngress {
    events: std::collections::VecDeque<UsbEvent>,
    poll_calls: usize,
}

impl Default for QueuedUsbIngress {
    fn default() -> Self {
        Self::new()
    }
}

impl QueuedUsbIngress {
    /// Creates an empty queued ingress adapter.
    pub fn new() -> Self {
        Self {
            events: std::collections::VecDeque::new(),
            poll_calls: 0,
        }
    }

    /// Creates an ingress adapter with one queued event.
    pub fn with_event(event: UsbEvent) -> Self {
        let mut events = std::collections::VecDeque::new();
        events.push_back(event);
        Self {
            events,
            poll_calls: 0,
        }
    }

    /// Queues an event to be returned.
    pub fn queue_event(&mut self, event: UsbEvent) {
        self.events.push_back(event);
    }

    /// Replaces the entire queue with a single event.
    pub fn set_event(&mut self, event: UsbEvent) {
        self.events.clear();
        self.events.push_back(event);
    }

    /// Returns how many times the ingress has been polled.
    pub fn poll_calls(&self) -> usize {
        self.poll_calls
    }
}

impl UsbIngress for QueuedUsbIngress {
    fn poll_event(&mut self) -> Option<UsbEvent> {
        self.poll_calls += 1;
        self.events.pop_front()
    }
}

/// Errors that can occur when using the USB host boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbHostError {
    /// The USB host transport is not ready.
    NotReady,
    /// Installing the USB host stack failed.
    Install,
    /// Registering a USB host client failed.
    ClientRegister,
    /// Obtaining device information failed.
    DeviceInfo,
    /// A transport-level failure occurred.
    Transport,
}

/// ESP-IDF-backed USB host ingress for embedded builds.
#[cfg(target_os = "espidf")]
pub struct EspUsbHostIngress {
    client_hdl: esp_idf_sys::usb_host_client_handle_t,
    events: std::collections::VecDeque<UsbEvent>,
}

/// Host stub for the ESP-IDF-backed USB host ingress.
#[cfg(not(target_os = "espidf"))]
pub struct EspUsbHostIngress;

#[cfg(target_os = "espidf")]
impl EspUsbHostIngress {
    /// Initializes the USB host stack and registers a single client.
    pub fn new_single_client() -> Result<Self, UsbHostError> {
        // SAFETY: USB host installation and client registration are standard ESP-IDF calls.
        unsafe {
            let config = esp_idf_sys::usb_host_config_t {
                skip_phy_setup: false,
                intr_flags: esp_idf_sys::ESP_INTR_FLAG_LEVEL1 as i32,
                enum_filter_cb: None,
                enum_filter_cb_arg: std::ptr::null_mut(),
            };

            let res = esp_idf_sys::usb_host_install(&config);
            if res != esp_idf_sys::ESP_OK {
                // If already installed, we might want to continue, but for smoke test we expect clean start.
                return Err(UsbHostError::Install);
            }

            let client_config = esp_idf_sys::usb_host_client_config_t {
                is_within_static_size: false,
                max_num_event_msg: 5,
                client_event_callback: Some(client_event_cb),
                callback_arg: std::ptr::null_mut(),
            };

            let mut client_hdl: esp_idf_sys::usb_host_client_handle_t = std::ptr::null_mut();
            let res = esp_idf_sys::usb_host_client_register(&client_config, &mut client_hdl);
            if res != esp_idf_sys::ESP_OK {
                return Err(UsbHostError::ClientRegister);
            }

            // Store the handle in a way the callback can access it if needed,
            // or just rely on the fact that we have one global client for now.
            // For this smoke test, we'll use a static to pass events to the instance.
            // This is NOT ideal for production but acceptable for a "first hardware signal" smoke path.
            GLOBAL_INGRESS_QUEUE = Some(std::collections::VecDeque::new());

            Ok(Self {
                client_hdl,
                events: std::collections::VecDeque::new(),
            })
        }
    }

    /// Services the USB host stack and client handle until no more immediate work is available.
    pub fn service_until_idle(&mut self) -> Result<usize, UsbHostError> {
        let mut work_done = 0;

        // SAFETY: Standard ESP-IDF USB host event handling calls.
        unsafe {
            // Handle library events
            let res = esp_idf_sys::usb_host_lib_handle_events(0);
            if res == esp_idf_sys::ESP_OK {
                work_done += 1;
            }

            // Handle client events
            let res = esp_idf_sys::usb_host_client_handle_events(self.client_hdl, 0);
            if res == esp_idf_sys::ESP_OK {
                work_done += 1;
            }
        }

        // Pull any events from the global queue populated by the callback
        unsafe {
            if let Some(ref mut queue) = GLOBAL_INGRESS_QUEUE {
                while let Some(event) = queue.pop_front() {
                    self.events.push_back(event);
                    work_done += 1;
                }
            }
        }

        Ok(work_done)
    }
}

#[cfg(target_os = "espidf")]
static mut GLOBAL_INGRESS_QUEUE: Option<std::collections::VecDeque<UsbEvent>> = None;

#[cfg(target_os = "espidf")]
unsafe extern "C" fn client_event_cb(
    event_msg: *const esp_idf_sys::usb_host_client_event_msg_t,
    _arg: *mut std::ffi::c_void,
) {
    let msg = *event_msg;
    match msg.event {
        esp_idf_sys::usb_host_client_event_t_USB_HOST_CLIENT_EVENT_NEW_DEV => {
            // We need to open the device to get VID/PID.
            // For this smoke test, we'll just signal attachment with 0s if we can't easily get VID/PID
            // without full device opening logic.
            if let Some(ref mut queue) = GLOBAL_INGRESS_QUEUE {
                queue.push_back(UsbEvent::DeviceAttached(DeviceMeta {
                    device_id: UsbDeviceId::new(msg.new_dev.address),
                    vendor_id: 0,
                    product_id: 0,
                }));
            }
        }
        esp_idf_sys::usb_host_client_event_t_USB_HOST_CLIENT_EVENT_DEV_GONE => {
            if let Some(ref mut queue) = GLOBAL_INGRESS_QUEUE {
                queue.push_back(UsbEvent::DeviceDetached(UsbDeviceId::new(
                    msg.dev_gone.address,
                )));
            }
        }
        _ => {}
    }
}

#[cfg(target_os = "espidf")]
impl UsbIngress for EspUsbHostIngress {
    fn poll_event(&mut self) -> Option<UsbEvent> {
        self.events.pop_front()
    }
}

#[cfg(not(target_os = "espidf"))]
impl EspUsbHostIngress {
    /// Returns not ready on host targets.
    pub fn new_single_client() -> Result<Self, UsbHostError> {
        Err(UsbHostError::NotReady)
    }

    /// Returns not ready on host targets.
    pub fn service_until_idle(&mut self) -> Result<usize, UsbHostError> {
        Err(UsbHostError::NotReady)
    }
}

#[cfg(not(target_os = "espidf"))]
impl UsbIngress for EspUsbHostIngress {
    fn poll_event(&mut self) -> Option<UsbEvent> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn esp_usb_host_ingress_new_single_client_returns_not_ready_on_host() {
        assert_eq!(
            EspUsbHostIngress::new_single_client().err(),
            Some(UsbHostError::NotReady)
        );
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn esp_usb_host_ingress_service_until_idle_returns_not_ready_on_host() {
        let mut ingress = EspUsbHostIngress;
        assert_eq!(
            ingress.service_until_idle().err(),
            Some(UsbHostError::NotReady)
        );
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn esp_usb_host_ingress_poll_event_returns_none_on_host() {
        let mut ingress = EspUsbHostIngress;
        assert_eq!(ingress.poll_event(), None);
    }

    #[test]
    fn usb_host_error_transport_compares_equal() {
        assert_eq!(UsbHostError::Transport, UsbHostError::Transport);
    }
}
