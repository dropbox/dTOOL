// © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// Tests for dashflow-cli output formatting

#[test]
fn test_format_duration_microseconds() {
    let duration = 500i64; // 500 microseconds
    let formatted = format_duration(duration);
    assert_eq!(formatted, "500μs");
}

#[test]
fn test_format_duration_milliseconds() {
    let duration = 1_500i64; // 1.5 milliseconds
    let formatted = format_duration(duration);
    assert_eq!(formatted, "1.50ms");
}

#[test]
fn test_format_duration_seconds() {
    let duration = 2_345_678i64; // ~2.35 seconds
    let formatted = format_duration(duration);
    assert_eq!(formatted, "2.35s");
}

#[test]
fn test_format_duration_minutes() {
    let duration = 125_000_000i64; // 125 seconds = 2m 5s
    let formatted = format_duration(duration);
    assert_eq!(formatted, "2m 5s");
}

#[test]
fn test_format_duration_exact_minute() {
    let duration = 60_000_000i64; // Exactly 1 minute
    let formatted = format_duration(duration);
    assert_eq!(formatted, "1m 0s");
}

#[test]
fn test_format_duration_zero() {
    let duration = 0i64;
    let formatted = format_duration(duration);
    assert_eq!(formatted, "0μs");
}

#[test]
fn test_format_duration_boundary_us_ms() {
    let duration = 999i64; // Just under 1ms
    let formatted = format_duration(duration);
    assert_eq!(formatted, "999μs");

    let duration = 1_000i64; // Exactly 1ms
    let formatted = format_duration(duration);
    assert_eq!(formatted, "1.00ms");
}

#[test]
fn test_format_duration_boundary_ms_s() {
    let duration = 999_999i64; // Just under 1s
    let formatted = format_duration(duration);
    assert_eq!(formatted, "1000.00ms");

    let duration = 1_000_000i64; // Exactly 1s
    let formatted = format_duration(duration);
    assert_eq!(formatted, "1.00s");
}

#[test]
fn test_format_bytes_bytes() {
    let size = 512usize;
    let formatted = format_bytes(size);
    assert_eq!(formatted, "512B");
}

#[test]
fn test_format_bytes_kilobytes() {
    let size = 2048usize; // 2KB
    let formatted = format_bytes(size);
    assert_eq!(formatted, "2.00KB");
}

#[test]
fn test_format_bytes_megabytes() {
    let size = 5_242_880usize; // 5MB
    let formatted = format_bytes(size);
    assert_eq!(formatted, "5.00MB");
}

#[test]
fn test_format_bytes_gigabytes() {
    let size = 2_147_483_648usize; // 2GB
    let formatted = format_bytes(size);
    assert_eq!(formatted, "2.00GB");
}

#[test]
fn test_format_bytes_zero() {
    let size = 0usize;
    let formatted = format_bytes(size);
    assert_eq!(formatted, "0B");
}

#[test]
fn test_format_bytes_boundary_b_kb() {
    let size = 1023usize; // Just under 1KB
    let formatted = format_bytes(size);
    assert_eq!(formatted, "1023B");

    let size = 1024usize; // Exactly 1KB
    let formatted = format_bytes(size);
    assert_eq!(formatted, "1.00KB");
}

#[test]
fn test_format_bytes_boundary_kb_mb() {
    let size = 1024 * 1024 - 1; // Just under 1MB
    let formatted = format_bytes(size);
    assert_eq!(formatted, "1024.00KB");

    let size = 1024 * 1024; // Exactly 1MB
    let formatted = format_bytes(size);
    assert_eq!(formatted, "1.00MB");
}

#[test]
fn test_format_timestamp() {
    // Test a known timestamp: 2024-01-01 00:00:00 UTC
    let timestamp_us = 1_704_067_200_000_000_i64; // 2024-01-01 00:00:00 in microseconds

    let formatted = format_timestamp(timestamp_us);

    // Should contain date and time components
    assert!(formatted.contains("2024"));
    assert!(formatted.contains("01"));
}

#[test]
fn test_format_timestamp_with_precision() {
    // Test timestamp with millisecond precision
    let timestamp_us = 1_704_067_200_123_456_i64; // With .123456 microseconds

    let formatted = format_timestamp(timestamp_us);

    // Should contain milliseconds (.123)
    assert!(formatted.contains(".123"));
}

#[test]
fn test_timestamp_conversion() {
    let micros = 1_234_567_890_123_456i64;
    let secs = micros / 1_000_000;
    let nanos = ((micros % 1_000_000) * 1000) as u32;

    assert_eq!(secs, 1_234_567_890);
    assert_eq!(nanos, 123_456_000);
}

#[test]
fn test_hex_encoding_message_id() {
    let message_id = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];
    let encoded = hex::encode(&message_id[..8]);
    assert_eq!(encoded, "0123456789abcdef");
}

#[test]
fn test_hex_encoding_short_id() {
    let message_id = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];
    let short_id = hex::encode(&message_id[..4]);
    assert_eq!(short_id, "01234567");
}

#[test]
fn test_color_formatting_preserved() {
    use colored::*;

    let text = "test".bright_green();
    let formatted = format!("{}", text);

    // The formatted string should contain the text
    assert!(formatted.contains("test"));
}

#[test]
fn test_table_creation() {
    use comfy_table::{presets::UTF8_FULL, Table};

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Column 1", "Column 2"]);

    // Table should be valid
    assert_eq!(table.column_count(), 2);
}

#[test]
fn test_duration_precision() {
    // Test that we maintain precision for milliseconds
    let duration = 1_234i64; // 1.234ms
    let formatted = format_duration(duration);
    assert_eq!(formatted, "1.23ms");

    let duration = 12_345i64; // 12.345ms
    let formatted = format_duration(duration);
    assert_eq!(formatted, "12.35ms");
}

#[test]
fn test_bytes_precision() {
    // Test that we maintain precision for KB
    let size = 1536usize; // 1.5KB
    let formatted = format_bytes(size);
    assert_eq!(formatted, "1.50KB");

    let size = 5_767_168usize; // 5.5MB
    let formatted = format_bytes(size);
    assert_eq!(formatted, "5.50MB");
}

// Helper functions (duplicated from src/output.rs for testing)

fn format_duration(micros: i64) -> String {
    if micros < 1_000 {
        format!("{}μs", micros)
    } else if micros < 1_000_000 {
        format!("{:.2}ms", micros as f64 / 1_000.0)
    } else if micros < 60_000_000 {
        format!("{:.2}s", micros as f64 / 1_000_000.0)
    } else {
        let minutes = micros / 60_000_000;
        let seconds = (micros % 60_000_000) / 1_000_000;
        format!("{}m {}s", minutes, seconds)
    }
}

fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_timestamp(micros: i64) -> String {
    use chrono::DateTime;

    let secs = micros / 1_000_000;
    let nanos = ((micros % 1_000_000) * 1000) as u32;

    if let Some(dt) = DateTime::from_timestamp(secs, nanos) {
        dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
    } else {
        format!("{}", micros)
    }
}
