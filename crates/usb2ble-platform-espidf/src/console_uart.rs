#[cfg(target_os = "espidf")]
use esp_idf_svc::hal::gpio;
#[cfg(target_os = "espidf")]
use esp_idf_svc::hal::prelude::*;
#[cfg(target_os = "espidf")]
use esp_idf_svc::hal::uart;

/// Fixed-capacity RX buffer size for framed console bytes.
pub const RX_BUFFER_CAPACITY: usize = 128;

/// Fixed-capacity TX buffer size for framed console bytes.
pub const TX_BUFFER_CAPACITY: usize = 128;

/// Errors that can occur when using the console boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleError {
    /// The console transport is not ready.
    NotReady,
    /// A transport-level failure occurred.
    Transport,
}

/// Command ingress boundary for the future UART console glue.
pub trait CommandSource {
    /// Returns the next available command, if any.
    fn poll_command(&mut self) -> Option<usb2ble_proto::messages::Command>;
}

/// Response egress boundary for the future UART console glue.
pub trait ResponseSink {
    /// Sends a typed protocol response.
    fn send_response(
        &mut self,
        response: usb2ble_proto::messages::Response,
    ) -> Result<(), ConsoleError>;
}

/// Errors that can occur while buffering or framing console traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameBufferError {
    /// Appending bytes to RX would exceed the fixed RX buffer.
    RxOverflow {
        /// The total RX length that was attempted.
        attempted: usize,
        /// The maximum RX buffer capacity.
        max: usize,
    },
    /// Appending bytes to TX would exceed the fixed TX buffer.
    TxOverflow {
        /// The total TX length that was attempted.
        attempted: usize,
        /// The maximum TX buffer capacity.
        max: usize,
    },
    /// Decoding a newline-terminated command frame failed.
    Decode(usb2ble_proto::framing::FrameError),
    /// Encoding a typed response frame failed.
    Encode(usb2ble_proto::framing::FrameError),
}

/// Fixed-capacity RX/TX framing buffer for the future UART console bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramedConsoleBuffer {
    rx: [u8; RX_BUFFER_CAPACITY],
    rx_len: usize,
    tx: [u8; TX_BUFFER_CAPACITY],
    tx_len: usize,
}

impl Default for FramedConsoleBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// ESP-IDF-backed UART console transport for embedded builds.
#[cfg(target_os = "espidf")]
pub struct EspUartBufferedConsole {
    driver: uart::UartDriver<'static>,
}

/// Host stub for the ESP-IDF-backed UART console transport.
#[cfg(not(target_os = "espidf"))]
pub struct EspUartBufferedConsole;

#[cfg(target_os = "espidf")]
impl EspUartBufferedConsole {
    /// Creates a UART transport using default UART0 settings.
    pub fn new_default() -> Result<Self, ConsoleError> {
        let peripherals = Peripherals::take().map_err(|_| ConsoleError::Transport)?;
        let config = uart::config::Config::new().baudrate(115_200.into());

        // Use the default console UART path (UART0) with HAL defaults
        let driver = uart::UartDriver::new(
            peripherals.uart0,
            peripherals.pins.gpio43,
            peripherals.pins.gpio44,
            Option::<gpio::Gpio0>::None,
            Option::<gpio::Gpio0>::None,
            &config,
        )
        .map_err(|_| ConsoleError::Transport)?;

        Ok(Self { driver })
    }

    /// Pulls RX bytes from UART into the provided framed buffer.
    pub fn pull_rx_into(
        &mut self,
        buffer: &mut FramedConsoleBuffer,
    ) -> Result<usize, ConsoleError> {
        let mut temp_buf = [0_u8; 64];

        // Non-blocking read
        let len = self
            .driver
            .read(&mut temp_buf, 0)
            .map_err(|_| ConsoleError::Transport)?;

        if len > 0 {
            buffer
                .push_rx_bytes(&temp_buf[..len])
                .map_err(|_| ConsoleError::Transport)?;
        }

        Ok(len)
    }

    /// Flushes queued TX bytes from the framed buffer back to UART.
    pub fn flush_tx_from(
        &mut self,
        buffer: &mut FramedConsoleBuffer,
    ) -> Result<usize, ConsoleError> {
        let mut temp_buf = [0_u8; 64];
        let mut total_written = 0;

        loop {
            let drained = buffer.drain_tx_into(&mut temp_buf);
            if drained == 0 {
                break;
            }

            self.driver
                .write(&temp_buf[..drained])
                .map_err(|_| ConsoleError::Transport)?;

            total_written += drained;
        }

        Ok(total_written)
    }
}

#[cfg(not(target_os = "espidf"))]
impl EspUartBufferedConsole {
    /// Returns not ready on host targets.
    pub fn new_default() -> Result<Self, ConsoleError> {
        Err(ConsoleError::NotReady)
    }

    /// Returns not ready on host targets.
    pub fn pull_rx_into(
        &mut self,
        _buffer: &mut FramedConsoleBuffer,
    ) -> Result<usize, ConsoleError> {
        Err(ConsoleError::NotReady)
    }

    /// Returns not ready on host targets.
    pub fn flush_tx_from(
        &mut self,
        _buffer: &mut FramedConsoleBuffer,
    ) -> Result<usize, ConsoleError> {
        Err(ConsoleError::NotReady)
    }
}

impl FramedConsoleBuffer {
    /// Creates an empty framed console buffer with zeroed RX and TX storage.
    pub fn new() -> Self {
        Self {
            rx: [0_u8; RX_BUFFER_CAPACITY],
            rx_len: 0,
            tx: [0_u8; TX_BUFFER_CAPACITY],
            tx_len: 0,
        }
    }

    /// Returns the number of valid bytes currently buffered in RX.
    pub fn rx_len(&self) -> usize {
        self.rx_len
    }

    /// Returns the number of valid bytes currently buffered in TX.
    pub fn tx_len(&self) -> usize {
        self.tx_len
    }

    /// Returns the valid queued TX bytes without trailing zeroes.
    pub fn tx_bytes(&self) -> &[u8] {
        &self.tx[..self.tx_len]
    }

    /// Clears the RX buffer length without overwriting prior bytes.
    pub fn clear_rx(&mut self) {
        self.rx_len = 0;
    }

    /// Clears the TX buffer length without overwriting prior bytes.
    pub fn clear_tx(&mut self) {
        self.tx_len = 0;
    }

    /// Appends raw bytes into the RX buffer without partial writes.
    pub fn push_rx_bytes(&mut self, input: &[u8]) -> Result<(), FrameBufferError> {
        let attempted = self.rx_len + input.len();

        if attempted > RX_BUFFER_CAPACITY {
            return Err(FrameBufferError::RxOverflow {
                attempted,
                max: RX_BUFFER_CAPACITY,
            });
        }

        self.rx[self.rx_len..attempted].copy_from_slice(input);
        self.rx_len = attempted;

        Ok(())
    }

    /// Attempts to decode one newline-terminated command frame from RX.
    pub fn try_decode_command(
        &mut self,
    ) -> Result<Option<usb2ble_proto::messages::Command>, FrameBufferError> {
        let Some(frame_len) = self.rx[..self.rx_len]
            .iter()
            .position(|&byte| byte == b'\n')
            .map(|index| index + 1)
        else {
            return Ok(None);
        };

        let decoded = usb2ble_proto::framing::decode_command(&self.rx[..frame_len]);
        consume_prefix(&mut self.rx, &mut self.rx_len, frame_len);

        match decoded {
            Ok(command) => Ok(Some(command)),
            Err(error) => Err(FrameBufferError::Decode(error)),
        }
    }

    /// Encodes and appends one typed response frame into TX.
    pub fn queue_response(
        &mut self,
        response: usb2ble_proto::messages::Response,
    ) -> Result<(), FrameBufferError> {
        let encoded =
            usb2ble_proto::framing::encode_response(response).map_err(FrameBufferError::Encode)?;
        let attempted = self.tx_len + encoded.len();

        if attempted > TX_BUFFER_CAPACITY {
            return Err(FrameBufferError::TxOverflow {
                attempted,
                max: TX_BUFFER_CAPACITY,
            });
        }

        self.tx[self.tx_len..attempted].copy_from_slice(encoded.as_bytes());
        self.tx_len = attempted;

        Ok(())
    }

    /// Drains as many queued TX bytes as will fit into the provided output buffer.
    pub fn drain_tx_into(&mut self, out: &mut [u8]) -> usize {
        if self.tx_len == 0 || out.is_empty() {
            return 0;
        }

        let copy_len = self.tx_len.min(out.len());
        out[..copy_len].copy_from_slice(&self.tx[..copy_len]);
        consume_prefix(&mut self.tx, &mut self.tx_len, copy_len);

        copy_len
    }
}

fn consume_prefix<const N: usize>(buffer: &mut [u8; N], len: &mut usize, consumed: usize) {
    if consumed >= *len {
        *len = 0;
        return;
    }

    buffer.copy_within(consumed..*len, 0);
    *len -= consumed;
}

/// In-memory single-slot command source for host-side use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueuedCommandSource {
    next_command: Option<usb2ble_proto::messages::Command>,
    poll_calls: usize,
}

impl Default for QueuedCommandSource {
    fn default() -> Self {
        Self::new()
    }
}

impl QueuedCommandSource {
    /// Creates an empty queued command source.
    pub fn new() -> Self {
        Self {
            next_command: None,
            poll_calls: 0,
        }
    }

    /// Creates a queued command source with one pending command.
    pub fn with_command(command: usb2ble_proto::messages::Command) -> Self {
        Self {
            next_command: Some(command),
            poll_calls: 0,
        }
    }

    /// Queues or replaces the next command to be returned.
    pub fn queue_command(&mut self, command: usb2ble_proto::messages::Command) {
        self.next_command = Some(command);
    }

    /// Returns how many times the source has been polled.
    pub fn poll_calls(&self) -> usize {
        self.poll_calls
    }
}

impl CommandSource for QueuedCommandSource {
    fn poll_command(&mut self) -> Option<usb2ble_proto::messages::Command> {
        self.poll_calls += 1;
        self.next_command.take()
    }
}

/// In-memory response sink that records the last sent response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordingResponseSink {
    last_response: Option<usb2ble_proto::messages::Response>,
    send_calls: usize,
    fail_with: Option<ConsoleError>,
}

impl Default for RecordingResponseSink {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordingResponseSink {
    /// Creates an empty recording response sink.
    pub fn new() -> Self {
        Self {
            last_response: None,
            send_calls: 0,
            fail_with: None,
        }
    }

    /// Returns the most recently sent response, if any.
    pub fn last_response(&self) -> Option<usb2ble_proto::messages::Response> {
        self.last_response
    }

    /// Returns how many times a response send was attempted.
    pub fn send_calls(&self) -> usize {
        self.send_calls
    }

    /// Forces future sends to fail with the provided error.
    pub fn set_fail_with(&mut self, error: ConsoleError) {
        self.fail_with = Some(error);
    }

    /// Clears any forced send failure.
    pub fn clear_failure(&mut self) {
        self.fail_with = None;
    }

    /// Clears the last recorded response.
    pub fn clear_last_response(&mut self) {
        self.last_response = None;
    }
}

impl ResponseSink for RecordingResponseSink {
    fn send_response(
        &mut self,
        response: usb2ble_proto::messages::Response,
    ) -> Result<(), ConsoleError> {
        self.send_calls += 1;

        match self.fail_with {
            Some(error) => Err(error),
            None => {
                self.last_response = Some(response);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConsoleError, FrameBufferError, FramedConsoleBuffer, QueuedCommandSource,
        RecordingResponseSink, RX_BUFFER_CAPACITY, TX_BUFFER_CAPACITY,
    };
    use crate::console_uart::{CommandSource, ResponseSink};
    use usb2ble_core::profile::V1_PROFILE_ID;
    use usb2ble_proto::framing::FrameError;
    use usb2ble_proto::messages::{Command, Response};

    #[test]
    fn framed_console_buffer_new_starts_empty() {
        let buffer = FramedConsoleBuffer::new();

        assert_eq!(buffer.rx_len(), 0);
        assert_eq!(buffer.tx_len(), 0);
        assert!(buffer.tx_bytes().is_empty());
    }

    #[test]
    fn framed_console_buffer_pushes_partial_rx_and_decodes_get_info() {
        let mut buffer = FramedConsoleBuffer::new();

        assert_eq!(buffer.push_rx_bytes(b"GET_"), Ok(()));
        assert_eq!(buffer.push_rx_bytes(b"INFO\n"), Ok(()));
        assert_eq!(buffer.try_decode_command(), Ok(Some(Command::GetInfo)));
        assert_eq!(buffer.rx_len(), 0);
    }

    #[test]
    fn framed_console_buffer_returns_none_for_partial_frame_without_newline() {
        let mut buffer = FramedConsoleBuffer::new();

        assert_eq!(buffer.push_rx_bytes(b"GET_INFO"), Ok(()));
        assert_eq!(buffer.try_decode_command(), Ok(None));
        assert_eq!(buffer.rx_len(), 8);
    }

    #[test]
    fn framed_console_buffer_decodes_multiple_frames_in_order() {
        let mut buffer = FramedConsoleBuffer::new();

        assert_eq!(buffer.push_rx_bytes(b"GET_INFO\nGET_PROFILE\n"), Ok(()));
        assert_eq!(buffer.try_decode_command(), Ok(Some(Command::GetInfo)));
        assert_eq!(buffer.try_decode_command(), Ok(Some(Command::GetProfile)));
        assert_eq!(buffer.try_decode_command(), Ok(None));
        assert_eq!(buffer.rx_len(), 0);
    }

    #[test]
    fn framed_console_buffer_consumes_malformed_frames_before_next_decode() {
        let mut buffer = FramedConsoleBuffer::new();

        assert_eq!(buffer.push_rx_bytes(b"NOPE\nGET_INFO\n"), Ok(()));
        assert_eq!(
            buffer.try_decode_command(),
            Err(FrameBufferError::Decode(FrameError::UnknownCommand))
        );
        assert_eq!(buffer.try_decode_command(), Ok(Some(Command::GetInfo)));
        assert_eq!(buffer.rx_len(), 0);
    }

    #[test]
    fn framed_console_buffer_rejects_rx_overflow_without_mutating_contents() {
        let mut buffer = FramedConsoleBuffer::new();
        let overflow_input = [b'A'; RX_BUFFER_CAPACITY + 1];

        assert_eq!(
            buffer.push_rx_bytes(&overflow_input),
            Err(FrameBufferError::RxOverflow {
                attempted: RX_BUFFER_CAPACITY + 1,
                max: RX_BUFFER_CAPACITY,
            })
        );
        assert_eq!(buffer.rx_len(), 0);
        assert_eq!(buffer.try_decode_command(), Ok(None));
    }

    #[test]
    fn framed_console_buffer_queues_responses_back_to_back_in_tx() {
        let mut buffer = FramedConsoleBuffer::new();

        assert_eq!(buffer.queue_response(Response::Ack), Ok(()));
        assert_eq!(
            buffer.queue_response(Response::Profile {
                active_profile: V1_PROFILE_ID,
            }),
            Ok(())
        );
        assert_eq!(buffer.tx_bytes(), b"ACK\nPROFILE|t16000m_v1\n");
    }

    #[test]
    fn framed_console_buffer_supports_partial_tx_draining() {
        let mut buffer = FramedConsoleBuffer::new();
        let mut first_out = [0_u8; 2];
        let mut second_out = [0_u8; 8];

        assert_eq!(buffer.queue_response(Response::Ack), Ok(()));
        assert_eq!(buffer.drain_tx_into(&mut first_out), 2);
        assert_eq!(&first_out[..2], b"AC");
        assert_eq!(buffer.tx_bytes(), b"K\n");
        assert_eq!(buffer.drain_tx_into(&mut second_out), 2);
        assert_eq!(&second_out[..2], b"K\n");
        assert!(buffer.tx_bytes().is_empty());
        assert_eq!(buffer.tx_len(), 0);
    }

    #[test]
    fn framed_console_buffer_rejects_tx_overflow_without_losing_queued_bytes() {
        let mut buffer = FramedConsoleBuffer::new();
        let mut expected = [0_u8; TX_BUFFER_CAPACITY];

        for index in 0..(TX_BUFFER_CAPACITY / 4) {
            assert_eq!(buffer.queue_response(Response::Ack), Ok(()));
            expected[index * 4..(index + 1) * 4].copy_from_slice(b"ACK\n");
        }

        assert_eq!(
            buffer.queue_response(Response::Ack),
            Err(FrameBufferError::TxOverflow {
                attempted: TX_BUFFER_CAPACITY + 4,
                max: TX_BUFFER_CAPACITY,
            })
        );
        assert_eq!(buffer.tx_len(), TX_BUFFER_CAPACITY);
        assert_eq!(buffer.tx_bytes(), &expected[..]);
    }

    #[test]
    fn queued_command_source_new_starts_empty() {
        let mut source = QueuedCommandSource::new();

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
        let mut source = QueuedCommandSource::with_command(Command::GetInfo);

        source.queue_command(Command::GetProfile);

        assert_eq!(source.poll_command(), Some(Command::GetProfile));
    }

    #[test]
    fn queued_command_source_poll_calls_increment_on_every_poll() {
        let mut source = QueuedCommandSource::new();

        let _ = source.poll_command();
        let _ = source.poll_command();

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
        let mut sink = RecordingResponseSink::new();

        assert_eq!(sink.send_response(Response::Ack), Ok(()));
        sink.set_fail_with(ConsoleError::Transport);

        assert_eq!(
            sink.send_response(Response::Profile {
                active_profile: V1_PROFILE_ID,
            }),
            Err(ConsoleError::Transport)
        );
        assert_eq!(sink.send_calls(), 2);
        assert_eq!(sink.last_response(), Some(Response::Ack));
    }

    #[test]
    fn recording_response_sink_clear_failure_removes_forced_failure() {
        let mut sink = RecordingResponseSink::new();

        sink.set_fail_with(ConsoleError::Transport);
        sink.clear_failure();

        assert_eq!(sink.send_response(Response::Ack), Ok(()));
    }

    #[test]
    fn recording_response_sink_clear_last_response_resets_stored_response() {
        let mut sink = RecordingResponseSink::new();

        assert_eq!(sink.send_response(Response::Ack), Ok(()));
        sink.clear_last_response();

        assert_eq!(sink.last_response(), None);
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn esp_uart_buffered_console_new_default_returns_not_ready_on_host() {
        use super::EspUartBufferedConsole;
        assert_eq!(
            EspUartBufferedConsole::new_default().err(),
            Some(ConsoleError::NotReady)
        );
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn esp_uart_buffered_console_pull_rx_into_returns_not_ready_on_host() {
        use super::EspUartBufferedConsole;
        let mut console = EspUartBufferedConsole;
        let mut buffer = FramedConsoleBuffer::new();
        assert_eq!(
            console.pull_rx_into(&mut buffer).err(),
            Some(ConsoleError::NotReady)
        );
    }

    #[cfg(not(target_os = "espidf"))]
    #[test]
    fn esp_uart_buffered_console_flush_tx_from_returns_not_ready_on_host() {
        use super::EspUartBufferedConsole;
        let mut console = EspUartBufferedConsole;
        let mut buffer = FramedConsoleBuffer::new();
        assert_eq!(
            console.flush_tx_from(&mut buffer).err(),
            Some(ConsoleError::NotReady)
        );
    }
}
