//! Tests for Kitty graphics protocol implementation.

use super::command::*;
use super::storage::*;
use std::sync::Arc;

#[test]
fn test_action_from_byte_unknown_defaults_to_transmit() {
    assert_eq!(Action::from_byte(b'?'), Action::Transmit);
    assert_eq!(Action::from_byte(0), Action::Transmit);
}

#[test]
fn test_transmission_type_unknown_defaults_to_direct() {
    assert_eq!(TransmissionType::from_byte(b'?'), TransmissionType::Direct);
}

#[test]
fn test_image_format_unknown_defaults_to_rgba32() {
    assert_eq!(ImageFormat::from_value(0), ImageFormat::Rgba32);
    assert_eq!(ImageFormat::from_value(999), ImageFormat::Rgba32);
}

#[test]
fn test_delete_action_frees_data_lowercase() {
    // Lowercase delete actions do NOT free data
    assert!(!DeleteAction::AllVisiblePlacements.frees_data());
    assert!(!DeleteAction::ById.frees_data());
    assert!(!DeleteAction::ByNumber.frees_data());
    assert!(!DeleteAction::AtCursor.frees_data());
    assert!(!DeleteAction::AnimationFrames.frees_data());
    assert!(!DeleteAction::AtCell.frees_data());
    assert!(!DeleteAction::AtCellWithZ.frees_data());
    assert!(!DeleteAction::ByIdRange.frees_data());
    assert!(!DeleteAction::InColumn.frees_data());
    assert!(!DeleteAction::InRow.frees_data());
    assert!(!DeleteAction::ByZIndex.frees_data());
}

#[test]
fn test_delete_action_frees_data_uppercase() {
    // Uppercase delete actions DO free data
    assert!(DeleteAction::AllVisiblePlacementsAndData.frees_data());
    assert!(DeleteAction::ByIdAndData.frees_data());
    assert!(DeleteAction::ByNumberAndData.frees_data());
    assert!(DeleteAction::AtCursorAndData.frees_data());
    assert!(DeleteAction::AnimationFramesAndData.frees_data());
    assert!(DeleteAction::AtCellAndData.frees_data());
    assert!(DeleteAction::AtCellWithZAndData.frees_data());
    assert!(DeleteAction::ByIdRangeAndData.frees_data());
    assert!(DeleteAction::InColumnAndData.frees_data());
    assert!(DeleteAction::InRowAndData.frees_data());
    assert!(DeleteAction::ByZIndexAndData.frees_data());
}

#[test]
fn test_compression_type_to_byte() {
    assert_eq!(CompressionType::None.to_byte(), None);
    assert_eq!(CompressionType::Zlib.to_byte(), Some(b'z'));
}

#[test]
fn test_cursor_movement_roundtrip() {
    assert_eq!(
        CursorMovement::from_value(CursorMovement::Move.to_value()),
        CursorMovement::Move
    );
    assert_eq!(
        CursorMovement::from_value(CursorMovement::Stay.to_value()),
        CursorMovement::Stay
    );
}

#[test]
fn test_command_parse_with_malformed_input() {
    // Empty key
    let cmd = KittyGraphicsCommand::parse(b"=123");
    assert_eq!(cmd.image_id, 0); // Not parsed

    // Missing value
    let cmd = KittyGraphicsCommand::parse(b"i=");
    assert_eq!(cmd.image_id, 0); // Not parsed

    // Multiple equals signs
    let cmd = KittyGraphicsCommand::parse(b"i=1=2");
    assert_eq!(cmd.image_id, 1); // Parses up to first equals

    // Trailing comma
    let cmd = KittyGraphicsCommand::parse(b"i=123,");
    assert_eq!(cmd.image_id, 123);

    // Multiple commas
    let cmd = KittyGraphicsCommand::parse(b"i=1,,s=10");
    assert_eq!(cmd.image_id, 1);
    assert_eq!(cmd.data_width, 10);
}

#[test]
fn test_command_parse_numeric_overflow() {
    // Very large number - saturating
    let cmd = KittyGraphicsCommand::parse(b"i=99999999999999999999");
    assert_eq!(cmd.image_id, u32::MAX);
}

#[test]
fn test_command_parse_negative_z_large() {
    let cmd = KittyGraphicsCommand::parse(b"z=-2147483648");
    assert_eq!(cmd.z_index, i32::MIN);
}

#[test]
fn test_image_data_arc_sharing() {
    let data = vec![0u8; 1000];
    let image = KittyImage::new(1, 10, 100, data);
    let data_clone = Arc::clone(&image.data);

    assert_eq!(Arc::strong_count(&image.data), 2);
    assert_eq!(image.data_size(), 1000);
    drop(data_clone);
    assert_eq!(Arc::strong_count(&image.data), 1);
}

#[test]
fn test_storage_replace_image_same_id() {
    let mut storage = KittyImageStorage::new();

    let data1 = vec![0u8; 100];
    let image1 = KittyImage::new(42, 10, 10, data1);
    storage.store_image(image1).unwrap();
    assert_eq!(storage.total_bytes(), 100);

    let data2 = vec![0u8; 200];
    let image2 = KittyImage::new(42, 10, 20, data2);
    storage.store_image(image2).unwrap();

    assert_eq!(storage.image_count(), 1);
    assert_eq!(storage.total_bytes(), 200);
    assert_eq!(storage.get_image(42).unwrap().height, 20);
}

#[test]
fn test_storage_image_number_mapping() {
    let mut storage = KittyImageStorage::new();

    let data = vec![0u8; 100];
    let mut image = KittyImage::new(0, 10, 10, data);
    image.number = Some(999);

    let id = storage.store_image(image).unwrap();
    assert!(storage.get_image_by_number(999).is_some());
    assert_eq!(storage.get_image_by_number(999).unwrap().id, id);
}

#[test]
fn test_storage_clear() {
    let mut storage = KittyImageStorage::new();

    for _ in 0..10 {
        let data = vec![0u8; 100];
        let image = KittyImage::new(0, 10, 10, data);
        storage.store_image(image).unwrap();
    }

    assert_eq!(storage.image_count(), 10);
    storage.clear();
    assert_eq!(storage.image_count(), 0);
    assert_eq!(storage.total_bytes(), 0);
    assert!(!storage.is_loading());
}

#[test]
fn test_storage_dirty_flag() {
    let mut storage = KittyImageStorage::new();
    assert!(!storage.is_dirty());

    let data = vec![0u8; 100];
    let image = KittyImage::new(0, 10, 10, data);
    storage.store_image(image).unwrap();
    assert!(storage.is_dirty());

    storage.clear_dirty();
    assert!(!storage.is_dirty());

    storage.mark_dirty();
    assert!(storage.is_dirty());
}

#[test]
fn test_storage_delete_all() {
    let mut storage = KittyImageStorage::new();

    for _ in 0..5 {
        let data = vec![0u8; 100];
        let image = KittyImage::new(0, 10, 10, data);
        let id = storage.store_image(image).unwrap();
        storage
            .add_placement(id, KittyPlacement::new(0, 0, 0))
            .unwrap();
    }

    // Delete placements only
    let cmd = KittyGraphicsCommand::parse(b"a=d,d=a");
    storage.delete(&cmd);

    // Images still exist, placements gone
    assert_eq!(storage.image_count(), 5);
    for img in storage.iter_images() {
        assert!(!img.has_placements());
    }

    // Delete placements AND data
    let cmd = KittyGraphicsCommand::parse(b"a=d,d=A");
    storage.delete(&cmd);
    assert_eq!(storage.image_count(), 0);
}

#[test]
fn test_storage_delete_by_z_index() {
    let mut storage = KittyImageStorage::new();

    let data = vec![0u8; 100];
    let image = KittyImage::new(1, 10, 10, data);
    storage.store_image(image).unwrap();

    let mut p1 = KittyPlacement::new(1, 0, 0);
    p1.z_index = 5;
    let mut p2 = KittyPlacement::new(2, 0, 0);
    p2.z_index = -5;
    let mut p3 = KittyPlacement::new(3, 0, 0);
    p3.z_index = 5;

    storage.add_placement(1, p1).unwrap();
    storage.add_placement(1, p2).unwrap();
    storage.add_placement(1, p3).unwrap();

    let cmd = KittyGraphicsCommand::parse(b"a=d,d=z,z=5");
    storage.delete(&cmd);

    let image = storage.get_image(1).unwrap();
    assert_eq!(image.placement_count(), 1);
    assert!(image.get_placement(2).is_some());
}

#[test]
fn test_storage_delete_by_id_range() {
    let mut storage = KittyImageStorage::new();

    for i in 10..=20 {
        let data = vec![0u8; 10];
        let image = KittyImage::new(i, 1, 1, data);
        storage.store_image(image).unwrap();
    }

    // Delete IDs 12-18
    let cmd = KittyGraphicsCommand::parse(b"a=d,d=R,x=12,y=18");
    storage.delete(&cmd);

    // IDs 10, 11, 19, 20 should remain
    assert_eq!(storage.image_count(), 4);
    assert!(storage.get_image(10).is_some());
    assert!(storage.get_image(11).is_some());
    assert!(storage.get_image(19).is_some());
    assert!(storage.get_image(20).is_some());
    assert!(storage.get_image(15).is_none());
}

#[test]
fn test_storage_chunked_cancel() {
    let mut storage = KittyImageStorage::new();
    let cmd = KittyGraphicsCommand::parse(b"a=t,m=1");

    storage.start_chunked(cmd).unwrap();
    assert!(storage.is_loading());

    storage.cancel_chunked();
    assert!(!storage.is_loading());
}

#[test]
fn test_storage_chunked_error_on_double_start() {
    let mut storage = KittyImageStorage::new();
    let cmd = KittyGraphicsCommand::parse(b"a=t,m=1");

    storage.start_chunked(cmd.clone()).unwrap();
    let result = storage.start_chunked(cmd);
    assert!(matches!(
        result,
        Err(KittyGraphicsError::ChunkedTransmissionInProgress)
    ));
}

#[test]
fn test_storage_continue_without_start() {
    let mut storage = KittyImageStorage::new();
    let result = storage.continue_chunked(b"data", false);
    assert!(matches!(
        result,
        Err(KittyGraphicsError::NoChunkedTransmission)
    ));
}

#[test]
fn test_base64_decode_standard_padding() {
    // Standard padding
    assert_eq!(decode_base64(b"YQ==").unwrap(), b"a");
    assert_eq!(decode_base64(b"YWI=").unwrap(), b"ab");
    assert_eq!(decode_base64(b"YWJj").unwrap(), b"abc");
}

#[test]
fn test_base64_decode_whitespace() {
    // Whitespace should be ignored
    assert_eq!(decode_base64(b"YWJj\n").unwrap(), b"abc");
    assert_eq!(decode_base64(b"YW Jj").unwrap(), b"abc");
    assert_eq!(decode_base64(b"YW\tJj").unwrap(), b"abc");
}

#[test]
fn test_base64_decode_special_chars() {
    // '+' and '/' characters
    let decoded = decode_base64(b"+/==").unwrap();
    assert!(!decoded.is_empty());
}

#[test]
fn test_rgb_to_rgba_empty() {
    assert!(rgb_to_rgba(&[]).is_empty());
}

#[test]
fn test_rgb_to_rgba_single_pixel() {
    let rgb = vec![128, 64, 32];
    let rgba = rgb_to_rgba(&rgb);
    assert_eq!(rgba, vec![128, 64, 32, 255]);
}

#[test]
fn test_placement_absolute_position() {
    let placement = KittyPlacement::new(1, 100, 50);
    assert_eq!(placement.absolute_position(), Some((100, 50)));
}

#[test]
fn test_placement_virtual_no_absolute_position() {
    let cmd = KittyGraphicsCommand::parse(b"U=1,p=1");
    let placement = KittyPlacement::from_command(&cmd, 100, 50);
    assert!(placement.absolute_position().is_none());
}

#[test]
fn test_placement_relative_no_absolute_position() {
    let cmd = KittyGraphicsCommand::parse(b"P=42,Q=1");
    let placement = KittyPlacement::from_command(&cmd, 100, 50);
    assert!(placement.absolute_position().is_none());
}

#[test]
fn test_image_placement_management() {
    let data = vec![0u8; 100];
    let mut image = KittyImage::new(1, 10, 10, data);

    // Auto-assign IDs
    let p1 = KittyPlacement::new(0, 0, 0);
    let p2 = KittyPlacement::new(0, 0, 0);
    let id1 = image.add_placement(p1);
    let id2 = image.add_placement(p2);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert!(image.has_placements());

    // Remove one
    image.remove_placement(id1);
    assert!(image.get_placement(id1).is_none());
    assert!(image.get_placement(id2).is_some());

    // Clear all
    image.clear_placements();
    assert!(!image.has_placements());
}

#[test]
fn test_error_display() {
    let errors = [
        KittyGraphicsError::ImageNotFound(42),
        KittyGraphicsError::PlacementNotFound(1, 2),
        KittyGraphicsError::DimensionsTooLarge {
            width: 20000,
            height: 30000,
            max_width: 10000,
            max_height: 10000,
        },
        KittyGraphicsError::StorageQuotaExceeded,
        KittyGraphicsError::TooManyImages,
        KittyGraphicsError::InvalidImageData("test".to_string()),
        KittyGraphicsError::Base64DecodeError,
        KittyGraphicsError::CompressionError("zlib".to_string()),
        KittyGraphicsError::FileAccessError("file.png".to_string()),
        KittyGraphicsError::ParentNotFound(1, 2),
        KittyGraphicsError::PlacementChainTooDeep,
        KittyGraphicsError::ChunkedTransmissionInProgress,
        KittyGraphicsError::NoChunkedTransmission,
    ];

    for error in &errors {
        // Just ensure Display trait works without panic
        let _ = format!("{}", error);
    }
}

#[test]
fn test_iter_placements() {
    let mut storage = KittyImageStorage::new();

    // Create two images with placements
    let data1 = vec![0u8; 100];
    let image1 = KittyImage::new(1, 10, 10, data1);
    storage.store_image(image1).unwrap();
    storage
        .add_placement(1, KittyPlacement::new(0, 0, 0))
        .unwrap();
    storage
        .add_placement(1, KittyPlacement::new(0, 1, 1))
        .unwrap();

    let data2 = vec![0u8; 100];
    let image2 = KittyImage::new(2, 10, 10, data2);
    storage.store_image(image2).unwrap();
    storage
        .add_placement(2, KittyPlacement::new(0, 2, 2))
        .unwrap();

    let placements: Vec<_> = storage.iter_placements().collect();
    assert_eq!(placements.len(), 3);
}

#[test]
fn test_delete_at_position() {
    let mut storage = KittyImageStorage::new();

    let data = vec![0u8; 100];
    let image = KittyImage::new(1, 10, 10, data);
    storage.store_image(image).unwrap();

    storage
        .add_placement(1, KittyPlacement::new(1, 5, 10))
        .unwrap();
    storage
        .add_placement(1, KittyPlacement::new(2, 5, 10))
        .unwrap();
    storage
        .add_placement(1, KittyPlacement::new(3, 5, 20))
        .unwrap();

    storage.delete_at_position(5, 10, false);

    let image = storage.get_image(1).unwrap();
    assert_eq!(image.placement_count(), 1);
    assert!(image.get_placement(3).is_some());
}

#[test]
fn test_command_all_key_values() {
    // Test parsing all supported keys
    let cmd = KittyGraphicsCommand::parse(
        b"a=T,t=f,o=z,f=24,m=1,i=123,I=456,p=789,q=1,s=100,v=200,S=1024,O=512,\
          w=50,h=60,x=10,y=20,X=5,Y=6,c=4,r=3,z=-10,C=1,d=I,U=1,P=42,Q=7,H=-5,V=3",
    );

    assert_eq!(cmd.action, Action::TransmitAndDisplay);
    assert_eq!(cmd.transmission_type, TransmissionType::File);
    assert_eq!(cmd.compression, CompressionType::Zlib);
    assert_eq!(cmd.format, ImageFormat::Rgb24);
    assert!(cmd.more);
    assert_eq!(cmd.image_id, 123);
    assert_eq!(cmd.image_number, 456);
    assert_eq!(cmd.placement_id, 789);
    assert_eq!(cmd.quiet, 1);
    assert_eq!(cmd.data_width, 100);
    assert_eq!(cmd.data_height, 200);
    assert_eq!(cmd.data_size, 1024);
    assert_eq!(cmd.data_offset, 512);
    assert_eq!(cmd.source_width, 50);
    assert_eq!(cmd.source_height, 60);
    assert_eq!(cmd.source_x, 10);
    assert_eq!(cmd.source_y, 20);
    assert_eq!(cmd.cell_x_offset, 5);
    assert_eq!(cmd.cell_y_offset, 6);
    assert_eq!(cmd.num_columns, 4);
    assert_eq!(cmd.num_rows, 3);
    assert_eq!(cmd.z_index, -10);
    assert_eq!(cmd.cursor_movement, CursorMovement::Stay);
    assert_eq!(cmd.delete_action, DeleteAction::ByIdAndData);
    assert!(cmd.unicode_placement);
    assert_eq!(cmd.parent_id, 42);
    assert_eq!(cmd.parent_placement_id, 7);
    assert_eq!(cmd.offset_from_parent_x, -5);
    assert_eq!(cmd.offset_from_parent_y, 3);
}

// === Animation Tests ===

#[test]
fn test_animation_state_roundtrip() {
    assert_eq!(
        AnimationState::from_value(AnimationState::Stopped.to_value()),
        AnimationState::Stopped
    );
    assert_eq!(
        AnimationState::from_value(AnimationState::Loading.to_value()),
        AnimationState::Loading
    );
    assert_eq!(
        AnimationState::from_value(AnimationState::Running.to_value()),
        AnimationState::Running
    );
}

#[test]
fn test_animation_state_from_value() {
    assert_eq!(AnimationState::from_value(1), AnimationState::Stopped);
    assert_eq!(AnimationState::from_value(2), AnimationState::Loading);
    assert_eq!(AnimationState::from_value(3), AnimationState::Running);
    assert_eq!(AnimationState::from_value(0), AnimationState::Stopped); // Default
    assert_eq!(AnimationState::from_value(99), AnimationState::Stopped); // Default
}

#[test]
fn test_composition_mode_roundtrip() {
    assert_eq!(
        CompositionMode::from_value(CompositionMode::AlphaBlend.to_value()),
        CompositionMode::AlphaBlend
    );
    assert_eq!(
        CompositionMode::from_value(CompositionMode::Overwrite.to_value()),
        CompositionMode::Overwrite
    );
}

#[test]
fn test_composition_mode_from_value() {
    assert_eq!(CompositionMode::from_value(0), CompositionMode::AlphaBlend);
    assert_eq!(CompositionMode::from_value(1), CompositionMode::Overwrite);
    assert_eq!(CompositionMode::from_value(99), CompositionMode::AlphaBlend); // Default
}

#[test]
fn test_command_parse_animation_frame() {
    // a=f for transmit animation frame
    let cmd = KittyGraphicsCommand::parse(b"a=f,i=123,z=100,b=1");
    assert_eq!(cmd.action, Action::TransmitAnimationFrame);
    assert_eq!(cmd.image_id, 123);
    assert_eq!(cmd.frame_gap, 100);
    assert_eq!(cmd.base_frame, 1);
    assert!(cmd.expects_payload());
}

#[test]
fn test_command_parse_animation_control() {
    // a=a for control animation
    let cmd = KittyGraphicsCommand::parse(b"a=a,i=123,s=3,v=5");
    assert_eq!(cmd.action, Action::ControlAnimation);
    assert_eq!(cmd.image_id, 123);
    assert_eq!(cmd.animation_state, AnimationState::Running);
    assert_eq!(cmd.loop_count, 5);
}

#[test]
fn test_command_parse_animation_compose() {
    // a=c for compose animation frames
    let cmd = KittyGraphicsCommand::parse(b"a=c,i=123,r=1,c=2,C=1");
    assert_eq!(cmd.action, Action::ComposeAnimation);
    assert_eq!(cmd.image_id, 123);
    assert_eq!(cmd.source_frame, 1);
    assert_eq!(cmd.dest_frame, 2);
    assert_eq!(cmd.composition_mode, CompositionMode::Overwrite);
}

#[test]
fn test_animation_frame_new() {
    let data = vec![0u8; 400]; // 10x10 RGBA
    let frame = AnimationFrame::new(1, data, 10, 10);

    assert_eq!(frame.number, 1);
    assert_eq!(frame.width, 10);
    assert_eq!(frame.height, 10);
    assert_eq!(frame.data_size(), 400);
    assert_eq!(frame.x_offset, 0);
    assert_eq!(frame.y_offset, 0);
    assert_eq!(frame.gap, 0);
    assert_eq!(frame.base_frame, 0);
    assert_eq!(frame.composition_mode, CompositionMode::AlphaBlend);
}

#[test]
fn test_image_add_frame() {
    let data = vec![0u8; 400];
    let mut image = KittyImage::new(1, 10, 10, data);

    assert!(!image.is_animated());
    assert_eq!(image.frame_count(), 0);

    let frame_data = vec![0u8; 400];
    let frame = AnimationFrame::new(0, frame_data, 10, 10); // 0 = auto-assign
    let num = image.add_frame(frame).unwrap();

    assert_eq!(num, 1);
    assert!(image.is_animated());
    assert_eq!(image.frame_count(), 1);
}

#[test]
fn test_image_get_frame() {
    let data = vec![0u8; 400];
    let mut image = KittyImage::new(1, 10, 10, data);

    let frame_data = vec![1u8; 400]; // Different data
    let mut frame = AnimationFrame::new(0, frame_data, 10, 10);
    frame.gap = 100;
    image.add_frame(frame).unwrap();

    // Frame 0 is root (returns None)
    assert!(image.get_frame(0).is_none());

    // Frame 1 exists
    let f = image.get_frame(1).unwrap();
    assert_eq!(f.gap, 100);
    assert_eq!(f.data[0], 1);
}

#[test]
fn test_image_remove_frame() {
    let data = vec![0u8; 400];
    let mut image = KittyImage::new(1, 10, 10, data);

    let frame_data = vec![0u8; 400];
    let frame = AnimationFrame::new(0, frame_data, 10, 10);
    image.add_frame(frame).unwrap();

    assert_eq!(image.frame_count(), 1);

    let removed = image.remove_frame(1);
    assert!(removed.is_some());
    assert_eq!(image.frame_count(), 0);

    // Removing non-existent frame
    let removed = image.remove_frame(99);
    assert!(removed.is_none());
}

#[test]
fn test_image_clear_frames() {
    let data = vec![0u8; 400];
    let mut image = KittyImage::new(1, 10, 10, data);

    for _ in 0..5 {
        let frame_data = vec![0u8; 400];
        let frame = AnimationFrame::new(0, frame_data, 10, 10);
        image.add_frame(frame).unwrap();
    }

    image.animation_state = AnimationState::Running;
    image.current_frame_index = 3;

    image.clear_frames();

    assert_eq!(image.frame_count(), 0);
    assert_eq!(image.animation_state, AnimationState::Stopped);
    assert_eq!(image.current_frame_index, 0);
}

#[test]
fn test_image_data_size_includes_frames() {
    let data = vec![0u8; 400];
    let mut image = KittyImage::new(1, 10, 10, data);
    assert_eq!(image.data_size(), 400);

    let frame_data = vec![0u8; 200];
    let frame = AnimationFrame::new(0, frame_data, 10, 5);
    image.add_frame(frame).unwrap();

    assert_eq!(image.data_size(), 600); // 400 + 200
}

#[test]
fn test_image_current_frame_data() {
    let root_data = vec![1u8; 400];
    let mut image = KittyImage::new(1, 10, 10, root_data);

    // Without frames, returns root data
    assert_eq!(image.current_frame_data()[0], 1);

    let frame_data = vec![2u8; 400];
    let frame = AnimationFrame::new(0, frame_data, 10, 10);
    image.add_frame(frame).unwrap();

    // At frame 0, still root
    assert_eq!(image.current_frame_data()[0], 1);

    // Move to frame 1
    image.current_frame_index = 1;
    assert_eq!(image.current_frame_data()[0], 2);
}

#[test]
fn test_image_advance_frame() {
    let data = vec![0u8; 400];
    let mut image = KittyImage::new(1, 10, 10, data);

    // Add 3 frames
    for _ in 0..3 {
        let frame_data = vec![0u8; 400];
        let frame = AnimationFrame::new(0, frame_data, 10, 10);
        image.add_frame(frame).unwrap();
    }

    // Not running - should not advance
    assert!(!image.advance_frame());

    // Start animation with infinite loop
    image.animation_state = AnimationState::Running;
    image.max_loops = 1;
    image.current_frame_index = 0;

    assert!(image.advance_frame());
    assert_eq!(image.current_frame_index, 1);

    assert!(image.advance_frame());
    assert_eq!(image.current_frame_index, 2);

    assert!(image.advance_frame());
    assert_eq!(image.current_frame_index, 3);

    // Should loop back to 0
    assert!(image.advance_frame());
    assert_eq!(image.current_frame_index, 0);
}

#[test]
fn test_image_advance_frame_limited_loops() {
    let data = vec![0u8; 400];
    let mut image = KittyImage::new(1, 10, 10, data);

    // Add 2 frames
    for _ in 0..2 {
        let frame_data = vec![0u8; 400];
        let frame = AnimationFrame::new(0, frame_data, 10, 10);
        image.add_frame(frame).unwrap();
    }

    image.animation_state = AnimationState::Running;
    image.max_loops = 2; // Loop once (2-1=1 times)

    // Play through first time: 0 -> 1 -> 2 (end of frames)
    assert!(image.advance_frame()); // 0 -> 1
    assert!(image.advance_frame()); // 1 -> 2

    // Should loop back (first loop)
    assert!(image.advance_frame()); // 2 -> 0 (loop)
    assert_eq!(image.current_frame_index, 0);
    assert_eq!(image.current_loop, 1);

    // Play through second time
    assert!(image.advance_frame()); // 0 -> 1
    assert!(image.advance_frame()); // 1 -> 2

    // Should stop (second loop would exceed)
    assert!(!image.advance_frame());
    assert_eq!(image.animation_state, AnimationState::Stopped);
}

#[test]
fn test_image_reset_animation() {
    let data = vec![0u8; 400];
    let mut image = KittyImage::new(1, 10, 10, data);

    image.current_frame_index = 5;
    image.current_loop = 3;

    image.reset_animation();

    assert_eq!(image.current_frame_index, 0);
    assert_eq!(image.current_loop, 0);
}

#[test]
fn test_image_current_frame_gap() {
    let data = vec![0u8; 400];
    let mut image = KittyImage::new(1, 10, 10, data);

    // No frames - gap is 0
    assert_eq!(image.current_frame_gap(), 0);

    let frame_data = vec![0u8; 400];
    let mut frame = AnimationFrame::new(0, frame_data, 10, 10);
    frame.gap = 100;
    image.add_frame(frame).unwrap();

    // At root frame - gap is 0
    assert_eq!(image.current_frame_gap(), 0);

    // At frame 1
    image.current_frame_index = 1;
    assert_eq!(image.current_frame_gap(), 100);
}

#[test]
fn test_storage_add_frame() {
    let mut storage = KittyImageStorage::new();

    let data = vec![0u8; 400];
    let image = KittyImage::new(1, 10, 10, data);
    storage.store_image(image).unwrap();

    let frame_data = vec![0u8; 400];
    let frame = AnimationFrame::new(0, frame_data, 10, 10);
    let num = storage.add_frame(1, frame).unwrap();

    assert_eq!(num, 1);
    assert_eq!(storage.total_bytes(), 800); // Root + frame
    assert!(storage.is_dirty());

    let img = storage.get_image(1).unwrap();
    assert!(img.is_animated());
}

#[test]
fn test_storage_add_frame_image_not_found() {
    let mut storage = KittyImageStorage::new();

    let frame_data = vec![0u8; 400];
    let frame = AnimationFrame::new(0, frame_data, 10, 10);
    let result = storage.add_frame(99, frame);

    assert!(matches!(result, Err(KittyGraphicsError::ImageNotFound(99))));
}

#[test]
fn test_storage_add_frame_quota_exceeded() {
    let mut storage = KittyImageStorage::with_quota(500);

    let data = vec![0u8; 400];
    let image = KittyImage::new(1, 10, 10, data);
    storage.store_image(image).unwrap();

    let frame_data = vec![0u8; 200]; // Would exceed 500 quota
    let frame = AnimationFrame::new(0, frame_data, 10, 5);
    let result = storage.add_frame(1, frame);

    assert!(matches!(
        result,
        Err(KittyGraphicsError::StorageQuotaExceeded)
    ));
}

#[test]
fn test_storage_control_animation() {
    let mut storage = KittyImageStorage::new();

    let data = vec![0u8; 400];
    let image = KittyImage::new(1, 10, 10, data);
    storage.store_image(image).unwrap();

    // Add some frames
    for _ in 0..3 {
        let frame_data = vec![0u8; 400];
        let frame = AnimationFrame::new(0, frame_data, 10, 10);
        storage.add_frame(1, frame).unwrap();
    }

    // Start animation
    storage
        .control_animation(1, AnimationState::Running, Some(5))
        .unwrap();

    let img = storage.get_image(1).unwrap();
    assert_eq!(img.animation_state, AnimationState::Running);
    assert_eq!(img.max_loops, 5);
    assert_eq!(img.current_frame_index, 0); // Reset on start
}

#[test]
fn test_storage_control_animation_not_found() {
    let mut storage = KittyImageStorage::new();
    let result = storage.control_animation(99, AnimationState::Running, None);
    assert!(matches!(result, Err(KittyGraphicsError::ImageNotFound(99))));
}

#[test]
fn test_storage_delete_frames() {
    let mut storage = KittyImageStorage::new();

    let data = vec![0u8; 400];
    let image = KittyImage::new(1, 10, 10, data);
    storage.store_image(image).unwrap();

    // Add frames
    for _ in 0..3 {
        let frame_data = vec![0u8; 400];
        let frame = AnimationFrame::new(0, frame_data, 10, 10);
        storage.add_frame(1, frame).unwrap();
    }

    assert_eq!(storage.total_bytes(), 1600); // 400 + 3*400

    storage.delete_frames(1, false);

    assert_eq!(storage.total_bytes(), 400); // Only root remains
    let img = storage.get_image(1).unwrap();
    assert!(!img.is_animated());
}

#[test]
fn test_storage_animated_images() {
    let mut storage = KittyImageStorage::new();

    // Image 1: animated and running
    let data1 = vec![0u8; 400];
    let image1 = KittyImage::new(1, 10, 10, data1);
    storage.store_image(image1).unwrap();
    let frame = AnimationFrame::new(0, vec![0u8; 400], 10, 10);
    storage.add_frame(1, frame).unwrap();
    storage
        .control_animation(1, AnimationState::Running, None)
        .unwrap();

    // Image 2: animated but stopped
    let data2 = vec![0u8; 400];
    let image2 = KittyImage::new(2, 10, 10, data2);
    storage.store_image(image2).unwrap();
    let frame = AnimationFrame::new(0, vec![0u8; 400], 10, 10);
    storage.add_frame(2, frame).unwrap();

    // Image 3: not animated
    let data3 = vec![0u8; 400];
    let image3 = KittyImage::new(3, 10, 10, data3);
    storage.store_image(image3).unwrap();

    let animated = storage.animated_images();
    assert_eq!(animated.len(), 1);
    assert!(animated.contains(&1));
}

#[test]
fn test_storage_advance_animations() {
    let mut storage = KittyImageStorage::new();

    let data = vec![0u8; 400];
    let image = KittyImage::new(1, 10, 10, data);
    storage.store_image(image).unwrap();

    // Add frames
    for _ in 0..3 {
        let frame = AnimationFrame::new(0, vec![0u8; 400], 10, 10);
        storage.add_frame(1, frame).unwrap();
    }

    // Not running - should not advance
    let advanced = storage.advance_animations();
    assert!(advanced.is_empty());

    // Start with infinite loop
    storage
        .control_animation(1, AnimationState::Running, Some(1))
        .unwrap();

    let advanced = storage.advance_animations();
    assert_eq!(advanced.len(), 1);
    assert!(advanced.contains(&1));

    let img = storage.get_image(1).unwrap();
    assert_eq!(img.current_frame_index, 1);
}

#[test]
fn test_too_many_frames_error() {
    let data = vec![0u8; 100];
    let mut image = KittyImage::new(1, 10, 10, data);

    // This test would be slow with actual MAX_FRAMES_PER_IMAGE (1000)
    // So we verify the logic with a smaller number
    for _ in 0..MAX_FRAMES_PER_IMAGE {
        let frame = AnimationFrame::new(0, vec![0u8; 4], 1, 1);
        image.add_frame(frame).unwrap();
    }

    // One more should fail
    let frame = AnimationFrame::new(0, vec![0u8; 4], 1, 1);
    let result = image.add_frame(frame);
    assert!(matches!(result, Err(KittyGraphicsError::TooManyFrames)));
}

#[test]
fn test_error_display_animation() {
    let errors = [
        KittyGraphicsError::FrameNotFound(42),
        KittyGraphicsError::TooManyFrames,
    ];

    for error in &errors {
        let msg = format!("{}", error);
        assert!(!msg.is_empty());
    }
}

// === Frame Composition Tests ===

#[test]
fn test_compose_frames_overwrite_root_to_root() {
    // Create 2x2 image with red pixels
    let mut root_data = vec![0u8; 16]; // 2x2 RGBA
    // Pixel (0,0) = red, fully opaque
    root_data[0..4].copy_from_slice(&[255, 0, 0, 255]);
    // Other pixels = blue
    for i in 1..4 {
        root_data[i * 4..(i + 1) * 4].copy_from_slice(&[0, 0, 255, 255]);
    }

    let mut image = KittyImage::new(1, 2, 2, root_data);

    // Compose root onto itself should be no-op (but technically works)
    // More useful: compose frame with offset onto root
    assert!(image.compose_frames(0, 0, CompositionMode::Overwrite, 0).is_ok());
}

#[test]
fn test_compose_frames_overwrite_frame_to_root() {
    // Create 2x2 root (blue)
    let root_data = vec![0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255];
    let mut image = KittyImage::new(1, 2, 2, root_data);

    // Create 1x1 frame (red) to overlay at origin
    let frame_data = vec![255, 0, 0, 255];
    let frame = AnimationFrame::new(1, frame_data, 1, 1);
    image.add_frame(frame).unwrap();

    // Compose frame 1 onto root (frame 0)
    image
        .compose_frames(1, 0, CompositionMode::Overwrite, 0)
        .unwrap();

    // Top-left pixel should now be red
    assert_eq!(image.data[0], 255); // R
    assert_eq!(image.data[1], 0); // G
    assert_eq!(image.data[2], 0); // B
    assert_eq!(image.data[3], 255); // A

    // Other pixels should still be blue
    assert_eq!(image.data[4], 0); // R of pixel (1,0)
    assert_eq!(image.data[5], 0); // G
    assert_eq!(image.data[6], 255); // B
}

#[test]
fn test_compose_frames_alpha_blend() {
    // Create 2x2 root (white)
    let root_data = vec![
        255, 255, 255, 255, // white
        255, 255, 255, 255, // white
        255, 255, 255, 255, // white
        255, 255, 255, 255, // white
    ];
    let mut image = KittyImage::new(1, 2, 2, root_data);

    // Create 1x1 frame (red, 50% transparent) at origin
    let frame_data = vec![255, 0, 0, 128]; // Red with ~50% alpha
    let frame = AnimationFrame::new(1, frame_data, 1, 1);
    image.add_frame(frame).unwrap();

    // Compose frame 1 onto root with alpha blend
    image
        .compose_frames(1, 0, CompositionMode::AlphaBlend, 0)
        .unwrap();

    // Top-left should be blended (white + 50% red = pinkish)
    // Red channel: 255 * 0.5 + 255 * 0.5 ≈ 255
    // Green channel: 0 * 0.5 + 255 * 0.5 ≈ 127
    // Blue channel: 0 * 0.5 + 255 * 0.5 ≈ 127
    let r = image.data[0];
    let g = image.data[1];
    let b = image.data[2];

    // Allow some rounding tolerance
    assert!(r > 250, "Expected R > 250, got {}", r);
    assert!(g > 100 && g < 140, "Expected G ~127, got {}", g);
    assert!(b > 100 && b < 140, "Expected B ~127, got {}", b);
}

#[test]
fn test_compose_frames_fully_transparent() {
    // Create 2x2 root (green)
    let root_data = vec![
        0, 255, 0, 255, // green
        0, 255, 0, 255, // green
        0, 255, 0, 255, // green
        0, 255, 0, 255, // green
    ];
    let mut image = KittyImage::new(1, 2, 2, root_data);

    // Create 1x1 frame (red, fully transparent)
    let frame_data = vec![255, 0, 0, 0]; // Red but alpha = 0
    let frame = AnimationFrame::new(1, frame_data, 1, 1);
    image.add_frame(frame).unwrap();

    // Compose with alpha blend - should leave dest unchanged
    image
        .compose_frames(1, 0, CompositionMode::AlphaBlend, 0)
        .unwrap();

    // Should still be green (transparent source has no effect)
    assert_eq!(image.data[0], 0); // R
    assert_eq!(image.data[1], 255); // G
    assert_eq!(image.data[2], 0); // B
}

#[test]
fn test_compose_frames_overwrite_transparent_uses_background() {
    // Create 2x2 root (blue)
    let root_data = vec![
        0, 0, 255, 255, // blue
        0, 0, 255, 255, // blue
        0, 0, 255, 255, // blue
        0, 0, 255, 255, // blue
    ];
    let mut image = KittyImage::new(1, 2, 2, root_data);

    // Create 1x1 frame (fully transparent)
    let frame_data = vec![0, 0, 0, 0]; // Fully transparent
    let frame = AnimationFrame::new(1, frame_data, 1, 1);
    image.add_frame(frame).unwrap();

    // Compose with overwrite mode and red background
    // Background color is RGBA packed as 0xRRGGBBAA
    let bg_color: u32 = 0xFF0000FF; // Red, fully opaque
    image
        .compose_frames(1, 0, CompositionMode::Overwrite, bg_color)
        .unwrap();

    // Top-left should now be red (background color fills transparent)
    assert_eq!(image.data[0], 255); // R
    assert_eq!(image.data[1], 0); // G
    assert_eq!(image.data[2], 0); // B
    assert_eq!(image.data[3], 255); // A
}

#[test]
fn test_compose_frames_with_offset() {
    // Create 3x3 root (all black)
    let root_data = vec![0u8; 36]; // 3x3 * 4 = 36 bytes
    let mut image = KittyImage::new(1, 3, 3, root_data);

    // Create 1x1 frame (white) with offset (1, 1)
    let frame_data = vec![255, 255, 255, 255];
    let mut frame = AnimationFrame::new(1, frame_data, 1, 1);
    frame.x_offset = 1;
    frame.y_offset = 1;
    image.add_frame(frame).unwrap();

    // Compose frame 1 onto root
    image
        .compose_frames(1, 0, CompositionMode::Overwrite, 0)
        .unwrap();

    // Only pixel at (1,1) should be white
    // Index: (1 * 3 + 1) * 4 = 16
    assert_eq!(image.data[16], 255); // R
    assert_eq!(image.data[17], 255); // G
    assert_eq!(image.data[18], 255); // B
    assert_eq!(image.data[19], 255); // A

    // Pixel at (0,0) should still be black (index 0)
    assert_eq!(image.data[0], 0);
}

#[test]
fn test_compose_frames_frame_not_found() {
    let root_data = vec![0u8; 16];
    let mut image = KittyImage::new(1, 2, 2, root_data);

    // Try to compose non-existent frame
    let result = image.compose_frames(99, 0, CompositionMode::Overwrite, 0);
    assert!(matches!(result, Err(KittyGraphicsError::FrameNotFound(99))));

    // Try to compose onto non-existent destination frame
    let result = image.compose_frames(0, 99, CompositionMode::Overwrite, 0);
    assert!(matches!(result, Err(KittyGraphicsError::FrameNotFound(99))));
}

#[test]
fn test_compose_frames_frame_to_frame() {
    // Create 2x2 root (black)
    let root_data = vec![0u8; 16];
    let mut image = KittyImage::new(1, 2, 2, root_data);

    // Create frame 1 (red)
    let frame1_data = vec![255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255];
    let frame1 = AnimationFrame::new(1, frame1_data, 2, 2);
    image.add_frame(frame1).unwrap();

    // Create frame 2 (blue)
    let frame2_data = vec![0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255];
    let frame2 = AnimationFrame::new(2, frame2_data, 2, 2);
    image.add_frame(frame2).unwrap();

    // Compose frame 1 (red) onto frame 2 (blue) with overwrite
    image
        .compose_frames(1, 2, CompositionMode::Overwrite, 0)
        .unwrap();

    // Frame 2 should now be red
    let frame = image.get_frame(2).unwrap();
    assert_eq!(frame.data[0], 255); // R
    assert_eq!(frame.data[1], 0); // G
    assert_eq!(frame.data[2], 0); // B
}
