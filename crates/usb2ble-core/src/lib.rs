//! Core domain contracts for the USB-to-BLE bridge workspace.

/// HID descriptor intermediate representation for the lean v1 domain model.
pub mod hid_descriptor {
    /// The maximum number of descriptor fields supported by the fixed summary.
    pub const MAX_FIELDS: usize = 32;

    /// Identifies a HID short item category.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ItemType {
        /// Main item.
        Main,
        /// Global item.
        Global,
        /// Local item.
        Local,
        /// Reserved item type.
        Reserved,
    }

    /// One parsed HID short item token.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ShortItem {
        /// The decoded item type.
        pub item_type: ItemType,
        /// The decoded 4-bit tag value.
        pub tag: u8,
        /// The number of data bytes carried by this item.
        pub size_bytes: u8,
        /// The raw little-endian item data.
        pub data: u32,
    }

    impl ShortItem {
        /// Returns the raw unsigned item value.
        pub fn unsigned_value(self) -> u32 {
            self.data
        }

        /// Returns the signed item value for the stored data width.
        pub fn signed_value(self) -> i32 {
            sign_extend_item_data(self.data, self.size_bytes)
        }
    }

    /// Errors that can occur while tokenizing HID short items.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ItemParseError {
        /// HID long items are not supported in lean v1.
        LongItemsUnsupported,
        /// The input ended before a full short item could be read.
        Truncated {
            /// The total number of bytes needed starting at the requested offset.
            needed: usize,
            /// The number of bytes actually remaining starting at the offset.
            remaining: usize,
        },
    }

    /// Identifies a HID usage page in the descriptor summary.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum UsagePage {
        /// Generic Desktop Controls usage page.
        GenericDesktop,
        /// Button usage page.
        Button,
        /// An unrecognized usage page value.
        Unknown(u16),
    }

    /// Identifies a Generic Desktop usage in the descriptor summary.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum GenericDesktopUsage {
        /// X axis.
        X,
        /// Y axis.
        Y,
        /// Z axis.
        Z,
        /// Rx axis.
        Rx,
        /// Ry axis.
        Ry,
        /// Rz axis.
        Rz,
        /// Hat switch.
        HatSwitch,
        /// An unrecognized Generic Desktop usage value.
        Unknown(u16),
    }

    /// Identifies a concrete HID usage in the descriptor summary.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Usage {
        /// A Generic Desktop usage.
        GenericDesktop(GenericDesktopUsage),
        /// A one-based button usage identifier.
        Button(u16),
        /// An unknown usage with its page and identifier preserved.
        Unknown {
            /// The usage page associated with the unknown usage.
            page: UsagePage,
            /// The raw usage identifier value.
            id: u16,
        },
    }

    /// Identifies whether a field is variable or array data.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FieldKind {
        /// Variable field data.
        Variable,
        /// Array field data.
        Array,
    }

    /// A single decoded descriptor field summary entry.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ReportField {
        /// The report identifier for this field.
        pub report_id: u8,
        /// The bit offset of the field within the report payload.
        pub bit_offset: u16,
        /// The field width in bits.
        pub bit_size: u8,
        /// The logical minimum value.
        pub logical_min: i32,
        /// The logical maximum value.
        pub logical_max: i32,
        /// The semantic usage of the field.
        pub usage: Usage,
        /// Whether the field is variable or array data.
        pub kind: FieldKind,
    }

    /// A fixed-capacity summary of descriptor fields for lean v1.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ReportDescriptorSummary {
        /// The fixed-capacity field storage.
        pub fields: [Option<ReportField>; MAX_FIELDS],
        /// The number of populated field entries.
        pub field_count: usize,
    }

    impl Default for ReportDescriptorSummary {
        fn default() -> Self {
            Self {
                fields: [None; MAX_FIELDS],
                field_count: 0,
            }
        }
    }

    /// Errors that can occur when building a descriptor summary.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DescriptorError {
        /// Too many fields were pushed into the fixed-capacity summary.
        TooManyFields {
            /// The total number of fields that was attempted.
            attempted: usize,
            /// The maximum supported number of fields.
            max: usize,
        },
        /// A field was pushed with a bit offset earlier than the previous field.
        FieldOutOfOrder,
    }

    /// Errors that can occur while parsing a descriptor into the summary IR.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DescriptorParseError {
        /// A short-item tokenization error occurred.
        Item(ItemParseError),
        /// An input item required a current usage page that was not set.
        MissingUsagePage,
        /// An input item required a pending usage that was not set.
        MissingUsage,
        /// An input item required a report size that was not set.
        MissingReportSize,
        /// An input item required both logical minimum and maximum values.
        MissingLogicalRange,
        /// Lean v1 only supports descriptors with report count 1 per input item.
        UnsupportedReportCount {
            /// The unsupported report count value.
            count: u32,
        },
        /// The parsed report size did not fit into the supported bit-size type.
        ReportSizeOutOfRange {
            /// The raw report size in bits.
            size_bits: u32,
        },
        /// The parsed report id did not fit into `u8`.
        ReportIdOutOfRange {
            /// The raw report id value.
            report_id: u32,
        },
        /// Building the fixed-capacity summary failed.
        Summary(DescriptorError),
    }

    impl ReportDescriptorSummary {
        /// Returns the number of populated fields in the summary.
        pub fn field_count(&self) -> usize {
            self.field_count
        }

        /// Returns whether the summary contains no fields.
        pub fn is_empty(&self) -> bool {
            self.field_count == 0
        }

        /// Returns a copied field by index if it is present.
        pub fn field(&self, index: usize) -> Option<ReportField> {
            self.fields.get(index).copied().flatten()
        }

        /// Pushes one field into the fixed-capacity summary.
        pub fn push_field(&mut self, field: ReportField) -> Result<(), DescriptorError> {
            if self.field_count >= MAX_FIELDS {
                return Err(DescriptorError::TooManyFields {
                    attempted: self.field_count + 1,
                    max: MAX_FIELDS,
                });
            }

            if self.field_count > 0 {
                let previous = self.fields[self.field_count - 1];

                if let Some(previous) = previous {
                    if field.bit_offset < previous.bit_offset {
                        return Err(DescriptorError::FieldOutOfOrder);
                    }
                }
            }

            self.fields[self.field_count] = Some(field);
            self.field_count += 1;
            Ok(())
        }
    }

    fn sign_extend_item_data(data: u32, size_bytes: u8) -> i32 {
        match size_bytes {
            0 => 0,
            1 => i32::from(data as u8 as i8),
            2 => i32::from(data as u16 as i16),
            4 => data as i32,
            _ => data as i32,
        }
    }

    /// Maps a raw item type code into the typed HID short-item category.
    pub fn item_type_from_u8(raw: u8) -> ItemType {
        match raw {
            0 => ItemType::Main,
            1 => ItemType::Global,
            2 => ItemType::Local,
            3 => ItemType::Reserved,
            _ => ItemType::Reserved,
        }
    }

    /// Parses one HID short item token starting at the provided byte offset.
    pub fn parse_short_item(
        input: &[u8],
        offset: usize,
    ) -> Result<(ShortItem, usize), ItemParseError> {
        if offset >= input.len() {
            return Err(ItemParseError::Truncated {
                needed: 1,
                remaining: 0,
            });
        }

        let prefix = input[offset];

        if prefix == 0xFE {
            return Err(ItemParseError::LongItemsUnsupported);
        }

        let size_code = prefix & 0b11;
        let type_code = (prefix >> 2) & 0b11;
        let tag = (prefix >> 4) & 0b1111;
        let size_bytes = match size_code {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 4,
            _ => 0,
        };
        let needed = 1 + usize::from(size_bytes);
        let remaining = input.len() - offset;

        if remaining < needed {
            return Err(ItemParseError::Truncated { needed, remaining });
        }

        let mut data = 0_u32;

        for byte_index in 0..usize::from(size_bytes) {
            data |= u32::from(input[offset + 1 + byte_index]) << (byte_index * 8);
        }

        Ok((
            ShortItem {
                item_type: item_type_from_u8(type_code),
                tag,
                size_bytes,
                data,
            },
            offset + needed,
        ))
    }

    /// Parses a lean v1 subset of HID descriptor items into a summary IR.
    pub fn parse_descriptor_summary(
        input: &[u8],
    ) -> Result<ReportDescriptorSummary, DescriptorParseError> {
        let mut summary = ReportDescriptorSummary::default();
        let mut offset = 0_usize;
        let mut current_usage_page: Option<u16> = None;
        let mut pending_usage: Option<u16> = None;
        let mut current_logical_min: Option<i32> = None;
        let mut current_logical_max: Option<i32> = None;
        let mut current_report_size: Option<u8> = None;
        let mut current_report_count = 1_u32;
        let mut current_report_id = 0_u8;
        let mut current_bit_offset = 0_u16;

        while offset < input.len() {
            let (item, next_offset) =
                parse_short_item(input, offset).map_err(DescriptorParseError::Item)?;
            offset = next_offset;

            match item.item_type {
                ItemType::Global => match item.tag {
                    0 => {
                        current_usage_page = Some(item.unsigned_value() as u16);
                    }
                    1 => {
                        current_logical_min = Some(item.signed_value());
                    }
                    2 => {
                        current_logical_max = Some(item.signed_value());
                    }
                    7 => {
                        let raw_value = item.unsigned_value();
                        let report_size = u8::try_from(raw_value).map_err(|_| {
                            DescriptorParseError::ReportSizeOutOfRange {
                                size_bits: raw_value,
                            }
                        })?;
                        current_report_size = Some(report_size);
                    }
                    8 => {
                        let raw_value = item.unsigned_value();
                        let report_id = u8::try_from(raw_value).map_err(|_| {
                            DescriptorParseError::ReportIdOutOfRange {
                                report_id: raw_value,
                            }
                        })?;
                        current_report_id = report_id;
                    }
                    9 => {
                        current_report_count = item.unsigned_value();
                    }
                    _ => {}
                },
                ItemType::Local => {
                    if item.tag == 0 {
                        pending_usage = Some(item.unsigned_value() as u16);
                    }
                }
                ItemType::Main => match item.tag {
                    8 => {
                        let input_flags = item.unsigned_value();

                        if current_report_count != 1 {
                            return Err(DescriptorParseError::UnsupportedReportCount {
                                count: current_report_count,
                            });
                        }

                        let usage_page =
                            current_usage_page.ok_or(DescriptorParseError::MissingUsagePage)?;
                        let usage = pending_usage.ok_or(DescriptorParseError::MissingUsage)?;
                        let report_size =
                            current_report_size.ok_or(DescriptorParseError::MissingReportSize)?;
                        let (logical_min, logical_max) =
                            match (current_logical_min, current_logical_max) {
                                (Some(min), Some(max)) => (min, max),
                                _ => {
                                    return Err(DescriptorParseError::MissingLogicalRange);
                                }
                            };
                        let is_constant = input_flags & 0b1 != 0;

                        if is_constant {
                            current_bit_offset =
                                current_bit_offset.wrapping_add(u16::from(report_size));
                            pending_usage = None;
                            continue;
                        }

                        let kind = if input_flags & 0b10 != 0 {
                            FieldKind::Variable
                        } else {
                            FieldKind::Array
                        };
                        let field = ReportField {
                            report_id: current_report_id,
                            bit_offset: current_bit_offset,
                            bit_size: report_size,
                            logical_min,
                            logical_max,
                            usage: usage_from_parts(usage_page, usage),
                            kind,
                        };

                        summary
                            .push_field(field)
                            .map_err(DescriptorParseError::Summary)?;
                        current_bit_offset =
                            current_bit_offset.wrapping_add(u16::from(report_size));
                        pending_usage = None;
                    }
                    10 | 12 => {}
                    _ => {}
                },
                ItemType::Reserved => {}
            }
        }

        Ok(summary)
    }

    /// Maps a raw usage page identifier into the typed usage page model.
    pub fn usage_page_from_u16(raw: u16) -> UsagePage {
        match raw {
            0x0001 => UsagePage::GenericDesktop,
            0x0009 => UsagePage::Button,
            _ => UsagePage::Unknown(raw),
        }
    }

    /// Maps a raw Generic Desktop usage identifier into the typed usage model.
    pub fn generic_desktop_usage_from_u16(raw: u16) -> GenericDesktopUsage {
        match raw {
            0x0030 => GenericDesktopUsage::X,
            0x0031 => GenericDesktopUsage::Y,
            0x0032 => GenericDesktopUsage::Z,
            0x0033 => GenericDesktopUsage::Rx,
            0x0034 => GenericDesktopUsage::Ry,
            0x0035 => GenericDesktopUsage::Rz,
            0x0039 => GenericDesktopUsage::HatSwitch,
            _ => GenericDesktopUsage::Unknown(raw),
        }
    }

    /// Maps raw page and usage identifiers into the typed usage model.
    pub fn usage_from_parts(page: u16, id: u16) -> Usage {
        match usage_page_from_u16(page) {
            UsagePage::GenericDesktop => Usage::GenericDesktop(generic_desktop_usage_from_u16(id)),
            UsagePage::Button => Usage::Button(id),
            page => Usage::Unknown { page, id },
        }
    }
}

/// HID report decoding against the lean v1 descriptor summary IR.
pub mod hid_decode {
    /// The maximum number of decoded fields supported by the fixed report.
    pub const MAX_DECODED_FIELDS: usize = crate::hid_descriptor::MAX_FIELDS;

    /// One decoded field value paired with its semantic usage.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DecodedFieldValue {
        /// The semantic usage of the decoded field.
        pub usage: crate::hid_descriptor::Usage,
        /// The decoded integer value.
        pub value: i32,
    }

    /// A fixed-capacity decoded report.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DecodedReport {
        /// The fixed-capacity storage for decoded field values.
        pub fields: [Option<DecodedFieldValue>; MAX_DECODED_FIELDS],
        /// The number of populated decoded fields.
        pub field_count: usize,
    }

    impl Default for DecodedReport {
        fn default() -> Self {
            Self {
                fields: [None; MAX_DECODED_FIELDS],
                field_count: 0,
            }
        }
    }

    /// Errors that can occur while decoding a report payload.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DecodeError {
        /// The payload does not contain enough bits for the requested field.
        ReportTooShort {
            /// The number of bits required by the field.
            required_bits: usize,
            /// The number of bits actually available in the payload.
            actual_bits: usize,
        },
        /// The field uses a bit width that lean v1 does not decode yet.
        UnsupportedFieldSize {
            /// The unsupported field width in bits.
            bit_size: u8,
        },
        /// Too many fields were decoded into the fixed-capacity report.
        TooManyDecodedFields {
            /// The total number of decoded fields that was attempted.
            attempted: usize,
            /// The maximum supported number of decoded fields.
            max: usize,
        },
        /// Array-style fields are not decoded yet in lean v1.
        ArrayFieldsUnsupported,
    }

    impl DecodedReport {
        /// Returns the number of decoded fields in the report.
        pub fn field_count(&self) -> usize {
            self.field_count
        }

        /// Returns whether the decoded report contains no fields.
        pub fn is_empty(&self) -> bool {
            self.field_count == 0
        }

        /// Returns a copied decoded field by index if it is present.
        pub fn field(&self, index: usize) -> Option<DecodedFieldValue> {
            self.fields.get(index).copied().flatten()
        }
    }

    fn extract_bits(bit_offset: usize, bit_size: u8, payload: &[u8]) -> u32 {
        let mut raw = 0_u32;

        for bit_index in 0..usize::from(bit_size) {
            let absolute_bit = bit_offset + bit_index;
            let byte_index = absolute_bit / 8;
            let bit_in_byte = absolute_bit % 8;
            let bit = (payload[byte_index] >> bit_in_byte) & 1;

            raw |= u32::from(bit) << bit_index;
        }

        raw
    }

    /// Decodes all matching fields from a payload for one report identifier.
    pub fn decode_report(
        summary: &crate::hid_descriptor::ReportDescriptorSummary,
        report_id: u8,
        payload: &[u8],
    ) -> Result<DecodedReport, DecodeError> {
        let mut decoded = DecodedReport::default();

        for index in 0..summary.field_count() {
            let Some(field) = summary.field(index) else {
                continue;
            };

            if field.report_id != report_id {
                continue;
            }

            if field.kind == crate::hid_descriptor::FieldKind::Array {
                return Err(DecodeError::ArrayFieldsUnsupported);
            }

            if decoded.field_count >= MAX_DECODED_FIELDS {
                return Err(DecodeError::TooManyDecodedFields {
                    attempted: decoded.field_count + 1,
                    max: MAX_DECODED_FIELDS,
                });
            }

            let value = decode_field_value(field, payload)?;
            decoded.fields[decoded.field_count] = Some(DecodedFieldValue {
                usage: field.usage,
                value,
            });
            decoded.field_count += 1;
        }

        Ok(decoded)
    }

    /// Decodes one field value from the raw payload bits.
    pub fn decode_field_value(
        field: crate::hid_descriptor::ReportField,
        payload: &[u8],
    ) -> Result<i32, DecodeError> {
        match field.bit_size {
            1 | 8 | 16 => {}
            bit_size => {
                return Err(DecodeError::UnsupportedFieldSize { bit_size });
            }
        }

        let required_bits = usize::from(field.bit_offset) + usize::from(field.bit_size);
        let actual_bits = payload.len() * 8;

        if required_bits > actual_bits {
            return Err(DecodeError::ReportTooShort {
                required_bits,
                actual_bits,
            });
        }

        let raw = extract_bits(usize::from(field.bit_offset), field.bit_size, payload);

        if field.logical_min < 0 {
            let sign_bit = 1_u32 << (u32::from(field.bit_size) - 1);

            if raw & sign_bit == 0 {
                Ok(raw as i32)
            } else {
                Ok((raw as i32) - (1_i32 << u32::from(field.bit_size)))
            }
        } else {
            Ok(raw as i32)
        }
    }
}

/// Normalized joystick state contracts shared across the workspace.
pub mod normalize {
    /// The fixed number of buttons supported by the lean v1 normalized state.
    pub const BUTTON_COUNT: usize = 16;

    /// The minimum normalized axis value.
    pub const AXIS_MIN: i16 = i16::MIN;

    /// The maximum normalized axis value.
    pub const AXIS_MAX: i16 = i16::MAX;

    /// Identifies one of the normalized joystick axes.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Axis {
        /// The primary X axis.
        X,
        /// The primary Y axis.
        Y,
        /// The twist axis carried as `Rz`.
        Rz,
    }

    /// Represents the normalized eight-way hat plus centered state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum HatPosition {
        /// No hat direction is pressed.
        Centered,
        /// Up.
        Up,
        /// Up and right.
        UpRight,
        /// Right.
        Right,
        /// Down and right.
        DownRight,
        /// Down.
        Down,
        /// Down and left.
        DownLeft,
        /// Left.
        Left,
        /// Up and left.
        UpLeft,
    }

    /// A zero-based button index within the normalized v1 button range.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ButtonIndex(u8);

    impl ButtonIndex {
        /// Creates a validated zero-based button index.
        pub fn new(index: u8) -> Result<Self, ButtonIndexError> {
            if usize::from(index) < BUTTON_COUNT {
                Ok(Self(index))
            } else {
                Err(ButtonIndexError::OutOfRange {
                    index,
                    max_exclusive: BUTTON_COUNT,
                })
            }
        }

        /// Returns the zero-based button index.
        pub fn get(self) -> u8 {
            self.0
        }
    }

    /// Errors that can occur when constructing a [`ButtonIndex`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ButtonIndexError {
        /// The provided zero-based index is outside the supported range.
        OutOfRange {
            /// The invalid zero-based index supplied by the caller.
            index: u8,
            /// The exclusive upper bound for valid indices.
            max_exclusive: usize,
        },
    }

    /// Compact normalized joystick state for the fixed lean v1 contract.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NormalizedJoystickState {
        x: i16,
        y: i16,
        rz: i16,
        hat: HatPosition,
        buttons: u16,
    }

    impl Default for NormalizedJoystickState {
        fn default() -> Self {
            Self {
                x: 0,
                y: 0,
                rz: 0,
                hat: HatPosition::Centered,
                buttons: 0,
            }
        }
    }

    impl NormalizedJoystickState {
        fn button_mask(index: ButtonIndex) -> u16 {
            1_u16 << u32::from(index.get())
        }

        /// Returns the current value for the requested axis.
        pub fn axis(&self, axis: Axis) -> i16 {
            match axis {
                Axis::X => self.x,
                Axis::Y => self.y,
                Axis::Rz => self.rz,
            }
        }

        /// Sets the requested axis to the provided normalized value.
        pub fn set_axis(&mut self, axis: Axis, value: i16) {
            match axis {
                Axis::X => self.x = value,
                Axis::Y => self.y = value,
                Axis::Rz => self.rz = value,
            }
        }

        /// Returns the current hat position.
        pub fn hat(&self) -> HatPosition {
            self.hat
        }

        /// Sets the current hat position.
        pub fn set_hat(&mut self, hat: HatPosition) {
            self.hat = hat;
        }

        /// Returns whether the requested button is currently pressed.
        pub fn button(&self, index: ButtonIndex) -> bool {
            self.buttons & Self::button_mask(index) != 0
        }

        /// Sets or clears the requested button bit.
        pub fn set_button(&mut self, index: ButtonIndex, pressed: bool) {
            let mask = Self::button_mask(index);

            if pressed {
                self.buttons |= mask;
            } else {
                self.buttons &= !mask;
            }
        }

        /// Resets the full state back to the centered default.
        pub fn clear(&mut self) {
            *self = Self::default();
        }
    }

    /// Errors that can occur while normalizing decoded HID fields.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NormalizeError {
        /// A decoded axis value does not fit in the normalized axis range.
        AxisOutOfRange {
            /// The axis usage that failed normalization.
            usage: crate::hid_descriptor::GenericDesktopUsage,
            /// The decoded value that was out of range.
            value: i32,
        },
        /// A decoded hat value is not supported by the lean v1 mapping.
        UnsupportedHatValue {
            /// The unsupported decoded hat value.
            value: i32,
        },
        /// A decoded button usage falls outside the supported v1 button range.
        ButtonOutOfRange {
            /// The one-based HID button usage that was out of range.
            usage: u16,
        },
    }

    /// Applies one decoded HID field into the accumulating normalized state.
    pub fn apply_decoded_field(
        state: &mut NormalizedJoystickState,
        field: crate::hid_decode::DecodedFieldValue,
    ) -> Result<(), NormalizeError> {
        match field.usage {
            crate::hid_descriptor::Usage::GenericDesktop(
                crate::hid_descriptor::GenericDesktopUsage::X,
            ) => match i16::try_from(field.value) {
                Ok(value) => {
                    state.set_axis(Axis::X, value);
                    Ok(())
                }
                Err(_) => Err(NormalizeError::AxisOutOfRange {
                    usage: crate::hid_descriptor::GenericDesktopUsage::X,
                    value: field.value,
                }),
            },
            crate::hid_descriptor::Usage::GenericDesktop(
                crate::hid_descriptor::GenericDesktopUsage::Y,
            ) => match i16::try_from(field.value) {
                Ok(value) => {
                    state.set_axis(Axis::Y, value);
                    Ok(())
                }
                Err(_) => Err(NormalizeError::AxisOutOfRange {
                    usage: crate::hid_descriptor::GenericDesktopUsage::Y,
                    value: field.value,
                }),
            },
            crate::hid_descriptor::Usage::GenericDesktop(
                crate::hid_descriptor::GenericDesktopUsage::Rz,
            ) => match i16::try_from(field.value) {
                Ok(value) => {
                    state.set_axis(Axis::Rz, value);
                    Ok(())
                }
                Err(_) => Err(NormalizeError::AxisOutOfRange {
                    usage: crate::hid_descriptor::GenericDesktopUsage::Rz,
                    value: field.value,
                }),
            },
            crate::hid_descriptor::Usage::GenericDesktop(
                crate::hid_descriptor::GenericDesktopUsage::HatSwitch,
            ) => {
                let hat = match field.value {
                    -1 | 8 => HatPosition::Centered,
                    0 => HatPosition::Up,
                    1 => HatPosition::UpRight,
                    2 => HatPosition::Right,
                    3 => HatPosition::DownRight,
                    4 => HatPosition::Down,
                    5 => HatPosition::DownLeft,
                    6 => HatPosition::Left,
                    7 => HatPosition::UpLeft,
                    value => {
                        return Err(NormalizeError::UnsupportedHatValue { value });
                    }
                };

                state.set_hat(hat);
                Ok(())
            }
            crate::hid_descriptor::Usage::Button(id) => {
                if !(1..=16).contains(&id) {
                    return Err(NormalizeError::ButtonOutOfRange { usage: id });
                }

                let index = ButtonIndex::new((id - 1) as u8)
                    .map_err(|_| NormalizeError::ButtonOutOfRange { usage: id })?;
                state.set_button(index, field.value != 0);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Normalizes a decoded report into the canonical lean v1 joystick state.
    pub fn normalize_decoded_report(
        report: &crate::hid_decode::DecodedReport,
    ) -> Result<NormalizedJoystickState, NormalizeError> {
        let mut state = NormalizedJoystickState::default();

        for index in 0..report.field_count() {
            if let Some(field) = report.field(index) {
                apply_decoded_field(&mut state, field)?;
            }
        }

        Ok(state)
    }
}

/// Runtime-domain contracts for lean v1 state and report rendering.
pub mod runtime {
    fn button_bits_from_normalized(state: crate::normalize::NormalizedJoystickState) -> u16 {
        let mut buttons = 0_u16;

        for raw_index in 0..crate::normalize::BUTTON_COUNT {
            match crate::normalize::ButtonIndex::new(raw_index as u8) {
                Ok(index) if state.button(index) => {
                    buttons |= 1_u16 << raw_index;
                }
                Ok(_) | Err(_) => {}
            }
        }

        buttons
    }

    /// The fixed lean v1 BLE gamepad report with 16 buttons.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct GenericBleGamepad16Report {
        /// The current X axis value.
        pub x: i16,
        /// The current Y axis value.
        pub y: i16,
        /// The current Rz axis value.
        pub rz: i16,
        /// The current hat position.
        pub hat: crate::normalize::HatPosition,
        /// The current 16-button bitfield.
        pub buttons: u16,
    }

    impl Default for GenericBleGamepad16Report {
        fn default() -> Self {
            Self {
                x: 0,
                y: 0,
                rz: 0,
                hat: crate::normalize::HatPosition::Centered,
                buttons: 0,
            }
        }
    }

    impl GenericBleGamepad16Report {
        /// Renders a fixed lean v1 report directly from normalized input.
        pub fn from_normalized(state: crate::normalize::NormalizedJoystickState) -> Self {
            Self {
                x: state.axis(crate::normalize::Axis::X),
                y: state.axis(crate::normalize::Axis::Y),
                rz: state.axis(crate::normalize::Axis::Rz),
                hat: state.hat(),
                buttons: button_bits_from_normalized(state),
            }
        }
    }

    /// The in-memory runtime state for lean v1.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RuntimeState {
        active_profile: crate::profile::ProfileId,
        last_input: crate::normalize::NormalizedJoystickState,
    }

    impl Default for RuntimeState {
        fn default() -> Self {
            Self {
                active_profile: crate::profile::V1_PROFILE_ID,
                last_input: crate::normalize::NormalizedJoystickState::default(),
            }
        }
    }

    impl RuntimeState {
        /// Creates a runtime state with the requested active profile.
        pub fn new(active_profile: crate::profile::ProfileId) -> Self {
            Self {
                active_profile,
                last_input: crate::normalize::NormalizedJoystickState::default(),
            }
        }

        /// Returns the active profile.
        pub fn active_profile(&self) -> crate::profile::ProfileId {
            self.active_profile
        }

        /// Updates the active profile.
        pub fn set_active_profile(&mut self, profile: crate::profile::ProfileId) {
            self.active_profile = profile;
        }

        /// Returns the last normalized input snapshot.
        pub fn last_input(&self) -> crate::normalize::NormalizedJoystickState {
            self.last_input
        }

        /// Replaces the last normalized input snapshot.
        pub fn update_input(&mut self, state: crate::normalize::NormalizedJoystickState) {
            self.last_input = state;
        }

        /// Clears the stored normalized input back to the default state.
        pub fn clear_input(&mut self) {
            self.last_input = crate::normalize::NormalizedJoystickState::default();
        }

        /// Renders the current fixed lean v1 BLE gamepad report.
        pub fn current_report(&self) -> GenericBleGamepad16Report {
            GenericBleGamepad16Report::from_normalized(self.last_input)
        }
    }
}

/// Fixed v1 profile and output persona contracts.
pub mod profile {
    /// Identifies a normalized input profile.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProfileId {
        /// The fixed lean v1 T.16000M profile.
        T16000mV1,
    }

    /// Identifies the downstream output persona for a profile.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum OutputPersona {
        /// A fixed generic BLE HID gamepad persona with 16 buttons.
        GenericBleGamepad16,
    }

    impl OutputPersona {
        /// Returns the stable string identifier for this persona.
        pub fn as_str(self) -> &'static str {
            match self {
                Self::GenericBleGamepad16 => "generic_ble_gamepad_16",
            }
        }
    }

    /// The fixed lean v1 profile identifier.
    pub const V1_PROFILE_ID: ProfileId = ProfileId::T16000mV1;

    /// The stable string name for the fixed lean v1 profile.
    pub const V1_PROFILE_NAME: &str = "t16000m_v1";

    impl ProfileId {
        /// Returns the stable string identifier for this profile.
        pub fn as_str(self) -> &'static str {
            match self {
                Self::T16000mV1 => V1_PROFILE_NAME,
            }
        }

        /// Returns the fixed output persona targeted by this profile.
        pub fn output_persona(self) -> OutputPersona {
            match self {
                Self::T16000mV1 => OutputPersona::GenericBleGamepad16,
            }
        }
    }
}

/// Stub for device-specific quirks.
pub mod quirks {}

/// Crate identity used by bootstrap verification.
pub const CORE_CRATE_NAME: &str = "usb2ble-core";

#[cfg(test)]
mod tests {
    use super::hid_decode::{
        decode_field_value, decode_report, DecodeError, DecodedFieldValue, DecodedReport,
        MAX_DECODED_FIELDS,
    };
    use super::hid_descriptor::{
        generic_desktop_usage_from_u16, item_type_from_u8, parse_descriptor_summary,
        parse_short_item, usage_from_parts, usage_page_from_u16, DescriptorError,
        DescriptorParseError, FieldKind, GenericDesktopUsage, ItemParseError, ItemType,
        ReportDescriptorSummary, ReportField, ShortItem, Usage, UsagePage, MAX_FIELDS,
    };
    use super::normalize::{
        apply_decoded_field, normalize_decoded_report, Axis, ButtonIndex, ButtonIndexError,
        HatPosition, NormalizeError, NormalizedJoystickState,
    };
    use super::profile::{OutputPersona, V1_PROFILE_ID};
    use super::runtime::{GenericBleGamepad16Report, RuntimeState};
    use super::CORE_CRATE_NAME;

    fn sample_report_field(bit_offset: u16) -> ReportField {
        ReportField {
            report_id: 1,
            bit_offset,
            bit_size: 8,
            logical_min: -127,
            logical_max: 127,
            usage: Usage::GenericDesktop(GenericDesktopUsage::X),
            kind: FieldKind::Variable,
        }
    }

    fn button_index(index: u8) -> ButtonIndex {
        match ButtonIndex::new(index) {
            Ok(index) => index,
            Err(error) => panic!("failed to create button index {index}: {error:?}"),
        }
    }

    #[test]
    fn core_crate_name_matches_expected() {
        assert_eq!(CORE_CRATE_NAME, "usb2ble-core");
    }

    #[test]
    fn item_type_from_u8_maps_main() {
        assert_eq!(item_type_from_u8(0), ItemType::Main);
    }

    #[test]
    fn item_type_from_u8_maps_global() {
        assert_eq!(item_type_from_u8(1), ItemType::Global);
    }

    #[test]
    fn item_type_from_u8_maps_local() {
        assert_eq!(item_type_from_u8(2), ItemType::Local);
    }

    #[test]
    fn item_type_from_u8_maps_reserved() {
        assert_eq!(item_type_from_u8(3), ItemType::Reserved);
    }

    #[test]
    fn parse_short_item_parses_usage_page() {
        let (item, next_offset) = match parse_short_item(&[0x05, 0x01], 0) {
            Ok(parsed) => parsed,
            Err(error) => panic!("parse_short_item should succeed: {error:?}"),
        };

        assert_eq!(
            item,
            ShortItem {
                item_type: ItemType::Global,
                tag: 0,
                size_bytes: 1,
                data: 1,
            }
        );
        assert_eq!(item.unsigned_value(), 1);
        assert_eq!(next_offset, 2);
    }

    #[test]
    fn parse_short_item_parses_usage() {
        let (item, _) = match parse_short_item(&[0x09, 0x30], 0) {
            Ok(parsed) => parsed,
            Err(error) => panic!("parse_short_item should succeed: {error:?}"),
        };

        assert_eq!(item.item_type, ItemType::Local);
        assert_eq!(item.tag, 0);
        assert_eq!(item.size_bytes, 1);
        assert_eq!(item.unsigned_value(), 0x30);
    }

    #[test]
    fn parse_short_item_parses_report_size() {
        let (item, _) = match parse_short_item(&[0x75, 0x08], 0) {
            Ok(parsed) => parsed,
            Err(error) => panic!("parse_short_item should succeed: {error:?}"),
        };

        assert_eq!(item.item_type, ItemType::Global);
        assert_eq!(item.tag, 7);
        assert_eq!(item.size_bytes, 1);
        assert_eq!(item.unsigned_value(), 8);
    }

    #[test]
    fn parse_short_item_parses_input_item() {
        let (item, _) = match parse_short_item(&[0x81, 0x02], 0) {
            Ok(parsed) => parsed,
            Err(error) => panic!("parse_short_item should succeed: {error:?}"),
        };

        assert_eq!(item.item_type, ItemType::Main);
        assert_eq!(item.tag, 8);
        assert_eq!(item.size_bytes, 1);
        assert_eq!(item.unsigned_value(), 2);
    }

    #[test]
    fn parse_short_item_parses_report_id() {
        let (item, _) = match parse_short_item(&[0x85, 0x01], 0) {
            Ok(parsed) => parsed,
            Err(error) => panic!("parse_short_item should succeed: {error:?}"),
        };

        assert_eq!(item.item_type, ItemType::Global);
        assert_eq!(item.tag, 8);
        assert_eq!(item.size_bytes, 1);
        assert_eq!(item.unsigned_value(), 1);
    }

    #[test]
    fn parse_short_item_parses_end_collection() {
        let (item, next_offset) = match parse_short_item(&[0xC0], 0) {
            Ok(parsed) => parsed,
            Err(error) => panic!("parse_short_item should succeed: {error:?}"),
        };

        assert_eq!(item.item_type, ItemType::Main);
        assert_eq!(item.tag, 12);
        assert_eq!(item.size_bytes, 0);
        assert_eq!(item.unsigned_value(), 0);
        assert_eq!(item.signed_value(), 0);
        assert_eq!(next_offset, 1);
    }

    #[test]
    fn short_item_signed_value_for_one_byte_negative_one() {
        let item = ShortItem {
            item_type: ItemType::Global,
            tag: 1,
            size_bytes: 1,
            data: 0xFF,
        };

        assert_eq!(item.signed_value(), -1);
    }

    #[test]
    fn short_item_signed_value_for_two_byte_negative_value() {
        let item = ShortItem {
            item_type: ItemType::Global,
            tag: 1,
            size_bytes: 2,
            data: 0x8000,
        };

        assert_eq!(item.signed_value(), -32768);
    }

    #[test]
    fn short_item_signed_value_for_four_byte_positive_value() {
        let item = ShortItem {
            item_type: ItemType::Global,
            tag: 1,
            size_bytes: 4,
            data: 0x7FFF_FFFF,
        };

        assert_eq!(item.signed_value(), 2_147_483_647);
    }

    #[test]
    fn parse_short_item_rejects_empty_input() {
        assert_eq!(
            parse_short_item(&[], 0),
            Err(ItemParseError::Truncated {
                needed: 1,
                remaining: 0,
            })
        );
    }

    #[test]
    fn parse_short_item_rejects_truncated_data_bytes() {
        assert_eq!(
            parse_short_item(&[0x75], 0),
            Err(ItemParseError::Truncated {
                needed: 2,
                remaining: 1,
            })
        );
    }

    #[test]
    fn parse_short_item_rejects_long_items() {
        assert_eq!(
            parse_short_item(&[0xFE, 0x00, 0x00], 0),
            Err(ItemParseError::LongItemsUnsupported)
        );
    }

    #[test]
    fn parse_short_item_advances_offset_across_sequence() {
        let input = [0x05, 0x01, 0x09, 0x30];

        let (first, first_next) = match parse_short_item(&input, 0) {
            Ok(parsed) => parsed,
            Err(error) => panic!("first parse_short_item should succeed: {error:?}"),
        };
        let (second, second_next) = match parse_short_item(&input, first_next) {
            Ok(parsed) => parsed,
            Err(error) => panic!("second parse_short_item should succeed: {error:?}"),
        };

        assert_eq!(first.item_type, ItemType::Global);
        assert_eq!(first_next, 2);
        assert_eq!(second.item_type, ItemType::Local);
        assert_eq!(second.unsigned_value(), 0x30);
        assert_eq!(second_next, 4);
    }

    #[test]
    fn parse_descriptor_summary_empty_descriptor_returns_default_summary() {
        assert_eq!(
            parse_descriptor_summary(&[]),
            Ok(ReportDescriptorSummary::default())
        );
    }

    #[test]
    fn parse_descriptor_summary_parses_single_x_input_field() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02,
        ];
        let summary = match parse_descriptor_summary(&descriptor) {
            Ok(summary) => summary,
            Err(error) => panic!("parse_descriptor_summary should succeed: {error:?}"),
        };

        assert_eq!(summary.field_count(), 1);
        assert_eq!(
            summary.field(0),
            Some(ReportField {
                report_id: 0,
                bit_offset: 0,
                bit_size: 8,
                logical_min: -127,
                logical_max: 127,
                usage: Usage::GenericDesktop(GenericDesktopUsage::X),
                kind: FieldKind::Variable,
            })
        );
    }

    #[test]
    fn parse_descriptor_summary_advances_offsets_for_two_fields() {
        let descriptor = [
            0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02,
            0x09, 0x31, 0x81, 0x02,
        ];
        let summary = match parse_descriptor_summary(&descriptor) {
            Ok(summary) => summary,
            Err(error) => panic!("parse_descriptor_summary should succeed: {error:?}"),
        };

        assert_eq!(summary.field_count(), 2);
        assert_eq!(summary.field(0).map(|field| field.bit_offset), Some(0));
        assert_eq!(summary.field(1).map(|field| field.bit_offset), Some(8));
        assert_eq!(
            summary.field(1).map(|field| field.usage),
            Some(Usage::GenericDesktop(GenericDesktopUsage::Y))
        );
    }

    #[test]
    fn parse_descriptor_summary_constant_input_padding_advances_bit_offset() {
        let descriptor = [
            0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x09, 0x30, 0x81, 0x02,
            0x09, 0x32, 0x81, 0x01, 0x09, 0x31, 0x81, 0x02,
        ];
        let summary = match parse_descriptor_summary(&descriptor) {
            Ok(summary) => summary,
            Err(error) => panic!("parse_descriptor_summary should succeed: {error:?}"),
        };

        assert_eq!(summary.field_count(), 2);
        assert_eq!(summary.field(1).map(|field| field.bit_offset), Some(16));
        assert_eq!(
            summary.field(1).map(|field| field.usage),
            Some(Usage::GenericDesktop(GenericDesktopUsage::Y))
        );
    }

    #[test]
    fn parse_descriptor_summary_stores_report_id() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x85, 0x01, 0x95, 0x01,
            0x81, 0x02,
        ];
        let summary = match parse_descriptor_summary(&descriptor) {
            Ok(summary) => summary,
            Err(error) => panic!("parse_descriptor_summary should succeed: {error:?}"),
        };

        assert_eq!(summary.field(0).map(|field| field.report_id), Some(1));
    }

    #[test]
    fn parse_descriptor_summary_input_flags_zero_produce_array_field_kind() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x81, 0x00,
        ];
        let summary = match parse_descriptor_summary(&descriptor) {
            Ok(summary) => summary,
            Err(error) => panic!("parse_descriptor_summary should succeed: {error:?}"),
        };

        assert_eq!(
            summary.field(0).map(|field| field.kind),
            Some(FieldKind::Array)
        );
    }

    #[test]
    fn parse_descriptor_summary_rejects_missing_usage_page() {
        let descriptor = [
            0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02,
        ];

        assert_eq!(
            parse_descriptor_summary(&descriptor),
            Err(DescriptorParseError::MissingUsagePage)
        );
    }

    #[test]
    fn parse_descriptor_summary_rejects_missing_usage() {
        let descriptor = [
            0x05, 0x01, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02,
        ];

        assert_eq!(
            parse_descriptor_summary(&descriptor),
            Err(DescriptorParseError::MissingUsage)
        );
    }

    #[test]
    fn parse_descriptor_summary_rejects_missing_report_size() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x95, 0x01, 0x81, 0x02,
        ];

        assert_eq!(
            parse_descriptor_summary(&descriptor),
            Err(DescriptorParseError::MissingReportSize)
        );
    }

    #[test]
    fn parse_descriptor_summary_rejects_missing_logical_range() {
        let descriptor = [0x05, 0x01, 0x09, 0x30, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02];

        assert_eq!(
            parse_descriptor_summary(&descriptor),
            Err(DescriptorParseError::MissingLogicalRange)
        );
    }

    #[test]
    fn parse_descriptor_summary_rejects_unsupported_report_count() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x02, 0x81, 0x02,
        ];

        assert_eq!(
            parse_descriptor_summary(&descriptor),
            Err(DescriptorParseError::UnsupportedReportCount { count: 2 })
        );
    }

    #[test]
    fn parse_descriptor_summary_wraps_long_item_tokenization_errors() {
        assert_eq!(
            parse_descriptor_summary(&[0xFE, 0x00, 0x00]),
            Err(DescriptorParseError::Item(
                ItemParseError::LongItemsUnsupported
            ))
        );
    }

    #[test]
    fn usage_page_maps_generic_desktop() {
        assert_eq!(usage_page_from_u16(0x0001), UsagePage::GenericDesktop);
    }

    #[test]
    fn usage_page_maps_button() {
        assert_eq!(usage_page_from_u16(0x0009), UsagePage::Button);
    }

    #[test]
    fn usage_page_maps_unknown_value() {
        assert_eq!(usage_page_from_u16(0x1234), UsagePage::Unknown(0x1234));
    }

    #[test]
    fn generic_desktop_usage_maps_x() {
        assert_eq!(
            generic_desktop_usage_from_u16(0x0030),
            GenericDesktopUsage::X
        );
    }

    #[test]
    fn generic_desktop_usage_maps_rz() {
        assert_eq!(
            generic_desktop_usage_from_u16(0x0035),
            GenericDesktopUsage::Rz
        );
    }

    #[test]
    fn generic_desktop_usage_maps_hat_switch() {
        assert_eq!(
            generic_desktop_usage_from_u16(0x0039),
            GenericDesktopUsage::HatSwitch
        );
    }

    #[test]
    fn generic_desktop_usage_maps_unknown_value() {
        assert_eq!(
            generic_desktop_usage_from_u16(0x9999),
            GenericDesktopUsage::Unknown(0x9999)
        );
    }

    #[test]
    fn usage_from_parts_maps_generic_desktop_x() {
        assert_eq!(
            usage_from_parts(0x0001, 0x0030),
            Usage::GenericDesktop(GenericDesktopUsage::X)
        );
    }

    #[test]
    fn usage_from_parts_maps_button_usage() {
        assert_eq!(usage_from_parts(0x0009, 3), Usage::Button(3));
    }

    #[test]
    fn descriptor_summary_default_starts_empty() {
        let summary = ReportDescriptorSummary::default();

        assert_eq!(summary, ReportDescriptorSummary::default());
    }

    #[test]
    fn descriptor_summary_field_count_starts_at_zero() {
        let summary = ReportDescriptorSummary::default();

        assert_eq!(summary.field_count(), 0);
    }

    #[test]
    fn descriptor_summary_is_empty_initially() {
        let summary = ReportDescriptorSummary::default();

        assert!(summary.is_empty());
    }

    #[test]
    fn descriptor_summary_push_field_stores_first_field() {
        let mut summary = ReportDescriptorSummary::default();
        let field = sample_report_field(0);

        assert_eq!(summary.push_field(field), Ok(()));
        assert_eq!(summary.field_count(), 1);
        assert!(!summary.is_empty());
        assert_eq!(summary.field(0), Some(field));
    }

    #[test]
    fn descriptor_summary_rejects_out_of_order_field() {
        let mut summary = ReportDescriptorSummary::default();

        assert_eq!(summary.push_field(sample_report_field(8)), Ok(()));
        assert_eq!(
            summary.push_field(sample_report_field(4)),
            Err(DescriptorError::FieldOutOfOrder)
        );
    }

    #[test]
    fn descriptor_summary_allows_filling_to_max_fields() {
        let mut summary = ReportDescriptorSummary::default();

        for index in 0..MAX_FIELDS {
            assert_eq!(
                summary.push_field(sample_report_field(index as u16)),
                Ok(())
            );
        }

        assert_eq!(summary.field_count(), MAX_FIELDS);
        assert_eq!(summary.field(MAX_FIELDS - 1), Some(sample_report_field(31)));
    }

    #[test]
    fn descriptor_summary_rejects_more_than_max_fields() {
        let mut summary = ReportDescriptorSummary::default();

        for index in 0..MAX_FIELDS {
            assert_eq!(
                summary.push_field(sample_report_field(index as u16)),
                Ok(())
            );
        }

        assert_eq!(
            summary.push_field(sample_report_field(MAX_FIELDS as u16)),
            Err(DescriptorError::TooManyFields {
                attempted: MAX_FIELDS + 1,
                max: MAX_FIELDS,
            })
        );
    }

    #[test]
    fn decode_field_value_decodes_8_bit_unsigned_at_offset_zero() {
        let field = ReportField {
            report_id: 1,
            bit_offset: 0,
            bit_size: 8,
            logical_min: 0,
            logical_max: 255,
            usage: Usage::GenericDesktop(GenericDesktopUsage::X),
            kind: FieldKind::Variable,
        };

        assert_eq!(decode_field_value(field, &[0x7F]), Ok(0x7F));
    }

    #[test]
    fn decode_field_value_decodes_8_bit_signed_negative_one() {
        let field = ReportField {
            report_id: 1,
            bit_offset: 0,
            bit_size: 8,
            logical_min: -127,
            logical_max: 127,
            usage: Usage::GenericDesktop(GenericDesktopUsage::Y),
            kind: FieldKind::Variable,
        };

        assert_eq!(decode_field_value(field, &[0xFF]), Ok(-1));
    }

    #[test]
    fn decode_field_value_decodes_16_bit_unsigned_little_endian() {
        let field = ReportField {
            report_id: 1,
            bit_offset: 0,
            bit_size: 16,
            logical_min: 0,
            logical_max: 65_535,
            usage: Usage::GenericDesktop(GenericDesktopUsage::Z),
            kind: FieldKind::Variable,
        };

        assert_eq!(decode_field_value(field, &[0x34, 0x12]), Ok(0x1234));
    }

    #[test]
    fn decode_field_value_decodes_one_bit_at_offset_zero() {
        let field = ReportField {
            report_id: 1,
            bit_offset: 0,
            bit_size: 1,
            logical_min: 0,
            logical_max: 1,
            usage: Usage::Button(1),
            kind: FieldKind::Variable,
        };

        assert_eq!(decode_field_value(field, &[0x01]), Ok(1));
    }

    #[test]
    fn decode_field_value_decodes_one_bit_at_offset_seven() {
        let field = ReportField {
            report_id: 1,
            bit_offset: 7,
            bit_size: 1,
            logical_min: 0,
            logical_max: 1,
            usage: Usage::Button(8),
            kind: FieldKind::Variable,
        };

        assert_eq!(decode_field_value(field, &[0x80]), Ok(1));
    }

    #[test]
    fn decode_field_value_rejects_unsupported_field_size() {
        let field = ReportField {
            report_id: 1,
            bit_offset: 0,
            bit_size: 12,
            logical_min: 0,
            logical_max: 4095,
            usage: Usage::GenericDesktop(GenericDesktopUsage::Rx),
            kind: FieldKind::Variable,
        };

        assert_eq!(
            decode_field_value(field, &[0x00, 0x00]),
            Err(DecodeError::UnsupportedFieldSize { bit_size: 12 })
        );
    }

    #[test]
    fn decode_field_value_rejects_too_short_payload() {
        let field = ReportField {
            report_id: 1,
            bit_offset: 8,
            bit_size: 8,
            logical_min: 0,
            logical_max: 255,
            usage: Usage::GenericDesktop(GenericDesktopUsage::Ry),
            kind: FieldKind::Variable,
        };

        assert_eq!(
            decode_field_value(field, &[0x12]),
            Err(DecodeError::ReportTooShort {
                required_bits: 16,
                actual_bits: 8,
            })
        );
    }

    #[test]
    fn decode_report_with_empty_summary_returns_empty_report() {
        let summary = ReportDescriptorSummary::default();

        assert_eq!(
            decode_report(&summary, 1, &[0x00]),
            Ok(DecodedReport::default())
        );
    }

    #[test]
    fn decode_report_decodes_two_matching_variable_fields_in_order() {
        let mut summary = ReportDescriptorSummary::default();
        let field_x = ReportField {
            report_id: 1,
            bit_offset: 0,
            bit_size: 8,
            logical_min: 0,
            logical_max: 255,
            usage: Usage::GenericDesktop(GenericDesktopUsage::X),
            kind: FieldKind::Variable,
        };
        let field_y = ReportField {
            report_id: 1,
            bit_offset: 8,
            bit_size: 8,
            logical_min: -127,
            logical_max: 127,
            usage: Usage::GenericDesktop(GenericDesktopUsage::Y),
            kind: FieldKind::Variable,
        };

        assert_eq!(summary.push_field(field_x), Ok(()));
        assert_eq!(summary.push_field(field_y), Ok(()));

        let decoded = match decode_report(&summary, 1, &[0x12, 0xFE]) {
            Ok(decoded) => decoded,
            Err(error) => panic!("decode_report should succeed: {error:?}"),
        };

        assert_eq!(decoded.field_count(), 2);
        assert!(!decoded.is_empty());
        assert_eq!(
            decoded.field(0),
            Some(DecodedFieldValue {
                usage: Usage::GenericDesktop(GenericDesktopUsage::X),
                value: 0x12,
            })
        );
        assert_eq!(
            decoded.field(1),
            Some(DecodedFieldValue {
                usage: Usage::GenericDesktop(GenericDesktopUsage::Y),
                value: -2,
            })
        );
    }

    #[test]
    fn decode_report_skips_fields_with_different_report_id() {
        let mut summary = ReportDescriptorSummary::default();
        let skipped = ReportField {
            report_id: 2,
            bit_offset: 0,
            bit_size: 8,
            logical_min: 0,
            logical_max: 255,
            usage: Usage::GenericDesktop(GenericDesktopUsage::Z),
            kind: FieldKind::Variable,
        };
        let matched = ReportField {
            report_id: 1,
            bit_offset: 8,
            bit_size: 8,
            logical_min: 0,
            logical_max: 255,
            usage: Usage::GenericDesktop(GenericDesktopUsage::Rz),
            kind: FieldKind::Variable,
        };

        assert_eq!(summary.push_field(skipped), Ok(()));
        assert_eq!(summary.push_field(matched), Ok(()));

        let decoded = match decode_report(&summary, 1, &[0xAA, 0x55]) {
            Ok(decoded) => decoded,
            Err(error) => panic!("decode_report should succeed: {error:?}"),
        };

        assert_eq!(decoded.field_count(), 1);
        assert_eq!(
            decoded.field(0),
            Some(DecodedFieldValue {
                usage: Usage::GenericDesktop(GenericDesktopUsage::Rz),
                value: 0x55,
            })
        );
    }

    #[test]
    fn decode_report_rejects_matching_array_field() {
        let mut summary = ReportDescriptorSummary::default();
        let array_field = ReportField {
            report_id: 1,
            bit_offset: 0,
            bit_size: 8,
            logical_min: 0,
            logical_max: 7,
            usage: Usage::GenericDesktop(GenericDesktopUsage::HatSwitch),
            kind: FieldKind::Array,
        };

        assert_eq!(summary.push_field(array_field), Ok(()));
        assert_eq!(
            decode_report(&summary, 1, &[0x03]),
            Err(DecodeError::ArrayFieldsUnsupported)
        );
    }

    #[test]
    fn decoded_report_field_accessors_work_after_successful_decode() {
        let mut summary = ReportDescriptorSummary::default();
        let field = ReportField {
            report_id: 1,
            bit_offset: 0,
            bit_size: 1,
            logical_min: 0,
            logical_max: 1,
            usage: Usage::Button(1),
            kind: FieldKind::Variable,
        };

        assert_eq!(summary.push_field(field), Ok(()));

        let decoded = match decode_report(&summary, 1, &[0x01]) {
            Ok(decoded) => decoded,
            Err(error) => panic!("decode_report should succeed: {error:?}"),
        };

        assert_eq!(decoded.field_count(), 1);
        assert!(!decoded.is_empty());
        assert_eq!(
            decoded.field(0),
            Some(DecodedFieldValue {
                usage: Usage::Button(1),
                value: 1,
            })
        );
        assert_eq!(decoded.field(1), None);
    }

    #[test]
    fn apply_decoded_field_sets_x_axis() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::GenericDesktop(GenericDesktopUsage::X),
                    value: 123,
                }
            ),
            Ok(())
        );
        assert_eq!(state.axis(Axis::X), 123);
    }

    #[test]
    fn apply_decoded_field_sets_y_axis() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::GenericDesktop(GenericDesktopUsage::Y),
                    value: -456,
                }
            ),
            Ok(())
        );
        assert_eq!(state.axis(Axis::Y), -456);
    }

    #[test]
    fn apply_decoded_field_sets_rz_axis() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::GenericDesktop(GenericDesktopUsage::Rz),
                    value: 789,
                }
            ),
            Ok(())
        );
        assert_eq!(state.axis(Axis::Rz), 789);
    }

    #[test]
    fn apply_decoded_field_maps_hat_zero_to_up() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::GenericDesktop(GenericDesktopUsage::HatSwitch),
                    value: 0,
                }
            ),
            Ok(())
        );
        assert_eq!(state.hat(), HatPosition::Up);
    }

    #[test]
    fn apply_decoded_field_maps_hat_eight_to_centered() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::GenericDesktop(GenericDesktopUsage::HatSwitch),
                    value: 8,
                }
            ),
            Ok(())
        );
        assert_eq!(state.hat(), HatPosition::Centered);
    }

    #[test]
    fn apply_decoded_field_maps_hat_negative_one_to_centered() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::GenericDesktop(GenericDesktopUsage::HatSwitch),
                    value: -1,
                }
            ),
            Ok(())
        );
        assert_eq!(state.hat(), HatPosition::Centered);
    }

    #[test]
    fn apply_decoded_field_rejects_unsupported_hat_value() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::GenericDesktop(GenericDesktopUsage::HatSwitch),
                    value: 9,
                }
            ),
            Err(NormalizeError::UnsupportedHatValue { value: 9 })
        );
    }

    #[test]
    fn apply_decoded_field_maps_button_usage_one_to_zero_based_button_zero() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::Button(1),
                    value: 1,
                }
            ),
            Ok(())
        );
        assert!(state.button(button_index(0)));
    }

    #[test]
    fn apply_decoded_field_maps_button_usage_six_to_zero_based_button_five() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::Button(6),
                    value: 1,
                }
            ),
            Ok(())
        );
        assert!(state.button(button_index(5)));
    }

    #[test]
    fn apply_decoded_field_maps_button_usage_sixteen_to_zero_based_button_fifteen() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::Button(16),
                    value: 1,
                }
            ),
            Ok(())
        );
        assert!(state.button(button_index(15)));
    }

    #[test]
    fn apply_decoded_field_rejects_button_usage_out_of_range() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::Button(17),
                    value: 1,
                }
            ),
            Err(NormalizeError::ButtonOutOfRange { usage: 17 })
        );
    }

    #[test]
    fn apply_decoded_field_rejects_axis_value_out_of_i16_range() {
        let mut state = NormalizedJoystickState::default();

        assert_eq!(
            apply_decoded_field(
                &mut state,
                DecodedFieldValue {
                    usage: Usage::GenericDesktop(GenericDesktopUsage::X),
                    value: 40_000,
                }
            ),
            Err(NormalizeError::AxisOutOfRange {
                usage: GenericDesktopUsage::X,
                value: 40_000,
            })
        );
    }

    #[test]
    fn normalize_decoded_report_empty_returns_default_state() {
        let report = DecodedReport {
            fields: [None; MAX_DECODED_FIELDS],
            field_count: 0,
        };

        assert_eq!(
            normalize_decoded_report(&report),
            Ok(NormalizedJoystickState::default())
        );
    }

    #[test]
    fn normalize_decoded_report_maps_axes_hat_and_buttons() {
        let mut report = DecodedReport {
            fields: [None; MAX_DECODED_FIELDS],
            field_count: 6,
        };
        report.fields[0] = Some(DecodedFieldValue {
            usage: Usage::GenericDesktop(GenericDesktopUsage::X),
            value: 111,
        });
        report.fields[1] = Some(DecodedFieldValue {
            usage: Usage::GenericDesktop(GenericDesktopUsage::Y),
            value: -222,
        });
        report.fields[2] = Some(DecodedFieldValue {
            usage: Usage::GenericDesktop(GenericDesktopUsage::Rz),
            value: 333,
        });
        report.fields[3] = Some(DecodedFieldValue {
            usage: Usage::GenericDesktop(GenericDesktopUsage::HatSwitch),
            value: 3,
        });
        report.fields[4] = Some(DecodedFieldValue {
            usage: Usage::Button(1),
            value: 1,
        });
        report.fields[5] = Some(DecodedFieldValue {
            usage: Usage::Button(6),
            value: 1,
        });
        report.fields[6] = Some(DecodedFieldValue {
            usage: Usage::Button(16),
            value: 1,
        });
        report.field_count = 7;

        let state = match normalize_decoded_report(&report) {
            Ok(state) => state,
            Err(error) => panic!("normalize_decoded_report should succeed: {error:?}"),
        };

        assert_eq!(state.axis(Axis::X), 111);
        assert_eq!(state.axis(Axis::Y), -222);
        assert_eq!(state.axis(Axis::Rz), 333);
        assert_eq!(state.hat(), HatPosition::DownRight);
        assert!(state.button(button_index(0)));
        assert!(state.button(button_index(5)));
        assert!(state.button(button_index(15)));
    }

    #[test]
    fn normalize_decoded_report_ignores_unknown_usages() {
        let mut report = DecodedReport {
            fields: [None; MAX_DECODED_FIELDS],
            field_count: 2,
        };
        report.fields[0] = Some(DecodedFieldValue {
            usage: Usage::Unknown {
                page: UsagePage::Unknown(0x7777),
                id: 9,
            },
            value: 55,
        });
        report.fields[1] = Some(DecodedFieldValue {
            usage: Usage::GenericDesktop(GenericDesktopUsage::X),
            value: 42,
        });

        let state = match normalize_decoded_report(&report) {
            Ok(state) => state,
            Err(error) => panic!("normalize_decoded_report should succeed: {error:?}"),
        };

        assert_eq!(state.axis(Axis::X), 42);
        assert_eq!(state, {
            let mut expected = NormalizedJoystickState::default();
            expected.set_axis(Axis::X, 42);
            expected
        });
    }

    #[test]
    fn normalize_decoded_report_stops_on_first_invalid_field() {
        let mut report = DecodedReport {
            fields: [None; MAX_DECODED_FIELDS],
            field_count: 3,
        };
        report.fields[0] = Some(DecodedFieldValue {
            usage: Usage::GenericDesktop(GenericDesktopUsage::X),
            value: 10,
        });
        report.fields[1] = Some(DecodedFieldValue {
            usage: Usage::GenericDesktop(GenericDesktopUsage::HatSwitch),
            value: 9,
        });
        report.fields[2] = Some(DecodedFieldValue {
            usage: Usage::Button(1),
            value: 1,
        });

        assert_eq!(
            normalize_decoded_report(&report),
            Err(NormalizeError::UnsupportedHatValue { value: 9 })
        );
    }

    #[test]
    fn default_state_is_centered_with_no_buttons_pressed() {
        let state = NormalizedJoystickState::default();

        assert_eq!(state.axis(Axis::X), 0);
        assert_eq!(state.axis(Axis::Y), 0);
        assert_eq!(state.axis(Axis::Rz), 0);
        assert_eq!(state.hat(), HatPosition::Centered);

        for raw_index in 0_u8..16_u8 {
            assert!(!state.button(button_index(raw_index)));
        }
    }

    #[test]
    fn axis_accessors_cover_all_axes() {
        let mut state = NormalizedJoystickState::default();

        state.set_axis(Axis::X, -123);
        state.set_axis(Axis::Y, 456);
        state.set_axis(Axis::Rz, 789);

        assert_eq!(state.axis(Axis::X), -123);
        assert_eq!(state.axis(Axis::Y), 456);
        assert_eq!(state.axis(Axis::Rz), 789);
    }

    #[test]
    fn hat_accessors_work() {
        let mut state = NormalizedJoystickState::default();

        state.set_hat(HatPosition::DownLeft);

        assert_eq!(state.hat(), HatPosition::DownLeft);
    }

    #[test]
    fn button_index_accepts_zero() {
        match ButtonIndex::new(0) {
            Ok(index) => assert_eq!(index.get(), 0),
            Err(error) => panic!("button index 0 should be valid: {error:?}"),
        }
    }

    #[test]
    fn button_index_accepts_last_valid_button() {
        match ButtonIndex::new(15) {
            Ok(index) => assert_eq!(index.get(), 15),
            Err(error) => panic!("button index 15 should be valid: {error:?}"),
        }
    }

    #[test]
    fn button_index_rejects_out_of_range_value() {
        assert_eq!(
            ButtonIndex::new(16),
            Err(ButtonIndexError::OutOfRange {
                index: 16,
                max_exclusive: 16,
            })
        );
    }

    #[test]
    fn button_bit_can_be_set_and_cleared() {
        let mut state = NormalizedJoystickState::default();
        let index = button_index(5);

        assert!(!state.button(index));

        state.set_button(index, true);
        assert!(state.button(index));

        state.set_button(index, false);
        assert!(!state.button(index));
    }

    #[test]
    fn clear_resets_state_back_to_default() {
        let mut state = NormalizedJoystickState::default();

        state.set_axis(Axis::X, 111);
        state.set_axis(Axis::Y, -222);
        state.set_axis(Axis::Rz, 333);
        state.set_hat(HatPosition::UpRight);
        state.set_button(button_index(3), true);

        state.clear();

        assert_eq!(state, NormalizedJoystickState::default());
    }

    #[test]
    fn v1_profile_id_exposes_stable_name() {
        assert_eq!(V1_PROFILE_ID.as_str(), "t16000m_v1");
    }

    #[test]
    fn v1_profile_id_maps_to_generic_ble_gamepad_persona() {
        assert_eq!(
            V1_PROFILE_ID.output_persona(),
            OutputPersona::GenericBleGamepad16
        );
    }

    #[test]
    fn runtime_state_default_uses_v1_profile() {
        let state = RuntimeState::default();

        assert_eq!(state.active_profile(), V1_PROFILE_ID);
    }

    #[test]
    fn runtime_state_default_starts_with_centered_input() {
        let state = RuntimeState::default();

        assert_eq!(state.last_input(), NormalizedJoystickState::default());
    }

    #[test]
    fn runtime_state_new_sets_requested_profile() {
        let state = RuntimeState::new(V1_PROFILE_ID);

        assert_eq!(state.active_profile(), V1_PROFILE_ID);
        assert_eq!(state.last_input(), NormalizedJoystickState::default());
    }

    #[test]
    fn set_active_profile_stores_requested_profile() {
        let mut state = RuntimeState::default();

        state.set_active_profile(V1_PROFILE_ID);

        assert_eq!(state.active_profile(), V1_PROFILE_ID);
    }

    #[test]
    fn update_input_stores_new_normalized_state() {
        let mut state = RuntimeState::default();
        let mut input = NormalizedJoystickState::default();

        input.set_axis(Axis::X, 12);
        input.set_axis(Axis::Y, -34);
        input.set_axis(Axis::Rz, 56);
        input.set_hat(HatPosition::Right);
        input.set_button(button_index(1), true);

        state.update_input(input);

        assert_eq!(state.last_input(), input);
    }

    #[test]
    fn clear_input_resets_to_normalized_default() {
        let mut state = RuntimeState::default();
        let mut input = NormalizedJoystickState::default();

        input.set_axis(Axis::X, 99);
        input.set_hat(HatPosition::Down);
        input.set_button(button_index(4), true);
        state.update_input(input);

        state.clear_input();

        assert_eq!(state.last_input(), NormalizedJoystickState::default());
    }

    #[test]
    fn generic_ble_gamepad_report_default_is_centered() {
        let report = GenericBleGamepad16Report::default();

        assert_eq!(report.x, 0);
        assert_eq!(report.y, 0);
        assert_eq!(report.rz, 0);
        assert_eq!(report.hat, HatPosition::Centered);
        assert_eq!(report.buttons, 0);
    }

    #[test]
    fn report_from_normalized_maps_axes_hat_and_buttons() {
        let mut input = NormalizedJoystickState::default();

        input.set_axis(Axis::X, 111);
        input.set_axis(Axis::Y, -222);
        input.set_axis(Axis::Rz, 333);
        input.set_hat(HatPosition::UpLeft);
        input.set_button(button_index(0), true);
        input.set_button(button_index(5), true);
        input.set_button(button_index(15), true);

        let report = GenericBleGamepad16Report::from_normalized(input);

        assert_eq!(report.x, 111);
        assert_eq!(report.y, -222);
        assert_eq!(report.rz, 333);
        assert_eq!(report.hat, HatPosition::UpLeft);
        assert_eq!(report.buttons, 1_u16 | (1_u16 << 5) | (1_u16 << 15));
    }

    #[test]
    fn current_report_matches_report_rendered_from_last_input() {
        let mut state = RuntimeState::default();
        let mut input = NormalizedJoystickState::default();

        input.set_axis(Axis::X, -10);
        input.set_axis(Axis::Y, 20);
        input.set_axis(Axis::Rz, -30);
        input.set_hat(HatPosition::DownRight);
        input.set_button(button_index(0), true);
        input.set_button(button_index(5), true);
        input.set_button(button_index(15), true);

        state.update_input(input);

        assert_eq!(
            state.current_report(),
            GenericBleGamepad16Report::from_normalized(state.last_input())
        );
    }
}
