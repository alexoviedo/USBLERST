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
    /// Obtaining a descriptor failed.
    Descriptor,
    /// A USB transfer failed.
    Transfer,
    /// A transport-level failure occurred.
    Transport,
}

/// ESP-IDF-backed USB host ingress for embedded builds.
#[cfg(target_os = "espidf")]
pub struct EspUsbHostIngress {
    client_hdl: esp_idf_sys::usb_host_client_handle_t,
    events: std::collections::VecDeque<UsbEvent>,
    devices: std::collections::HashMap<u8, esp_idf_sys::usb_host_device_handle_t>,
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
                devices: std::collections::HashMap::new(),
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
                    match event {
                        UsbEvent::DeviceAttached(meta) => {
                            let address = meta.device_id.raw();
                            let mut dev_hdl: esp_idf_sys::usb_host_device_handle_t =
                                std::ptr::null_mut();

                            let res = esp_idf_sys::usb_host_device_open(
                                self.client_hdl,
                                address as u8,
                                &mut dev_hdl,
                            );

                            if res == esp_idf_sys::ESP_OK {
                                let mut dev_info = esp_idf_sys::usb_device_info_t::default();
                                let res = esp_idf_sys::usb_host_device_info(dev_hdl, &mut dev_info);

                                let enriched_meta = if res == esp_idf_sys::ESP_OK {
                                    DeviceMeta {
                                        device_id: meta.device_id,
                                        vendor_id: dev_info.vendor_id,
                                        product_id: dev_info.product_id,
                                    }
                                } else {
                                    meta
                                };

                                self.devices.insert(address, dev_hdl);
                                self.events
                                    .push_back(UsbEvent::DeviceAttached(enriched_meta));

                                // Attempt to fetch HID report descriptor
                                let _ = self.request_hid_descriptor(address, dev_hdl);
                            } else {
                                self.events.push_back(UsbEvent::DeviceAttached(meta));
                            }
                        }
                        UsbEvent::DeviceDetached(id) => {
                            let address = id.raw();
                            if let Some(dev_hdl) = self.devices.remove(&address) {
                                let _ =
                                    esp_idf_sys::usb_host_device_close(self.client_hdl, dev_hdl);
                            }
                            self.events.push_back(UsbEvent::DeviceDetached(id));
                        }
                        _ => {
                            self.events.push_back(event);
                        }
                    }
                    work_done += 1;
                }
            }
        }

        Ok(work_done)
    }

    unsafe fn request_hid_descriptor(
        &mut self,
        _address: u8,
        dev_hdl: esp_idf_sys::usb_host_device_handle_t,
    ) -> Result<(), UsbHostError> {
        let mut config_desc: *const esp_idf_sys::usb_config_desc_t = std::ptr::null();
        let res = esp_idf_sys::usb_host_get_active_config_descriptor(dev_hdl, &mut config_desc);

        if res != esp_idf_sys::ESP_OK {
            return Err(UsbHostError::Descriptor);
        }

        let mut offset = 0;
        let total_len = (*config_desc).wTotalLength as usize;

        // Simple iterator through descriptors to find the first HID interface
        while offset < total_len {
            let desc =
                (config_desc as *const u8).add(offset) as *const esp_idf_sys::usb_standard_desc_t;
            let b_length = (*desc).bLength as usize;
            let b_descriptor_type = (*desc).bDescriptorType;

            if b_descriptor_type == 0x04 {
                // Interface descriptor
                let iface_desc = desc as *const esp_idf_sys::usb_intf_desc_t;
                if (*iface_desc).bInterfaceClass == 0x03 {
                    // HID Interface found. Search for HID descriptor following it.
                    let mut hid_offset = offset + b_length;
                    while hid_offset < total_len {
                        let h_desc = (config_desc as *const u8).add(hid_offset)
                            as *const esp_idf_sys::usb_standard_desc_t;
                        if (*h_desc).bDescriptorType == 0x21 {
                            // HID Descriptor
                            // In ESP-IDF/USB, the report descriptor length is in the HID descriptor.
                            // The HID descriptor has variable length depending on bNumDescriptors.
                            // Byte 7 and 8 are the length of the first report descriptor.
                            let h_ptr = h_desc as *const u8;
                            let report_desc_len =
                                (*h_ptr.add(7) as u16) | ((*h_ptr.add(8) as u16) << 8);

                            return self
                                .submit_report_descriptor_transfer(dev_hdl, report_desc_len);
                        } else if (*h_desc).bDescriptorType == 0x04
                            || (*h_desc).bDescriptorType == 0x05
                        {
                            // Next interface or endpoint, stop searching for HID desc in this iface
                            break;
                        }
                        hid_offset += (*h_desc).bLength as usize;
                    }
                }
            }
            offset += b_length;
        }

        Ok(())
    }

    unsafe fn submit_report_descriptor_transfer(
        &mut self,
        dev_hdl: esp_idf_sys::usb_host_device_handle_t,
        report_desc_len: u16,
    ) -> Result<(), UsbHostError> {
        let mut transfer: *mut esp_idf_sys::usb_transfer_t = std::ptr::null_mut();
        let res = esp_idf_sys::usb_host_transfer_alloc(report_desc_len as usize, 0, &mut transfer);

        if res != esp_idf_sys::ESP_OK {
            return Err(UsbHostError::Transfer);
        }

        (*transfer).device_handle = dev_hdl;
        (*transfer).callback = Some(transfer_cb);
        (*transfer).context = std::ptr::null_mut(); // We could pass Self but need careful lifetime
        (*transfer).bEndpointAddress = 0x00; // Control pipe
        (*transfer).num_bytes = report_desc_len as i32;

        let setup_ptr = (*transfer).data_buffer as *mut esp_idf_sys::usb_setup_t;
        (*setup_ptr).bmRequestType = 0x81; // Device to Host, Standard, Interface
        (*setup_ptr).bRequest = 0x06; // GET_DESCRIPTOR
        (*setup_ptr).wValue = 0x2200; // Report Descriptor
        (*setup_ptr).wIndex = 0x0000; // Interface 0
        (*setup_ptr).wLength = report_desc_len;

        let res = esp_idf_sys::usb_host_transfer_submit(transfer);
        if res != esp_idf_sys::ESP_OK {
            let _ = esp_idf_sys::usb_host_transfer_free(transfer);
            return Err(UsbHostError::Transfer);
        }

        Ok(())
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
unsafe extern "C" fn transfer_cb(transfer: *mut esp_idf_sys::usb_transfer_t) {
    let mut data = [0_u8; 64];
    let actual_len = (*transfer).actual_num_bytes as usize;
    let copy_len = actual_len.min(64);

    // Skip the 8-byte setup packet if it's a control transfer result
    // (though in some ESP-IDF versions the buffer might start after setup)
    // Actually for GET_DESCRIPTOR report, the data buffer contains the descriptor.
    // In ESP-IDF usb_host, the data_buffer of a control transfer contains:
    // [setup (8 bytes)] [data (...)]
    if actual_len >= 8 {
        let data_ptr = (*transfer).data_buffer.add(8);
        let data_len = (actual_len - 8).min(64);
        std::ptr::copy_nonoverlapping(data_ptr, data.as_mut_ptr(), data_len);

        if let Some(ref mut queue) = GLOBAL_INGRESS_QUEUE {
            let mut address: u8 = 0;
            let _ = esp_idf_sys::usb_host_device_addr((*transfer).device_handle, &mut address);

            queue.push_back(UsbEvent::ReportDescriptorReceived {
                device_id: UsbDeviceId::new(address),
                bytes: data,
                len: data_len,
            });
        }
    }

    let _ = esp_idf_sys::usb_host_transfer_free(transfer);
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
