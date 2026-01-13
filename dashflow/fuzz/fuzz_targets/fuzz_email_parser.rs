// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for email format parsers in dashflow
//!
//! Tests parsing of:
//! 1. EML (email message format) - RFC 822 email headers and body
//! 2. MBOX (mailbox format) - Unix mailbox with "From " separators
//! 3. MHTML (MIME HTML) - Web archive multipart format
//! 4. EMLX (Apple Mail) - Email with byte count prefix
//!
//! SECURITY CRITICAL: These parsers accept user-supplied email content,
//! which may contain malformed headers, injection attempts, or malicious content.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// Parse EML format (email headers + body separated by blank line)
/// Mirrors the parsing logic in EMLLoader
fn parse_eml(content: &str, include_all_headers: bool) -> Option<Vec<(String, String)>> {
    let common_headers = ["From", "To", "Cc", "Bcc", "Subject", "Date", "Reply-To"];

    // Split headers from body
    let parts: Vec<&str> = content.splitn(2, "\n\n").collect();
    let headers_text = if parts.len() == 2 { parts[0] } else { content };

    let mut results = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    for line in headers_text.lines() {
        if line.trim().is_empty() {
            if let Some(key) = current_key.take() {
                let value = current_value.trim();
                let is_common = common_headers.iter().any(|h| h.eq_ignore_ascii_case(&key));
                if include_all_headers || is_common {
                    results.push((key, value.to_string()));
                }
                current_value.clear();
            }
            continue;
        }

        // Continuation lines start with whitespace
        if line.starts_with(' ') || line.starts_with('\t') {
            if current_key.is_some() {
                current_value.push(' ');
                current_value.push_str(line.trim());
            }
        } else if let Some(colon_pos) = line.find(':') {
            // New header - flush previous
            if let Some(key) = current_key.take() {
                let value = current_value.trim();
                let is_common = common_headers.iter().any(|h| h.eq_ignore_ascii_case(&key));
                if include_all_headers || is_common {
                    results.push((key, value.to_string()));
                }
                current_value.clear();
            }

            let (key, value) = line.split_at(colon_pos);
            current_key = Some(key.trim().to_string());
            current_value = value.get(1..).unwrap_or("").trim().to_string();
        }
    }

    // Flush final header
    if let Some(key) = current_key {
        let value = current_value.trim();
        let is_common = common_headers.iter().any(|h| h.eq_ignore_ascii_case(&key));
        if include_all_headers || is_common {
            results.push((key, value.to_string()));
        }
    }

    Some(results)
}

/// Parse MBOX format (messages separated by "From " lines)
/// Mirrors the parsing logic in MBOXLoader
fn parse_mbox(content: &str) -> Vec<String> {
    let mut documents = Vec::new();
    let mut current_message = Vec::new();

    for line in content.lines() {
        // MBOX format: messages start with "From " (note the space)
        if line.starts_with("From ") && !current_message.is_empty() {
            let message_text = current_message.join("\n");
            if !message_text.trim().is_empty() {
                documents.push(message_text);
            }
            current_message.clear();
        }
        current_message.push(line.to_string());
    }

    // Process last message
    if !current_message.is_empty() {
        let message_text = current_message.join("\n");
        if !message_text.trim().is_empty() {
            documents.push(message_text);
        }
    }

    documents
}

/// Parse MHTML multipart format
/// Mirrors the parsing logic in MHTMLLoader
fn parse_mhtml(content: &str) -> Vec<String> {
    let mut text_content = Vec::new();
    let mut in_text_part = false;
    let mut current_part = Vec::new();

    for line in content.lines() {
        // Check for content-type headers
        if line.to_lowercase().starts_with("content-type:") {
            let content_type = line.to_lowercase();
            in_text_part =
                content_type.contains("text/html") || content_type.contains("text/plain");

            // Save previous part if it was text
            if !current_part.is_empty() {
                let part_text = current_part.join("\n");
                if !part_text.trim().is_empty() {
                    text_content.push(part_text);
                }
                current_part.clear();
            }
            continue;
        }

        // Check for MIME boundaries (lines starting with --)
        if line.starts_with("--") && line.len() > 10 {
            in_text_part = false;
            if !current_part.is_empty() {
                let part_text = current_part.join("\n");
                if !part_text.trim().is_empty() {
                    text_content.push(part_text);
                }
                current_part.clear();
            }
            continue;
        }

        // Skip headers until we find the content
        if in_text_part && !line.is_empty() && !line.contains(':') {
            current_part.push(line.to_string());
        }
    }

    // Add final part
    if !current_part.is_empty() {
        let part_text = current_part.join("\n");
        if !part_text.trim().is_empty() {
            text_content.push(part_text);
        }
    }

    text_content
}

/// Parse EMLX format (Apple Mail with byte count prefix)
/// Mirrors the parsing logic in EMLXLoader
fn parse_emlx(content: &str) -> Option<(String, String, String, String)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return None;
    }

    // Skip first line (byte count), parse rest as email
    let email_text = lines.iter().skip(1).copied().collect::<Vec<_>>().join("\n");

    let mut from = String::new();
    let mut to = String::new();
    let mut subject = String::new();
    let mut date = String::new();
    let mut in_body = false;

    for line in email_text.lines() {
        if in_body {
            continue;
        } else if line.trim().is_empty() {
            in_body = true;
        } else {
            let header_lower = line.to_lowercase();
            if header_lower.starts_with("from:") {
                from = line.get(5..).unwrap_or("").trim().to_string();
            } else if header_lower.starts_with("to:") {
                to = line.get(3..).unwrap_or("").trim().to_string();
            } else if header_lower.starts_with("subject:") {
                subject = line.get(8..).unwrap_or("").trim().to_string();
            } else if header_lower.starts_with("date:") {
                date = line.get(5..).unwrap_or("").trim().to_string();
            }
        }
    }

    Some((from, to, subject, date))
}

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings (email is text-based)
    if let Ok(input) = std::str::from_utf8(data) {
        // Test 1: EML parser with common headers only
        let _ = parse_eml(input, false);

        // Test 2: EML parser with all headers
        let _ = parse_eml(input, true);

        // Test 3: MBOX parser
        let _ = parse_mbox(input);

        // Test 4: MHTML parser
        let _ = parse_mhtml(input);

        // Test 5: EMLX parser (Apple Mail)
        let _ = parse_emlx(input);

        // Test 6: Parse with various edge cases embedded
        // Test header continuation lines
        let with_continuation = format!("Subject: Line1\n Line2\n\t{}\n\n", input);
        let _ = parse_eml(&with_continuation, true);

        // Test MBOX with multiple From lines
        let mbox_multi = format!("From sender@test.com\n{}\nFrom other@test.com\n{}", input, input);
        let _ = parse_mbox(&mbox_multi);

        // Test MHTML with various content types
        let mhtml_multi = format!(
            "Content-Type: text/html\n\n{}\n--boundary123\nContent-Type: text/plain\n\n{}",
            input, input
        );
        let _ = parse_mhtml(&mhtml_multi);
    }
});
