//! Kitty Graphics Protocol fuzzer - tests image protocol parsing and storage.
//!
//! This fuzzer exercises the Kitty graphics protocol implementation:
//! 1. Command parsing robustness
//! 2. Image storage safety
//! 3. Animation frame handling
//! 4. Placement management
//! 5. Memory quota enforcement
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run kitty_graphics -- -max_total_time=600
//! ```

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use dterm_core::kitty_graphics::{
    Action, AnimationFrame, DeleteAction,
    KittyGraphicsCommand, KittyImage, KittyImageStorage, KittyPlacement,
};

/// Operations to perform on Kitty graphics storage.
#[derive(Debug, Arbitrary)]
enum KittyOperation {
    /// Parse a command from raw bytes
    ParseCommand { data: Vec<u8> },

    /// Store an image
    StoreImage {
        id: u32,
        width: u16,
        height: u16,
        data_len: u16,
    },

    /// Add a placement to an image
    AddPlacement { image_id: u32, row: u16, col: u16 },

    /// Remove a placement
    RemovePlacement { image_id: u32, placement_id: u32 },

    /// Remove an image
    RemoveImage { image_id: u32 },

    /// Delete by various criteria
    Delete { delete_type: u8, param: u32 },

    /// Delete at position
    DeleteAtPosition { row: u16, col: u16 },

    /// Delete in row
    DeleteInRow { row: u16 },

    /// Delete in column
    DeleteInColumn { col: u16 },

    /// Start chunked transmission
    StartChunked { command_data: Vec<u8> },

    /// Continue chunked transmission
    ContinueChunked { data: Vec<u8>, is_final: bool },

    /// Cancel chunked transmission
    CancelChunked,

    /// Clear all images
    Clear,

    /// Evict unreferenced images
    EvictUnreferenced,

    /// Set storage quota
    SetQuota { quota: u32 },

    /// Add animation frame to image
    AddAnimationFrame {
        image_id: u32,
        frame_number: u32,
        width: u16,
        height: u16,
        data_len: u16,
    },

    /// Remove animation frame
    RemoveFrame { image_id: u32, frame_number: u32 },

    /// Advance animation frame
    AdvanceFrame { image_id: u32 },

    /// Reset animation
    ResetAnimation { image_id: u32 },

    /// Query image
    GetImage { image_id: u32 },

    /// Query image by number
    GetImageByNumber { image_number: u32 },

    /// Check dirty flag
    CheckDirty,

    /// Clear dirty flag
    ClearDirty,

    /// Mark dirty
    MarkDirty,
}

/// Verify storage invariants.
fn verify_invariants(storage: &KittyImageStorage) {
    // Total bytes should not exceed quota by too much
    // (Some slack is allowed due to metadata)
    let total = storage.total_bytes();
    let quota = storage.quota();

    // Image count should be bounded
    let count = storage.image_count();
    assert!(count <= 10000, "Too many images: {}", count);

    // Note: We can't assert total <= quota because:
    // 1. Quota can be changed after data is loaded
    // 2. Multi-chunk loads may temporarily exceed quota
    // Instead, just verify we can query the values without panic
    let _ = (total, quota, count);
}

fn execute_operation(storage: &mut KittyImageStorage, op: &KittyOperation) {
    match op {
        KittyOperation::ParseCommand { data } => {
            // Parsing should never panic
            let _cmd = KittyGraphicsCommand::parse(data);
        }

        KittyOperation::StoreImage {
            id,
            width,
            height,
            data_len,
        } => {
            // Clamp dimensions
            let width = (*width).max(1).min(1000);
            let height = (*height).max(1).min(1000);
            let data_len = (*data_len as usize).min(width as usize * height as usize * 4);

            // Create image with dummy data
            let data = vec![0u8; data_len];
            let image = KittyImage::new(*id, width as u32, height as u32, data);

            // Store may succeed or fail (quota exceeded) - both are fine
            let _ = storage.store_image(image);
        }

        KittyOperation::AddPlacement { image_id, row, col } => {
            let placement = KittyPlacement::new(*image_id, *row as u32, *col as u32);
            let _ = storage.add_placement(*image_id, placement);
        }

        KittyOperation::RemovePlacement {
            image_id,
            placement_id,
        } => {
            let _ = storage.remove_placement(*image_id, *placement_id);
        }

        KittyOperation::RemoveImage { image_id } => {
            let _ = storage.remove_image(*image_id);
        }

        KittyOperation::Delete { delete_type, param } => {
            // Create a delete command
            let mut cmd = KittyGraphicsCommand::new();
            cmd.action = Action::Delete;
            cmd.image_id = *param;
            cmd.image_number = *param;
            cmd.delete_action = DeleteAction::from_byte(*delete_type);

            storage.delete(&cmd);
        }

        KittyOperation::DeleteAtPosition { row, col } => {
            storage.delete_at_position(*row as u32, *col as u32, true);
        }

        KittyOperation::DeleteInRow { row } => {
            storage.delete_in_row(*row as u32, true);
        }

        KittyOperation::DeleteInColumn { col } => {
            storage.delete_in_column(*col as u32, true);
        }

        KittyOperation::StartChunked { command_data } => {
            let cmd = KittyGraphicsCommand::parse(command_data);
            let _ = storage.start_chunked(cmd);
        }

        KittyOperation::ContinueChunked { data, is_final } => {
            let _ = storage.continue_chunked(data, *is_final);
        }

        KittyOperation::CancelChunked => {
            storage.cancel_chunked();
        }

        KittyOperation::Clear => {
            storage.clear();
        }

        KittyOperation::EvictUnreferenced => {
            storage.evict_unreferenced();
        }

        KittyOperation::SetQuota { quota } => {
            // Clamp quota to reasonable range
            let quota = (*quota as usize).max(1024).min(100 * 1024 * 1024);
            storage.set_quota(quota);
        }

        KittyOperation::AddAnimationFrame {
            image_id,
            frame_number,
            width,
            height,
            data_len,
        } => {
            if let Some(image) = storage.get_image_mut(*image_id) {
                let width = (*width).max(1).min(1000);
                let height = (*height).max(1).min(1000);
                let data_len = (*data_len as usize).min(width as usize * height as usize * 4);
                let data = vec![0u8; data_len];

                let frame = AnimationFrame::new(*frame_number, data, width as u32, height as u32);
                let _ = image.add_frame(frame);
            }
        }

        KittyOperation::RemoveFrame {
            image_id,
            frame_number,
        } => {
            if let Some(image) = storage.get_image_mut(*image_id) {
                let _ = image.remove_frame(*frame_number);
            }
        }

        KittyOperation::AdvanceFrame { image_id } => {
            if let Some(image) = storage.get_image_mut(*image_id) {
                let _ = image.advance_frame();
            }
        }

        KittyOperation::ResetAnimation { image_id } => {
            if let Some(image) = storage.get_image_mut(*image_id) {
                image.reset_animation();
            }
        }

        KittyOperation::GetImage { image_id } => {
            let _ = storage.get_image(*image_id);
        }

        KittyOperation::GetImageByNumber { image_number } => {
            let _ = storage.get_image_by_number(*image_number);
        }

        KittyOperation::CheckDirty => {
            let _ = storage.is_dirty();
        }

        KittyOperation::ClearDirty => {
            storage.clear_dirty();
        }

        KittyOperation::MarkDirty => {
            storage.mark_dirty();
        }
    }

    // Verify invariants after each operation
    verify_invariants(storage);
}

fuzz_target!(|data: &[u8]| {
    // === Phase 1: Pure command parsing ===
    // Try parsing the raw input as a command
    let _cmd = KittyGraphicsCommand::parse(data);

    // === Phase 2: Structured operations ===
    if let Ok(operations) =
        <Vec<KittyOperation> as Arbitrary>::arbitrary(&mut Unstructured::new(data))
    {
        // Use a small quota to trigger quota enforcement
        let mut storage = KittyImageStorage::with_quota(1024 * 1024); // 1MB

        for op in operations.iter().take(100) {
            execute_operation(&mut storage, op);
        }
    }

    // === Phase 3: Terminal integration ===
    // Test Kitty graphics through full terminal processing
    if data.len() >= 10 {
        use dterm_core::terminal::Terminal;

        let mut terminal = Terminal::new(24, 80);

        // Feed data that might contain Kitty graphics sequences
        terminal.process(data);

        // Verify terminal state
        let _ = terminal.cursor();
        let _ = terminal.kitty_graphics().image_count();
        let _ = terminal.kitty_graphics().total_bytes();
    }

    // === Phase 4: Specific escape sequence patterns ===
    if data.len() >= 4 {
        use dterm_core::terminal::Terminal;

        let mut terminal = Terminal::new(24, 80);

        // Build various Kitty graphics escape sequences
        // APC = ESC _ G, ST = ESC \

        // Query command
        let query = format!(
            "\x1b_Ga=q,i={}\x1b\\",
            u32::from_le_bytes([data[0], data[1], data[2], data[3]])
        );
        terminal.process(query.as_bytes());

        // Transmit command with dummy data
        let transmit = format!(
            "\x1b_Ga=T,i={},s=1,v=1,f=24;{}\x1b\\",
            data[0] as u32,
            base64_encode(&[0u8; 4])
        );
        terminal.process(transmit.as_bytes());

        // Delete command
        let delete = format!("\x1b_Ga=d,d={}\x1b\\", data[0] as char);
        terminal.process(delete.as_bytes());

        // Verify state
        let _ = terminal.kitty_graphics().image_count();
    }

    // === Phase 5: Chunked transmission ===
    if data.len() >= 20 {
        use dterm_core::terminal::Terminal;

        let mut terminal = Terminal::new(24, 80);

        // Start chunked transmission
        let start = format!(
            "\x1b_Ga=t,i=1,s={},v={},m=1;{}\x1b\\",
            data[0] as u32 % 100 + 1,
            data[1] as u32 % 100 + 1,
            base64_encode(&data[2..data.len().min(10)])
        );
        terminal.process(start.as_bytes());

        // Continue with more chunks
        for chunk in data[10..].chunks(8) {
            let cont = format!("\x1b_Gm=1;{}\x1b\\", base64_encode(chunk));
            terminal.process(cont.as_bytes());
        }

        // Final chunk
        terminal.process(b"\x1b_Gm=0;\x1b\\");

        // Verify state
        let _ = terminal.kitty_graphics().image_count();
    }

    // === Phase 6: Animation sequences ===
    if data.len() >= 8 {
        use dterm_core::terminal::Terminal;

        let mut terminal = Terminal::new(24, 80);

        // Create base image first
        let base = "\x1b_Ga=T,i=100,s=2,v=2,f=24;AAAAAAAAAAAAAA==\x1b\\";
        terminal.process(base.as_bytes());

        // Add animation frame
        let frame = format!(
            "\x1b_Ga=f,i=100,r={};{}\x1b\\",
            data[0] as u32,
            base64_encode(&data[1..data.len().min(8)])
        );
        terminal.process(frame.as_bytes());

        // Control animation
        let ctrl = format!("\x1b_Ga=a,i=100,s={}\x1b\\", data[0] % 4);
        terminal.process(ctrl.as_bytes());

        // Verify state
        let _ = terminal.kitty_graphics().image_count();
    }
});

/// Simple base64 encoder for generating test payloads.
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();
    let mut i = 0;

    while i < data.len() {
        let b0 = data[i];
        let b1 = data.get(i + 1).copied().unwrap_or(0);
        let b2 = data.get(i + 2).copied().unwrap_or(0);

        result.push(ALPHABET[(b0 >> 2) as usize] as char);
        result.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);

        if i + 1 < data.len() {
            result.push(ALPHABET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }

        if i + 2 < data.len() {
            result.push(ALPHABET[(b2 & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        i += 3;
    }

    result
}
