// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Fuzz target for vCard (VCF) contact format parser in dashflow
//!
//! Tests parsing of:
//! 1. Basic vCard 2.1/3.0/4.0 formats
//! 2. Contact fields (FN, EMAIL, TEL, ORG, etc.)
//! 3. Multiple contacts in a single file
//! 4. Escaped characters and encoding
//!
//! SECURITY CRITICAL: vCard files can be user-supplied and may contain
//! malformed data, oversized fields, or crafted content to exploit parsers.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// Parsed contact information
#[derive(Debug, Default)]
struct Contact {
    name: String,
    emails: Vec<String>,
    phones: Vec<String>,
    org: String,
    title: String,
    note: String,
    version: String,
}

/// Parse VCF format
/// Mirrors the parsing logic in VCFLoader
fn parse_vcf(content: &str) -> Vec<Contact> {
    let mut contacts = Vec::new();
    let mut current_contact_lines: Vec<String> = Vec::new();
    let mut in_vcard = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("BEGIN:VCARD") {
            in_vcard = true;
            current_contact_lines.clear();
            current_contact_lines.push(trimmed.to_string());
        } else if trimmed.starts_with("END:VCARD") {
            if in_vcard {
                current_contact_lines.push(trimmed.to_string());

                // Parse the contact
                let contact = parse_contact_lines(&current_contact_lines);
                contacts.push(contact);

                current_contact_lines.clear();
            }
            in_vcard = false;
        } else if in_vcard {
            current_contact_lines.push(trimmed.to_string());
        }
    }

    contacts
}

/// Parse contact lines into a Contact struct
fn parse_contact_lines(lines: &[String]) -> Contact {
    let mut contact = Contact::default();

    for line in lines {
        // Handle FN (formatted name)
        if line.starts_with("FN:") {
            contact.name = line.strip_prefix("FN:").unwrap_or("").to_string();
        }
        // Handle EMAIL (with optional type parameters)
        else if line.starts_with("EMAIL") {
            if let Some(email) = line.split(':').nth(1) {
                contact.emails.push(email.to_string());
            }
        }
        // Handle TEL (with optional type parameters)
        else if line.starts_with("TEL") {
            if let Some(phone) = line.split(':').nth(1) {
                contact.phones.push(phone.to_string());
            }
        }
        // Handle ORG
        else if line.starts_with("ORG:") {
            contact.org = line.strip_prefix("ORG:").unwrap_or("").to_string();
        }
        // Handle TITLE
        else if line.starts_with("TITLE:") {
            contact.title = line.strip_prefix("TITLE:").unwrap_or("").to_string();
        }
        // Handle NOTE
        else if line.starts_with("NOTE:") {
            contact.note = line.strip_prefix("NOTE:").unwrap_or("").to_string();
        }
        // Handle VERSION
        else if line.starts_with("VERSION:") {
            contact.version = line.strip_prefix("VERSION:").unwrap_or("").to_string();
        }
    }

    contact
}

/// Parse vCard property parameters (e.g., "EMAIL;TYPE=WORK:" -> "WORK")
fn parse_property_params(property_line: &str) -> Vec<(String, String)> {
    let mut params = Vec::new();

    // Split on colon to separate property+params from value
    if let Some(prop_part) = property_line.split(':').next() {
        // Split on semicolons to get individual parameters
        for param in prop_part.split(';').skip(1) {
            if let Some(eq_pos) = param.find('=') {
                let key = param[..eq_pos].to_string();
                let value = param[eq_pos + 1..].to_string();
                params.push((key, value));
            } else {
                // Parameter without value (e.g., "PREF")
                params.push((param.to_string(), String::new()));
            }
        }
    }

    params
}

/// Unescape vCard text (handles \n, \,, \;, etc.)
fn unescape_vcf_text(text: &str) -> String {
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

/// Parse N (structured name) field: "N:Last;First;Middle;Prefix;Suffix"
fn parse_structured_name(n_value: &str) -> (String, String, String, String, String) {
    let parts: Vec<&str> = n_value.split(';').collect();
    let last = parts.first().unwrap_or(&"").to_string();
    let first = parts.get(1).unwrap_or(&"").to_string();
    let middle = parts.get(2).unwrap_or(&"").to_string();
    let prefix = parts.get(3).unwrap_or(&"").to_string();
    let suffix = parts.get(4).unwrap_or(&"").to_string();
    (last, first, middle, prefix, suffix)
}

/// Parse ADR (address) field: "ADR:;;Street;City;State;Zip;Country"
fn parse_address(adr_value: &str) -> Vec<String> {
    adr_value.split(';').map(|s| s.to_string()).collect()
}

/// Handle line folding (RFC 6350: continuation lines start with space/tab)
fn unfold_vcf_lines(content: &str) -> String {
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
    // Only fuzz valid UTF-8 strings (VCF is text-based)
    if let Ok(input) = std::str::from_utf8(data) {
        // Test 1: Basic VCF parsing
        let _ = parse_vcf(input);

        // Test 2: Unfold lines first (RFC 6350 spec)
        let unfolded = unfold_vcf_lines(input);
        let _ = parse_vcf(&unfolded);

        // Test 3: Parse property parameters in input lines
        for line in input.lines() {
            let _ = parse_property_params(line);
        }

        // Test 4: Unescape text fields
        let _ = unescape_vcf_text(input);

        // Test 5: Parse structured name if input contains semicolons
        let _ = parse_structured_name(input);

        // Test 6: Parse as address
        let _ = parse_address(input);

        // Test 7: Parse with wrapped vCard context
        let wrapped = format!(
            "BEGIN:VCARD\nVERSION:3.0\nFN:{}\nEMAIL:{}\nTEL:{}\nORG:{}\nEND:VCARD",
            input.lines().next().unwrap_or("Test User"),
            input.lines().nth(1).unwrap_or("test@example.com"),
            input.lines().nth(2).unwrap_or("+1234567890"),
            input
        );
        let _ = parse_vcf(&wrapped);

        // Test 8: Parse multiple contacts
        let multi_contact = format!(
            "BEGIN:VCARD\nFN:Contact 1\nEMAIL:{}\nEND:VCARD\nBEGIN:VCARD\nFN:Contact 2\nEMAIL:{}\nEND:VCARD",
            input, input
        );
        let _ = parse_vcf(&multi_contact);

        // Test 9: Various vCard versions
        for version in &["2.1", "3.0", "4.0"] {
            let versioned = format!(
                "BEGIN:VCARD\nVERSION:{}\nFN:{}\nEND:VCARD",
                version, input
            );
            let _ = parse_vcf(&versioned);
        }
    }
});
