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
