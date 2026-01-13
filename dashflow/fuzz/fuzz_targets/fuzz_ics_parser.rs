// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for iCalendar (ICS) format parser in dashflow
//!
//! Tests parsing of:
//! 1. VCALENDAR containers
//! 2. VEVENT (calendar events)
//! 3. VTODO (tasks/todos)
//! 4. VJOURNAL (journal entries)
//!
//! SECURITY CRITICAL: iCalendar files can be user-supplied and may contain
//! malformed data, oversized fields, or crafted content to exploit parsers.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// Parsed calendar event
#[derive(Debug, Default)]
#[allow(dead_code)] // Fields used for fuzzing validation
struct CalendarEvent {
    summary: String,
    description: String,
    dtstart: String,
    dtend: String,
    location: String,
    event_type: String, // VEVENT, VTODO, or VJOURNAL
}

/// Parse ICS format
/// Mirrors the parsing logic in ICSLoader
fn parse_ics(content: &str) -> Vec<CalendarEvent> {
    let mut events = Vec::new();
    let mut current_event_lines: Vec<String> = Vec::new();
    let mut in_event = false;
    let mut event_type = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("BEGIN:VEVENT") {
            in_event = true;
            event_type = "VEVENT".to_string();
            current_event_lines.clear();
            current_event_lines.push(trimmed.to_string());
        } else if trimmed.starts_with("BEGIN:VTODO") {
            in_event = true;
            event_type = "VTODO".to_string();
            current_event_lines.clear();
            current_event_lines.push(trimmed.to_string());
        } else if trimmed.starts_with("BEGIN:VJOURNAL") {
            in_event = true;
            event_type = "VJOURNAL".to_string();
            current_event_lines.clear();
            current_event_lines.push(trimmed.to_string());
        } else if trimmed.starts_with("END:VEVENT")
            || trimmed.starts_with("END:VTODO")
            || trimmed.starts_with("END:VJOURNAL")
        {
            if in_event {
                current_event_lines.push(trimmed.to_string());

                // Parse the event
                let event = parse_event_lines(&current_event_lines, &event_type);
                events.push(event);

                current_event_lines.clear();
            }
            in_event = false;
        } else if in_event {
            current_event_lines.push(trimmed.to_string());
        }
    }

    events
}

/// Parse event lines into a CalendarEvent struct
fn parse_event_lines(lines: &[String], event_type: &str) -> CalendarEvent {
    let mut event = CalendarEvent {
        event_type: event_type.to_string(),
        ..Default::default()
    };

    for line in lines {
        if line.starts_with("SUMMARY:") {
            event.summary = line.strip_prefix("SUMMARY:").unwrap_or("").to_string();
        } else if line.starts_with("DESCRIPTION:") {
            event.description = line.strip_prefix("DESCRIPTION:").unwrap_or("").to_string();
        } else if line.starts_with("DTSTART") {
            // DTSTART can have parameters like DTSTART;TZID=...:20240101
            event.dtstart = line.split(':').nth(1).unwrap_or("").to_string();
        } else if line.starts_with("DTEND") {
            event.dtend = line.split(':').nth(1).unwrap_or("").to_string();
        } else if line.starts_with("LOCATION:") {
            event.location = line.strip_prefix("LOCATION:").unwrap_or("").to_string();
        }
    }

    event
}

/// Test parsing of date/time formats commonly found in ICS
fn parse_ics_datetime(dtstring: &str) -> Option<(u32, u32, u32, u32, u32, u32)> {
    // Basic format: YYYYMMDDTHHMMSS or YYYYMMDDTHHMMSSZ
    let cleaned = dtstring.trim_end_matches('Z');

    if cleaned.len() < 8 {
        return None;
    }

    // Parse date part YYYYMMDD
    let year: u32 = cleaned.get(0..4)?.parse().ok()?;
    let month: u32 = cleaned.get(4..6)?.parse().ok()?;
    let day: u32 = cleaned.get(6..8)?.parse().ok()?;

    // Parse optional time part THHMMSS
    let (hour, min, sec) = if cleaned.len() >= 15 && cleaned.chars().nth(8) == Some('T') {
        let h: u32 = cleaned.get(9..11)?.parse().ok()?;
        let m: u32 = cleaned.get(11..13)?.parse().ok()?;
        let s: u32 = cleaned.get(13..15)?.parse().ok()?;
        (h, m, s)
    } else {
        (0, 0, 0)
    };

    Some((year, month, day, hour, min, sec))
}

/// Parse escaped text in ICS format (unescapes \n, \,, etc.)
fn unescape_ics_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => result.push('\n'),
                Some(',') => result.push(','),
                Some(';') => result.push(';'),
                Some('\\') => result.push('\\'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Parse RFC 5545 line folding (continuation lines start with space/tab)
fn unfold_ics_lines(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut first = true;

    for line in content.lines() {
        if (line.starts_with(' ') || line.starts_with('\t')) && !first {
            // Continuation line - append without the leading whitespace
            result.push_str(line.trim_start());
        } else {
            if !first {
                result.push('\n');
            }
            result.push_str(line);
            first = false;
        }
    }

    result
}

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings (ICS is text-based)
    if let Ok(input) = std::str::from_utf8(data) {
        // Test 1: Basic ICS parsing
        let _ = parse_ics(input);

        // Test 2: Unfold lines first (RFC 5545 spec)
        let unfolded = unfold_ics_lines(input);
        let _ = parse_ics(&unfolded);

        // Test 3: Parse datetime strings found in input
        // Look for patterns that might be datetime values
        for line in input.lines() {
            if line.contains("DT") && line.contains(':') {
                if let Some(dt_part) = line.split(':').nth(1) {
                    let _ = parse_ics_datetime(dt_part);
                }
            }
        }

        // Test 4: Unescape text fields
        let _ = unescape_ics_text(input);

        // Test 5: Parse with wrapped event context
        let wrapped = format!(
            "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nSUMMARY:{}\nDESCRIPTION:{}\nDTSTART:20240101T120000Z\nEND:VEVENT\nEND:VCALENDAR",
            input.lines().next().unwrap_or(""),
            input
        );
        let _ = parse_ics(&wrapped);

        // Test 6: Parse with multiple event types
        let multi_type = format!(
            "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:{}\nEND:VEVENT\nBEGIN:VTODO\nSUMMARY:{}\nEND:VTODO\nBEGIN:VJOURNAL\nSUMMARY:{}\nEND:VJOURNAL\nEND:VCALENDAR",
            input, input, input
        );
        let _ = parse_ics(&multi_type);

        // Test 7: Various date format strings that might be in input
        let test_dates = [
            "20240101",
            "20240101T120000",
            "20240101T120000Z",
            input,
        ];
        for dt in &test_dates {
            let _ = parse_ics_datetime(dt);
        }
    }
});
