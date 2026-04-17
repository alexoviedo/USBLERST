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
    /// Internal signal that an input transfer has stopped for a device.
    #[doc(hidden)]
    InputTransferStopped(UsbDeviceId),
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

/// Internal staging area for USB host events received in callbacks.
#[cfg(target_os = "espidf")]
struct IngressStaging {
    /// Raw events detected by host client or transfer callbacks.
    events: std::collections::VecDeque<UsbEvent>,
}

/// ESP-IDF-backed USB host ingress for embedded builds.
#[cfg(target_os = "espidf")]
pub struct EspUsbHostIngress {
    client_hdl: esp_idf_sys::usb_host_client_handle_t,
    /// Staging area shared with C callbacks via raw pointer.
    staging: Box<IngressStaging>,
    /// Enriched and final events ready for the application to consume.
    final_events: std::collections::VecDeque<UsbEvent>,
    /// Tracked device handles for descriptor access and closing.
    devices: std::collections::HashMap<u8, esp_idf_sys::usb_host_device_handle_t>,
    /// Tracked input transfers per device to allow cleanup.
    input_transfers: std::collections::HashMap<u8, *mut esp_idf_sys::usb_transfer_t>,
}

/// Host stub for the ESP-IDF-backed USB host ingress.
#[cfg(not(target_os = "espidf"))]
pub struct EspUsbHostIngress;

#[cfg(target_os = "espidf")]
impl EspUsbHostIngress {
    /// Initializes the USB host stack and registers a single client.
    pub fn new_single_client() -> Result<Self, UsbHostError> {
        let staging = Box::new(IngressStaging {
            events: std::collections::VecDeque::new(),
        });
        let staging_ptr = Box::into_raw(staging);

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
                let _ = Box::from_raw(staging_ptr);
                // If already installed, we might want to continue, but for smoke test we expect clean start.
                return Err(UsbHostError::Install);
            }

            let client_config = esp_idf_sys::usb_host_client_config_t {
                is_within_static_size: false,
                max_num_event_msg: 5,
                client_event_callback: Some(client_event_cb),
                callback_arg: staging_ptr as *mut _,
            };

            let mut client_hdl: esp_idf_sys::usb_host_client_handle_t = std::ptr::null_mut();
            let res = esp_idf_sys::usb_host_client_register(&client_config, &mut client_hdl);
            if res != esp_idf_sys::ESP_OK {
                let _ = Box::from_raw(staging_ptr);
                return Err(UsbHostError::ClientRegister);
            }

            Ok(Self {
                client_hdl,
                staging: Box::from_raw(staging_ptr),
                final_events: std::collections::VecDeque::new(),
                devices: std::collections::HashMap::new(),
                input_transfers: std::collections::HashMap::new(),
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
            if res != esp_idf_sys::ESP_OK && res != esp_idf_sys::ESP_ERR_TIMEOUT {
                return Err(UsbHostError::Transport);
            }
            if res == esp_idf_sys::ESP_OK {
                work_done += 1;
            }

            // Handle client events
            let res = esp_idf_sys::usb_host_client_handle_events(self.client_hdl, 0);
            if res != esp_idf_sys::ESP_OK && res != esp_idf_sys::ESP_ERR_TIMEOUT {
                return Err(UsbHostError::Transport);
            }
            if res == esp_idf_sys::ESP_OK {
                work_done += 1;
            }
        }

        // Pull raw events from the shared staging area populated by callbacks
        while let Some(event) = self.staging.events.pop_front() {
            match event {
                UsbEvent::DeviceAttached(meta) => {
                    let address = meta.device_id.raw();
                    let mut dev_hdl: esp_idf_sys::usb_host_device_handle_t = std::ptr::null_mut();

                    // SAFETY: standard ESP-IDF USB host device open
                    let res = unsafe {
                        esp_idf_sys::usb_host_device_open(self.client_hdl, address, &mut dev_hdl)
                    };

                    if res == esp_idf_sys::ESP_OK {
                        let mut dev_info = esp_idf_sys::usb_device_info_t::default();
                        // SAFETY: standard ESP-IDF USB host device info
                        let res =
                            unsafe { esp_idf_sys::usb_host_device_info(dev_hdl, &mut dev_info) };

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
                        self.final_events
                            .push_back(UsbEvent::DeviceAttached(enriched_meta));

                        // Attempt to fetch HID report descriptor
                        // SAFETY: searching descriptors and submitting transfers
                        unsafe {
                            let _ = self.request_hid_descriptor(dev_hdl);
                        }
                    } else {
                        // If open fails, we can still report the basic attach
                        self.final_events.push_back(UsbEvent::DeviceAttached(meta));
                    }
                }
                UsbEvent::DeviceDetached(id) => {
                    let address = id.raw();
                    if let Some(transfer) = self.input_transfers.remove(&address) {
                        // SAFETY: standard ESP-IDF USB host transfer cancel/free
                        unsafe {
                            let _ = esp_idf_sys::usb_host_transfer_cancel(transfer);
                            // NOTE: transfer_free is typically called in the callback after cancellation
                        }
                    }
                    if let Some(dev_hdl) = self.devices.remove(&address) {
                        // SAFETY: standard ESP-IDF USB host device close
                        unsafe {
                            let _ = esp_idf_sys::usb_host_device_close(self.client_hdl, dev_hdl);
                        }
                    }
                    self.final_events.push_back(UsbEvent::DeviceDetached(id));
                }
                UsbEvent::ReportDescriptorReceived { .. }
                | UsbEvent::InputReportReceived { .. } => {
                    // Staging events from transfer callbacks are promoted to final queue
                    self.final_events.push_back(event);
                }
                UsbEvent::InputTransferStopped(id) => {
                    self.input_transfers.remove(&id.raw());
                }
            }
            work_done += 1;
            if work_done >= 20 {
                break;
            }
        }

        Ok(work_done)
    }

    unsafe fn request_hid_descriptor(
        &mut self,
        dev_hdl: esp_idf_sys::usb_host_device_handle_t,
    ) -> Result<(), UsbHostError> {
        let mut config_desc: *const esp_idf_sys::usb_config_desc_t = std::ptr::null();
        let res = esp_idf_sys::usb_host_get_active_config_descriptor(dev_hdl, &mut config_desc);

        if res != esp_idf_sys::ESP_OK {
            return Err(UsbHostError::Descriptor);
        }

        let mut offset = 0;
        let total_len = (*config_desc).wTotalLength as usize;

        // Simple iterator through descriptors to find the first HID interface and its interrupt IN endpoint
        while offset < total_len {
            let desc =
                (config_desc as *const u8).add(offset) as *const esp_idf_sys::usb_standard_desc_t;
            let b_length = (*desc).bLength as usize;
            let b_descriptor_type = (*desc).bDescriptorType;

            if b_descriptor_type == 0x04 {
                // Interface descriptor
                let iface_desc = desc as *const esp_idf_sys::usb_intf_desc_t;
                if (*iface_desc).bInterfaceClass == 0x03 {
                    let interface_number = (*iface_desc).bInterfaceNumber;
                    let mut hid_desc_info = None;
                    let mut endpoint_info = None;

                    // Search for HID descriptor and Interrupt IN endpoint within this interface
                    let mut sub_offset = offset + b_length;
                    while sub_offset < total_len {
                        let sub_desc = (config_desc as *const u8).add(sub_offset)
                            as *const esp_idf_sys::usb_standard_desc_t;
                        if (*sub_desc).bDescriptorType == 0x21 {
                            // HID Descriptor
                            let h_ptr = sub_desc as *const u8;
                            let report_desc_len =
                                (*h_ptr.add(7) as u16) | ((*h_ptr.add(8) as u16) << 8);
                            hid_desc_info = Some(report_desc_len);
                        } else if (*sub_desc).bDescriptorType == 0x05 {
                            // Endpoint descriptor
                            let ep_desc = sub_desc as *const esp_idf_sys::usb_ep_desc_t;
                            if ((*ep_desc).bmAttributes & 0x03) == 0x03
                                && ((*ep_desc).bEndpointAddress & 0x80) != 0
                            {
                                // Interrupt IN endpoint
                                endpoint_info =
                                    Some(((*ep_desc).bEndpointAddress, (*ep_desc).wMaxPacketSize));
                            }
                        } else if (*sub_desc).bDescriptorType == 0x04 {
                            // Next interface, stop searching
                            break;
                        }
                        sub_offset += (*sub_desc).bLength as usize;
                    }

                    if let Some(report_desc_len) = hid_desc_info {
                        let _ = self.submit_report_descriptor_transfer(
                            dev_hdl,
                            interface_number,
                            report_desc_len,
                        );
                    }

                    if let Some((ep_addr, max_packet_size)) = endpoint_info {
                        let _ = self.submit_input_report_transfer(
                            dev_hdl,
                            interface_number,
                            ep_addr,
                            max_packet_size,
                        );
                    }

                    return Ok(());
                }
            }
            offset += b_length;
        }

        Ok(())
    }

    unsafe fn submit_input_report_transfer(
        &mut self,
        dev_hdl: esp_idf_sys::usb_host_device_handle_t,
        interface_number: u8,
        ep_addr: u8,
        max_packet_size: u16,
    ) -> Result<(), UsbHostError> {
        // Claim interface before starting input reports
        let res =
            esp_idf_sys::usb_host_interface_claim(self.client_hdl, dev_hdl, interface_number, 0);
        if res != esp_idf_sys::ESP_OK {
            // It's fine if already claimed or failed for some reason in smoke path
        }

        let mut transfer: *mut esp_idf_sys::usb_transfer_t = std::ptr::null_mut();
        let res = esp_idf_sys::usb_host_transfer_alloc(max_packet_size as usize, 0, &mut transfer);

        if res != esp_idf_sys::ESP_OK {
            return Err(UsbHostError::Transfer);
        }

        (*transfer).device_handle = dev_hdl;
        (*transfer).callback = Some(input_report_cb);
        (*transfer).context = &mut *self.staging as *mut IngressStaging as *mut _;
        (*transfer).bEndpointAddress = ep_addr;
        (*transfer).num_bytes = max_packet_size as i32;

        let res = esp_idf_sys::usb_host_transfer_submit(transfer);
        if res != esp_idf_sys::ESP_OK {
            let _ = esp_idf_sys::usb_host_transfer_free(transfer);
            return Err(UsbHostError::Transfer);
        }

        let mut address: u8 = 0;
        let _ = esp_idf_sys::usb_host_device_addr(dev_hdl, &mut address);
        self.input_transfers.insert(address, transfer);

        Ok(())
    }

    unsafe fn submit_report_descriptor_transfer(
        &mut self,
        dev_hdl: esp_idf_sys::usb_host_device_handle_t,
        interface_number: u8,
        report_desc_len: u16,
    ) -> Result<(), UsbHostError> {
        let mut transfer: *mut esp_idf_sys::usb_transfer_t = std::ptr::null_mut();
        // Allocate space for setup (8 bytes) + data
        let res =
            esp_idf_sys::usb_host_transfer_alloc(8 + report_desc_len as usize, 0, &mut transfer);

        if res != esp_idf_sys::ESP_OK {
            return Err(UsbHostError::Transfer);
        }

        (*transfer).device_handle = dev_hdl;
        (*transfer).callback = Some(transfer_cb);
        // Pass the staging state pointer as context
        (*transfer).context = &mut *self.staging as *mut IngressStaging as *mut _;
        (*transfer).bEndpointAddress = 0x00; // Control pipe
        (*transfer).num_bytes = 8 + report_desc_len as i32;

        let setup_ptr = (*transfer).data_buffer as *mut esp_idf_sys::usb_setup_t;
        (*setup_ptr).bmRequestType = 0x81; // Device to Host, Standard, Interface
        (*setup_ptr).bRequest = 0x06; // GET_DESCRIPTOR
        (*setup_ptr).wValue = 0x2200; // Report Descriptor (0x22 is HID Report)
        (*setup_ptr).wIndex = interface_number as u16;
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
unsafe extern "C" fn input_report_cb(transfer: *mut esp_idf_sys::usb_transfer_t) {
    let staging = &mut *((*transfer).context as *mut IngressStaging);
    let actual_len = (*transfer).actual_num_bytes as usize;

    let mut address: u8 = 0;
    let _ = esp_idf_sys::usb_host_device_addr((*transfer).device_handle, &mut address);
    let device_id = UsbDeviceId::new(address);

    if (*transfer).status == esp_idf_sys::usb_transfer_status_t_USB_TRANSFER_STATUS_COMPLETED {
        if actual_len > 0 {
            let (data, copy_len) = copy_input_payload(transfer);
            let (report_id, _) = derive_report_id_and_len(&data[..copy_len]);

            staging.events.push_back(UsbEvent::InputReportReceived {
                device_id,
                report_id,
                bytes: data,
                len: copy_len,
            });
        }

        // Resubmit the transfer to keep polling
        let res = esp_idf_sys::usb_host_transfer_submit(transfer);
        if res != esp_idf_sys::ESP_OK {
            staging
                .events
                .push_back(UsbEvent::InputTransferStopped(device_id));
            let _ = esp_idf_sys::usb_host_transfer_free(transfer);
        }
    } else {
        // Transfer stopped or failed (cancelled/timeout/error)
        staging
            .events
            .push_back(UsbEvent::InputTransferStopped(device_id));
        let _ = esp_idf_sys::usb_host_transfer_free(transfer);
    }
}

#[cfg(target_os = "espidf")]
unsafe extern "C" fn client_event_cb(
    event_msg: *const esp_idf_sys::usb_host_client_event_msg_t,
    arg: *mut std::ffi::c_void,
) {
    let staging = &mut *(arg as *mut IngressStaging);
    let msg = *event_msg;

    match msg.event {
        esp_idf_sys::usb_host_client_event_t_USB_HOST_CLIENT_EVENT_NEW_DEV => {
            staging
                .events
                .push_back(UsbEvent::DeviceAttached(DeviceMeta {
                    device_id: UsbDeviceId::new(msg.new_dev.address),
                    vendor_id: 0,
                    product_id: 0,
                }));
        }
        esp_idf_sys::usb_host_client_event_t_USB_HOST_CLIENT_EVENT_DEV_GONE => {
            staging
                .events
                .push_back(UsbEvent::DeviceDetached(UsbDeviceId::new(
                    msg.dev_gone.address,
                )));
        }
        _ => {}
    }
}

#[cfg(target_os = "espidf")]
unsafe extern "C" fn transfer_cb(transfer: *mut esp_idf_sys::usb_transfer_t) {
    let staging = &mut *((*transfer).context as *mut IngressStaging);

    let (data, len) = extract_report_descriptor(transfer);
    if len > 0 {
        let mut address: u8 = 0;
        let _ = esp_idf_sys::usb_host_device_addr((*transfer).device_handle, &mut address);

        staging
            .events
            .push_back(UsbEvent::ReportDescriptorReceived {
                device_id: UsbDeviceId::new(address),
                bytes: data,
                len,
            });
    }

    let _ = esp_idf_sys::usb_host_transfer_free(transfer);
}

/// Derives the HID report ID and length from a raw input report.
pub fn derive_report_id_and_len(bytes: &[u8]) -> (u8, usize) {
    if bytes.is_empty() {
        (0, 0)
    } else {
        // For now, use the first byte as the report ID in this smoke path.
        (bytes[0], bytes.len())
    }
}

#[cfg(target_os = "espidf")]
unsafe fn copy_input_payload(transfer: *mut esp_idf_sys::usb_transfer_t) -> ([u8; 64], usize) {
    let mut data = [0_u8; 64];
    let actual_len = (*transfer).actual_num_bytes as usize;
    let copy_len = actual_len.min(64);
    std::ptr::copy_nonoverlapping((*transfer).data_buffer, data.as_mut_ptr(), copy_len);
    (data, copy_len)
}

#[cfg(target_os = "espidf")]
unsafe fn extract_report_descriptor(
    transfer: *mut esp_idf_sys::usb_transfer_t,
) -> ([u8; 64], usize) {
    let mut data = [0_u8; 64];
    let actual_len = (*transfer).actual_num_bytes as usize;

    // In ESP-IDF usb_host, the data_buffer of a control transfer contains:
    // [setup (8 bytes)] [data (...)]
    if actual_len > 8 {
        let data_ptr = (*transfer).data_buffer.add(8);
        let data_len = (actual_len - 8).min(64);
        std::ptr::copy_nonoverlapping(data_ptr, data.as_mut_ptr(), data_len);
        (data, data_len)
    } else {
        (data, 0)
    }
}

#[cfg(target_os = "espidf")]
impl UsbIngress for EspUsbHostIngress {
    fn poll_event(&mut self) -> Option<UsbEvent> {
        self.final_events.pop_front()
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

    #[test]
    fn derive_report_id_and_len_returns_zero_for_empty_input() {
        assert_eq!(derive_report_id_and_len(&[]), (0, 0));
    }

    #[test]
    fn derive_report_id_and_len_extracts_first_byte_as_id() {
        let data = [0x01, 0x02, 0x03];
        assert_eq!(derive_report_id_and_len(&data), (0x01, 3));
    }
}
