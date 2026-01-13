//! Kitty graphics command parsing.
//!
//! Parses the control data portion of Kitty graphics APC sequences.
//! Format: `key=value,key=value,...`

use super::KITTY_MAX_DIMENSION;

/// Action to perform with the graphics command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Action {
    /// Transmit image data only (default).
    #[default]
    Transmit,
    /// Transmit and immediately display.
    TransmitAndDisplay,
    /// Query protocol support (no storage).
    Query,
    /// Display (put) previously transmitted image.
    Display,
    /// Delete images/placements.
    Delete,
    /// Transmit animation frame data.
    TransmitAnimationFrame,
    /// Control animation playback.
    ControlAnimation,
    /// Compose animation frames.
    ComposeAnimation,
}

impl Action {
    /// Parse action from byte value.
    pub fn from_byte(b: u8) -> Self {
        match b {
            b't' => Self::Transmit,
            b'T' => Self::TransmitAndDisplay,
            b'q' => Self::Query,
            b'p' => Self::Display,
            b'd' => Self::Delete,
            b'f' => Self::TransmitAnimationFrame,
            b'a' => Self::ControlAnimation,
            b'c' => Self::ComposeAnimation,
            _ => Self::Transmit, // Default
        }
    }

    /// Convert action to byte value.
    pub fn to_byte(self) -> u8 {
        match self {
            Self::Transmit => b't',
            Self::TransmitAndDisplay => b'T',
            Self::Query => b'q',
            Self::Display => b'p',
            Self::Delete => b'd',
            Self::TransmitAnimationFrame => b'f',
            Self::ControlAnimation => b'a',
            Self::ComposeAnimation => b'c',
        }
    }
}

/// Transmission type for image data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransmissionType {
    /// Data embedded inline in escape sequence (base64).
    #[default]
    Direct,
    /// Read from file path (path in payload).
    File,
    /// Read from temp file, terminal deletes after.
    TemporaryFile,
    /// POSIX shared memory object (zero-copy).
    SharedMemory,
}

impl TransmissionType {
    /// Parse transmission type from byte value.
    pub fn from_byte(b: u8) -> Self {
        match b {
            b'd' => Self::Direct,
            b'f' => Self::File,
            b't' => Self::TemporaryFile,
            b's' => Self::SharedMemory,
            _ => Self::Direct, // Default
        }
    }

    /// Convert transmission type to byte value.
    pub fn to_byte(self) -> u8 {
        match self {
            Self::Direct => b'd',
            Self::File => b'f',
            Self::TemporaryFile => b't',
            Self::SharedMemory => b's',
        }
    }
}

/// Delete action specifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeleteAction {
    /// Delete all visible placements on cursor's layer (lowercase = keep data).
    #[default]
    AllVisiblePlacements,
    /// Delete all visible placements and free image data (uppercase).
    AllVisiblePlacementsAndData,
    /// Delete by image ID.
    ById,
    /// Delete by image ID and free data.
    ByIdAndData,
    /// Delete by image number (newest).
    ByNumber,
    /// Delete by image number and free data.
    ByNumberAndData,
    /// Delete intersecting cursor position.
    AtCursor,
    /// Delete intersecting cursor position and free data.
    AtCursorAndData,
    /// Delete animation frames.
    AnimationFrames,
    /// Delete animation frames and free data.
    AnimationFramesAndData,
    /// Delete intersecting specific cell.
    AtCell,
    /// Delete intersecting specific cell and free data.
    AtCellAndData,
    /// Delete intersecting cell with z-index.
    AtCellWithZ,
    /// Delete intersecting cell with z-index and free data.
    AtCellWithZAndData,
    /// Delete by ID range.
    ByIdRange,
    /// Delete by ID range and free data.
    ByIdRangeAndData,
    /// Delete intersecting column.
    InColumn,
    /// Delete intersecting column and free data.
    InColumnAndData,
    /// Delete intersecting row.
    InRow,
    /// Delete intersecting row and free data.
    InRowAndData,
    /// Delete by z-index.
    ByZIndex,
    /// Delete by z-index and free data.
    ByZIndexAndData,
}

impl DeleteAction {
    /// Parse delete action from byte value.
    pub fn from_byte(b: u8) -> Self {
        match b {
            b'a' => Self::AllVisiblePlacements,
            b'A' => Self::AllVisiblePlacementsAndData,
            b'i' => Self::ById,
            b'I' => Self::ByIdAndData,
            b'n' => Self::ByNumber,
            b'N' => Self::ByNumberAndData,
            b'c' => Self::AtCursor,
            b'C' => Self::AtCursorAndData,
            b'f' => Self::AnimationFrames,
            b'F' => Self::AnimationFramesAndData,
            b'p' => Self::AtCell,
            b'P' => Self::AtCellAndData,
            b'q' => Self::AtCellWithZ,
            b'Q' => Self::AtCellWithZAndData,
            b'r' => Self::ByIdRange,
            b'R' => Self::ByIdRangeAndData,
            b'x' => Self::InColumn,
            b'X' => Self::InColumnAndData,
            b'y' => Self::InRow,
            b'Y' => Self::InRowAndData,
            b'z' => Self::ByZIndex,
            b'Z' => Self::ByZIndexAndData,
            _ => Self::AllVisiblePlacements, // Default
        }
    }

    /// Returns true if this action should also free image data.
    pub fn frees_data(self) -> bool {
        matches!(
            self,
            Self::AllVisiblePlacementsAndData
                | Self::ByIdAndData
                | Self::ByNumberAndData
                | Self::AtCursorAndData
                | Self::AnimationFramesAndData
                | Self::AtCellAndData
                | Self::AtCellWithZAndData
                | Self::ByIdRangeAndData
                | Self::InColumnAndData
                | Self::InRowAndData
                | Self::ByZIndexAndData
        )
    }
}

/// Compression type for image data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionType {
    /// No compression.
    #[default]
    None,
    /// zlib deflate (RFC 1950).
    Zlib,
}

impl CompressionType {
    /// Parse compression type from byte value.
    pub fn from_byte(b: u8) -> Self {
        match b {
            b'z' => Self::Zlib,
            _ => Self::None,
        }
    }

    /// Convert compression type to byte value.
    pub fn to_byte(self) -> Option<u8> {
        match self {
            Self::None => None,
            Self::Zlib => Some(b'z'),
        }
    }
}

/// Image format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageFormat {
    /// 24-bit RGB (3 bytes/pixel).
    Rgb24,
    /// 32-bit RGBA (4 bytes/pixel, default).
    #[default]
    Rgba32,
    /// PNG encoded (dimensions from PNG data).
    Png,
}

impl ImageFormat {
    /// Parse image format from numeric value.
    pub fn from_value(v: u32) -> Self {
        match v {
            24 => Self::Rgb24,
            32 => Self::Rgba32,
            100 => Self::Png,
            _ => Self::Rgba32, // Default
        }
    }

    /// Convert image format to numeric value.
    pub fn to_value(self) -> u32 {
        match self {
            Self::Rgb24 => 24,
            Self::Rgba32 => 32,
            Self::Png => 100,
        }
    }

    /// Returns bytes per pixel for this format.
    pub fn bytes_per_pixel(self) -> Option<u32> {
        match self {
            Self::Rgb24 => Some(3),
            Self::Rgba32 => Some(4),
            Self::Png => None, // PNG is compressed, variable size
        }
    }
}

/// Cursor movement policy after displaying image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorMovement {
    /// Move cursor after image (default).
    #[default]
    Move,
    /// Don't move cursor.
    Stay,
}

impl CursorMovement {
    /// Parse cursor movement from value.
    pub fn from_value(v: u32) -> Self {
        match v {
            1 => Self::Stay,
            _ => Self::Move,
        }
    }

    /// Convert cursor movement to value.
    pub fn to_value(self) -> u32 {
        match self {
            Self::Move => 0,
            Self::Stay => 1,
        }
    }
}

/// Animation control state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationState {
    /// Animation is stopped.
    #[default]
    Stopped,
    /// Animation is loading (waiting for more frames).
    Loading,
    /// Animation is running/looping.
    Running,
}

impl AnimationState {
    /// Parse animation state from value (s key).
    pub fn from_value(v: u32) -> Self {
        match v {
            1 => Self::Stopped,
            2 => Self::Loading,
            3 => Self::Running,
            _ => Self::Stopped,
        }
    }

    /// Convert animation state to value.
    pub fn to_value(self) -> u32 {
        match self {
            Self::Stopped => 1,
            Self::Loading => 2,
            Self::Running => 3,
        }
    }
}

/// Frame composition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompositionMode {
    /// Alpha blending (default).
    #[default]
    AlphaBlend,
    /// Overwrite pixels completely.
    Overwrite,
}

impl CompositionMode {
    /// Parse composition mode from value.
    pub fn from_value(v: u32) -> Self {
        match v {
            1 => Self::Overwrite,
            _ => Self::AlphaBlend,
        }
    }

    /// Convert composition mode to value.
    pub fn to_value(self) -> u32 {
        match self {
            Self::AlphaBlend => 0,
            Self::Overwrite => 1,
        }
    }
}

/// Parsed Kitty graphics command.
///
/// Contains all parameters from the control data portion of an APC sequence.
#[derive(Debug, Clone, Default)]
pub struct KittyGraphicsCommand {
    // === Action ===
    /// Action to perform (a key).
    pub action: Action,

    // === Image Identification ===
    /// Image ID (i key). 0 means auto-assign.
    pub image_id: u32,
    /// Image number (I key). Terminal assigns ID.
    pub image_number: u32,
    /// Placement ID (p key). 0 means auto-assign.
    pub placement_id: u32,

    // === Transmission ===
    /// Transmission type (t key).
    pub transmission_type: TransmissionType,
    /// Compression type (o key).
    pub compression: CompressionType,
    /// More chunks follow (m key). 0 = final, 1 = more coming.
    pub more: bool,

    // === Image Data Dimensions ===
    /// Image format (f key).
    pub format: ImageFormat,
    /// Image width in pixels (s key).
    pub data_width: u32,
    /// Image height in pixels (v key).
    pub data_height: u32,
    /// Size of data to read from file (S key).
    pub data_size: u32,
    /// Offset in file to start reading (O key).
    pub data_offset: u32,

    // === Display: Source Rectangle ===
    /// Source rectangle width (w key).
    pub source_width: u32,
    /// Source rectangle height (h key).
    pub source_height: u32,
    /// Source rectangle x offset (x key).
    pub source_x: u32,
    /// Source rectangle y offset (y key).
    pub source_y: u32,

    // === Display: Cell Offset ===
    /// Pixel offset within starting cell, x (X key).
    pub cell_x_offset: u32,
    /// Pixel offset within starting cell, y (Y key).
    pub cell_y_offset: u32,

    // === Display: Size ===
    /// Number of columns to display (c key).
    pub num_columns: u32,
    /// Number of rows to display (r key).
    pub num_rows: u32,

    // === Display: Z-Index ===
    /// Vertical stacking order (z key). Negative = below text.
    pub z_index: i32,

    // === Display: Cursor ===
    /// Cursor movement policy (C key for non-animation).
    pub cursor_movement: CursorMovement,

    // === Delete ===
    /// Delete action type (d key).
    pub delete_action: DeleteAction,

    // === Unicode Placement ===
    /// Create virtual placement for Unicode placeholders (U key).
    pub unicode_placement: bool,

    // === Relative Placement ===
    /// Parent image ID for relative placement (P key).
    pub parent_id: u32,
    /// Parent placement ID (Q key).
    pub parent_placement_id: u32,
    /// Horizontal offset from parent in cells (H key).
    pub offset_from_parent_x: i32,
    /// Vertical offset from parent in cells (V key).
    pub offset_from_parent_y: i32,

    // === Response ===
    /// Quiet level (q key). 0 = respond, 1 = only errors, 2 = silent.
    pub quiet: u32,

    // === Animation ===
    /// Frame gap in milliseconds (z key for animation frames).
    /// Positive = wait time between frames.
    /// Negative = gapless frame (instantly skipped to next).
    pub frame_gap: i32,
    /// Source frame number for composition (r key for a=c).
    pub source_frame: u32,
    /// Destination frame number for composition (c key for a=c).
    pub dest_frame: u32,
    /// Animation state control (s key for a=a).
    pub animation_state: AnimationState,
    /// Loop count (v key for a=a). 0 = ignore, 1 = infinite, >1 = loop (n-1) times.
    pub loop_count: u32,
    /// Composition mode (C key for a=c).
    pub composition_mode: CompositionMode,
    /// Background color for frame composition (Y key for a=c), RGBA.
    pub background_color: u32,
    /// Base frame ID for delta frames (b key).
    pub base_frame: u32,
}

impl KittyGraphicsCommand {
    /// Create a new command with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a command from control data bytes.
    ///
    /// Control data format: `key=value,key=value,...`
    /// The 'G' prefix should already be stripped.
    pub fn parse(data: &[u8]) -> Self {
        let mut cmd = Self::new();

        // Split by comma
        for part in data.split(|&b| b == b',') {
            if part.is_empty() {
                continue;
            }

            // Find '='
            if let Some(eq_pos) = part.iter().position(|&b| b == b'=') {
                if eq_pos == 0 || eq_pos + 1 >= part.len() {
                    continue;
                }

                let key = part[0];
                let value = &part[eq_pos + 1..];

                cmd.parse_key_value(key, value);
            }
        }

        // Clamp dimensions to prevent DoS
        cmd.data_width = cmd.data_width.min(KITTY_MAX_DIMENSION);
        cmd.data_height = cmd.data_height.min(KITTY_MAX_DIMENSION);
        cmd.source_width = cmd.source_width.min(KITTY_MAX_DIMENSION);
        cmd.source_height = cmd.source_height.min(KITTY_MAX_DIMENSION);

        cmd
    }

    /// Parse a single key=value pair.
    fn parse_key_value(&mut self, key: u8, value: &[u8]) {
        match key {
            // Action
            b'a' => {
                if let Some(&b) = value.first() {
                    self.action = Action::from_byte(b);
                }
            }

            // Image identification
            b'i' => self.image_id = parse_u32(value),
            b'I' => self.image_number = parse_u32(value),
            b'p' => self.placement_id = parse_u32(value),

            // Transmission
            b't' => {
                if let Some(&b) = value.first() {
                    self.transmission_type = TransmissionType::from_byte(b);
                }
            }
            b'o' => {
                if let Some(&b) = value.first() {
                    self.compression = CompressionType::from_byte(b);
                }
            }
            b'm' => self.more = parse_u32(value) != 0,

            // Image data dimensions
            b'f' => self.format = ImageFormat::from_value(parse_u32(value)),
            // s = data_width for transmit, animation_state for control
            b's' => {
                let v = parse_u32(value);
                self.data_width = v;
                self.animation_state = AnimationState::from_value(v);
            }
            // Note: v means data_height for transmit, loop_count for animation control
            b'v' => {
                let v = parse_u32(value);
                self.data_height = v;
                self.loop_count = v;
            }
            b'S' => self.data_size = parse_u32(value),
            b'O' => self.data_offset = parse_u32(value),

            // Display: source rectangle
            b'w' => self.source_width = parse_u32(value),
            b'h' => self.source_height = parse_u32(value),
            b'x' => self.source_x = parse_u32(value),
            b'y' => self.source_y = parse_u32(value),

            // Display: cell offset (X also used for composition dest_x)
            b'X' => self.cell_x_offset = parse_u32(value),
            // Y: cell offset OR background color for composition
            b'Y' => {
                let v = parse_u32(value);
                self.cell_y_offset = v;
                self.background_color = v;
            }

            // Display: size / Animation: frame numbers
            // c = num_columns for display, dest_frame for animation composition
            b'c' => {
                let v = parse_u32(value);
                self.num_columns = v;
                self.dest_frame = v;
            }
            // r = num_rows for display, source_frame for animation composition
            b'r' => {
                let v = parse_u32(value);
                self.num_rows = v;
                self.source_frame = v;
            }

            // z = z_index for display, frame_gap for animation frames
            b'z' => {
                let v = parse_i32(value);
                self.z_index = v;
                self.frame_gap = v;
            }

            // C = cursor_movement for display, composition_mode for animation
            b'C' => {
                let v = parse_u32(value);
                self.cursor_movement = CursorMovement::from_value(v);
                self.composition_mode = CompositionMode::from_value(v);
            }

            // Delete
            b'd' => {
                if let Some(&b) = value.first() {
                    self.delete_action = DeleteAction::from_byte(b);
                }
            }

            // Unicode placement
            b'U' => self.unicode_placement = parse_u32(value) != 0,

            // Relative placement
            b'P' => self.parent_id = parse_u32(value),
            b'Q' => self.parent_placement_id = parse_u32(value),
            b'H' => self.offset_from_parent_x = parse_i32(value),
            b'V' => self.offset_from_parent_y = parse_i32(value),

            // Response
            b'q' => self.quiet = parse_u32(value),

            // Animation-specific keys
            b'b' => self.base_frame = parse_u32(value),

            // Unknown keys are ignored
            _ => {}
        }
    }

    /// Returns true if this command expects payload data.
    pub fn expects_payload(&self) -> bool {
        matches!(
            self.action,
            Action::Transmit | Action::TransmitAndDisplay | Action::TransmitAnimationFrame
        )
    }

    /// Returns true if this command should display the image.
    pub fn should_display(&self) -> bool {
        matches!(self.action, Action::TransmitAndDisplay | Action::Display)
    }

    /// Returns true if this is the final chunk of a multi-chunk transmission.
    pub fn is_final_chunk(&self) -> bool {
        !self.more
    }

    /// Returns true if response should be sent for successful operations.
    pub fn should_respond_on_success(&self) -> bool {
        self.quiet == 0
    }

    /// Returns true if response should be sent for errors.
    pub fn should_respond_on_error(&self) -> bool {
        self.quiet < 2
    }
}

/// Parse a u32 from ASCII decimal digits.
fn parse_u32(data: &[u8]) -> u32 {
    let mut result: u32 = 0;
    for &b in data {
        if b.is_ascii_digit() {
            result = result
                .saturating_mul(10)
                .saturating_add(u32::from(b - b'0'));
        } else {
            break;
        }
    }
    result
}

/// Parse an i32 from ASCII decimal digits with optional leading minus.
fn parse_i32(data: &[u8]) -> i32 {
    if data.is_empty() {
        return 0;
    }

    let (negative, digits) = if data[0] == b'-' {
        (true, &data[1..])
    } else {
        (false, data)
    };

    let magnitude = parse_u32(digits);

    if negative {
        // Handle i32::MIN case: 2147483648 as u32 can't be negated as i32
        // Clamp to i32::MIN for values >= 2147483648
        if magnitude > i32::MAX as u32 {
            i32::MIN
        } else {
            // SAFETY: magnitude is <= i32::MAX, so cast is safe
            #[allow(clippy::cast_possible_wrap)]
            {
                -(magnitude as i32)
            }
        }
    } else {
        // Clamp to i32::MAX for large positive values
        // SAFETY: magnitude is clamped to i32::MAX, so cast is safe
        #[allow(clippy::cast_possible_wrap)]
        {
            magnitude.min(i32::MAX as u32) as i32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let cmd = KittyGraphicsCommand::parse(b"");
        assert_eq!(cmd.action, Action::Transmit);
        assert_eq!(cmd.image_id, 0);
    }

    #[test]
    fn parse_simple_transmit() {
        let cmd = KittyGraphicsCommand::parse(b"a=T,i=123,s=100,v=50,f=32");
        assert_eq!(cmd.action, Action::TransmitAndDisplay);
        assert_eq!(cmd.image_id, 123);
        assert_eq!(cmd.data_width, 100);
        assert_eq!(cmd.data_height, 50);
        assert_eq!(cmd.format, ImageFormat::Rgba32);
    }

    #[test]
    fn parse_display() {
        let cmd = KittyGraphicsCommand::parse(b"a=p,i=42,p=1,c=10,r=5");
        assert_eq!(cmd.action, Action::Display);
        assert_eq!(cmd.image_id, 42);
        assert_eq!(cmd.placement_id, 1);
        assert_eq!(cmd.num_columns, 10);
        assert_eq!(cmd.num_rows, 5);
    }

    #[test]
    fn parse_delete() {
        let cmd = KittyGraphicsCommand::parse(b"a=d,d=I,i=99");
        assert_eq!(cmd.action, Action::Delete);
        assert_eq!(cmd.delete_action, DeleteAction::ByIdAndData);
        assert_eq!(cmd.image_id, 99);
        assert!(cmd.delete_action.frees_data());
    }

    #[test]
    fn parse_chunked() {
        let cmd = KittyGraphicsCommand::parse(b"a=t,i=1,s=100,v=100,m=1");
        assert!(cmd.more);
        assert!(!cmd.is_final_chunk());

        let cmd2 = KittyGraphicsCommand::parse(b"m=0");
        assert!(!cmd2.more);
        assert!(cmd2.is_final_chunk());
    }

    #[test]
    fn parse_query() {
        let cmd = KittyGraphicsCommand::parse(b"a=q,i=123");
        assert_eq!(cmd.action, Action::Query);
        assert_eq!(cmd.image_id, 123);
    }

    #[test]
    fn parse_z_index_negative() {
        let cmd = KittyGraphicsCommand::parse(b"a=p,i=1,z=-1");
        assert_eq!(cmd.z_index, -1);
    }

    #[test]
    fn parse_z_index_positive() {
        let cmd = KittyGraphicsCommand::parse(b"a=p,i=1,z=100");
        assert_eq!(cmd.z_index, 100);
    }

    #[test]
    fn parse_cursor_movement() {
        let cmd = KittyGraphicsCommand::parse(b"a=T,C=1");
        assert_eq!(cmd.cursor_movement, CursorMovement::Stay);

        let cmd2 = KittyGraphicsCommand::parse(b"a=T,C=0");
        assert_eq!(cmd2.cursor_movement, CursorMovement::Move);
    }

    #[test]
    fn parse_transmission_types() {
        let cmd = KittyGraphicsCommand::parse(b"t=d");
        assert_eq!(cmd.transmission_type, TransmissionType::Direct);

        let cmd = KittyGraphicsCommand::parse(b"t=f");
        assert_eq!(cmd.transmission_type, TransmissionType::File);

        let cmd = KittyGraphicsCommand::parse(b"t=t");
        assert_eq!(cmd.transmission_type, TransmissionType::TemporaryFile);

        let cmd = KittyGraphicsCommand::parse(b"t=s");
        assert_eq!(cmd.transmission_type, TransmissionType::SharedMemory);
    }

    #[test]
    fn parse_compression() {
        let cmd = KittyGraphicsCommand::parse(b"o=z");
        assert_eq!(cmd.compression, CompressionType::Zlib);
    }

    #[test]
    fn parse_format_types() {
        let cmd = KittyGraphicsCommand::parse(b"f=24");
        assert_eq!(cmd.format, ImageFormat::Rgb24);
        assert_eq!(cmd.format.bytes_per_pixel(), Some(3));

        let cmd = KittyGraphicsCommand::parse(b"f=32");
        assert_eq!(cmd.format, ImageFormat::Rgba32);
        assert_eq!(cmd.format.bytes_per_pixel(), Some(4));

        let cmd = KittyGraphicsCommand::parse(b"f=100");
        assert_eq!(cmd.format, ImageFormat::Png);
        assert_eq!(cmd.format.bytes_per_pixel(), None);
    }

    #[test]
    fn parse_unicode_placement() {
        let cmd = KittyGraphicsCommand::parse(b"a=T,U=1");
        assert!(cmd.unicode_placement);
    }

    #[test]
    fn parse_relative_placement() {
        let cmd = KittyGraphicsCommand::parse(b"a=p,P=42,Q=1,H=-5,V=3");
        assert_eq!(cmd.parent_id, 42);
        assert_eq!(cmd.parent_placement_id, 1);
        assert_eq!(cmd.offset_from_parent_x, -5);
        assert_eq!(cmd.offset_from_parent_y, 3);
    }

    #[test]
    fn parse_quiet_levels() {
        let cmd = KittyGraphicsCommand::parse(b"q=0");
        assert!(cmd.should_respond_on_success());
        assert!(cmd.should_respond_on_error());

        let cmd = KittyGraphicsCommand::parse(b"q=1");
        assert!(!cmd.should_respond_on_success());
        assert!(cmd.should_respond_on_error());

        let cmd = KittyGraphicsCommand::parse(b"q=2");
        assert!(!cmd.should_respond_on_success());
        assert!(!cmd.should_respond_on_error());
    }

    #[test]
    fn parse_source_rectangle() {
        let cmd = KittyGraphicsCommand::parse(b"a=p,x=10,y=20,w=50,h=30");
        assert_eq!(cmd.source_x, 10);
        assert_eq!(cmd.source_y, 20);
        assert_eq!(cmd.source_width, 50);
        assert_eq!(cmd.source_height, 30);
    }

    #[test]
    fn parse_cell_offset() {
        let cmd = KittyGraphicsCommand::parse(b"X=5,Y=3");
        assert_eq!(cmd.cell_x_offset, 5);
        assert_eq!(cmd.cell_y_offset, 3);
    }

    #[test]
    fn dimension_limit_enforced() {
        let cmd = KittyGraphicsCommand::parse(b"s=999999,v=999999");
        assert_eq!(cmd.data_width, KITTY_MAX_DIMENSION);
        assert_eq!(cmd.data_height, KITTY_MAX_DIMENSION);
    }

    #[test]
    fn parse_file_transmission() {
        let cmd = KittyGraphicsCommand::parse(b"t=f,S=1024,O=512");
        assert_eq!(cmd.transmission_type, TransmissionType::File);
        assert_eq!(cmd.data_size, 1024);
        assert_eq!(cmd.data_offset, 512);
    }

    #[test]
    fn expects_payload() {
        assert!(KittyGraphicsCommand::parse(b"a=t").expects_payload());
        assert!(KittyGraphicsCommand::parse(b"a=T").expects_payload());
        assert!(KittyGraphicsCommand::parse(b"a=f").expects_payload());
        assert!(!KittyGraphicsCommand::parse(b"a=p").expects_payload());
        assert!(!KittyGraphicsCommand::parse(b"a=d").expects_payload());
        assert!(!KittyGraphicsCommand::parse(b"a=q").expects_payload());
    }

    #[test]
    fn should_display() {
        assert!(KittyGraphicsCommand::parse(b"a=T").should_display());
        assert!(KittyGraphicsCommand::parse(b"a=p").should_display());
        assert!(!KittyGraphicsCommand::parse(b"a=t").should_display());
        assert!(!KittyGraphicsCommand::parse(b"a=d").should_display());
    }

    #[test]
    fn action_roundtrip() {
        let actions = [
            Action::Transmit,
            Action::TransmitAndDisplay,
            Action::Query,
            Action::Display,
            Action::Delete,
            Action::TransmitAnimationFrame,
            Action::ControlAnimation,
            Action::ComposeAnimation,
        ];

        for action in actions {
            let byte = action.to_byte();
            let parsed = Action::from_byte(byte);
            assert_eq!(action, parsed);
        }
    }

    #[test]
    fn transmission_type_roundtrip() {
        let types = [
            TransmissionType::Direct,
            TransmissionType::File,
            TransmissionType::TemporaryFile,
            TransmissionType::SharedMemory,
        ];

        for tt in types {
            let byte = tt.to_byte();
            let parsed = TransmissionType::from_byte(byte);
            assert_eq!(tt, parsed);
        }
    }

    #[test]
    fn format_roundtrip() {
        let formats = [ImageFormat::Rgb24, ImageFormat::Rgba32, ImageFormat::Png];

        for format in formats {
            let value = format.to_value();
            let parsed = ImageFormat::from_value(value);
            assert_eq!(format, parsed);
        }
    }

    #[test]
    fn delete_actions_complete() {
        // Test all delete action bytes
        let pairs = [
            (b'a', DeleteAction::AllVisiblePlacements),
            (b'A', DeleteAction::AllVisiblePlacementsAndData),
            (b'i', DeleteAction::ById),
            (b'I', DeleteAction::ByIdAndData),
            (b'n', DeleteAction::ByNumber),
            (b'N', DeleteAction::ByNumberAndData),
            (b'c', DeleteAction::AtCursor),
            (b'C', DeleteAction::AtCursorAndData),
            (b'f', DeleteAction::AnimationFrames),
            (b'F', DeleteAction::AnimationFramesAndData),
            (b'p', DeleteAction::AtCell),
            (b'P', DeleteAction::AtCellAndData),
            (b'q', DeleteAction::AtCellWithZ),
            (b'Q', DeleteAction::AtCellWithZAndData),
            (b'r', DeleteAction::ByIdRange),
            (b'R', DeleteAction::ByIdRangeAndData),
            (b'x', DeleteAction::InColumn),
            (b'X', DeleteAction::InColumnAndData),
            (b'y', DeleteAction::InRow),
            (b'Y', DeleteAction::InRowAndData),
            (b'z', DeleteAction::ByZIndex),
            (b'Z', DeleteAction::ByZIndexAndData),
        ];

        for (byte, expected) in pairs {
            let parsed = DeleteAction::from_byte(byte);
            assert_eq!(parsed, expected);
        }
    }
}
