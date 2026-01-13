//
//  iTermURLHelpers.swift
//  DashTerm2
//
//  Created by George Nachman on 6/20/25.
//

import Foundation

private let disallowedURLSchemes: Set<String> = ["javascript", "data", "vbscript"]

func urlHasDisallowedScheme(_ url: URL) -> Bool {
    guard let scheme = url.scheme?.lowercased() else {
        return false
    }
    return disallowedURLSchemes.contains(scheme)
}

private func stringHasDisallowedScheme(_ urlString: String) -> Bool {
    let lowercased = urlString.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    return disallowedURLSchemes.contains { scheme in
        lowercased.hasPrefix("\(scheme):")
    }
}

func normalizeURL(_ urlString: String) -> URL? {
    let trimmed = urlString.trimmingCharacters(in: .whitespacesAndNewlines)

    guard !stringHasDisallowedScheme(trimmed) else {
        return nil
    }

    // If it already has a scheme, use as-is
    if stringHasValidScheme(trimmed) {
        return URL(string: trimmed)
    }

    // If it looks like a domain/IP, add https://
    if isValidDomainOrIP(trimmed) {
        return URL(string: "https://\(trimmed)")
    }
    return nil
}

private func stringHasValidScheme(_ urlString: String) -> Bool {
    guard !stringHasDisallowedScheme(urlString) else {
        return false
    }
    return urlString.hasPrefix("http://") ||
    urlString.hasPrefix("https://") ||
    iTermBrowserSchemes.allSchemes.anySatisfies({urlString.hasPrefix($0 + ":")}) ||
    urlString.hasPrefix(iTermBrowserSchemes.about + ":") ||
    urlString.hasPrefix("about:") ||
    urlString.hasPrefix("file://")
}

func stringIsStronglyURLLike(_ urlString: String) -> Bool {
    let trimmed = urlString.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !stringHasDisallowedScheme(trimmed) else {
        return false
    }
    if stringHasValidScheme(trimmed) {
        return true
    }
    if isValidDomainOrIP(trimmed) && trimmed.contains(".") {
        return true
    }
    return false
}

private func isValidDomainOrIP(_ input: String) -> Bool {
    // Check if it contains spaces (definitely not a URL)
    if input.contains(" ") {
        return false
    }

    // BUG-965: Check for valid IPv4 address pattern with octet range validation
    if isValidIPv4(input) {
        return true
    }

    // BUG-965: If it LOOKS like an IPv4 address (all numeric with dots) but isValidIPv4
    // returned false, reject it entirely - don't fall through to domain check.
    // This prevents "192.168.1.256" from being accepted as a domain name.
    if looksLikeIPv4(input) {
        return false
    }

    // BUG-966: Check for IPv6 address pattern with proper validation
    if isValidIPv6Bracketed(input) {
        return true
    }

    // Check for localhost or local addresses
    if input.hasPrefix("localhost") || input.hasPrefix("127.0.0.1") {
        return true
    }

    // Check if it looks like a domain (contains a dot and no spaces)
    if input.contains(".") && !input.contains(" ") {
        // Additional validation: must have at least one character before and after the dot
        let components = input.split(separator: ".")
        return components.count >= 2 && components.allSatisfy { !$0.isEmpty }
    }

    // Check for intranet-style hostnames (single word, possibly with port)
    let hostPattern = #"^[a-zA-Z0-9-]+(:\d+)?$"#
    if input.range(of: hostPattern, options: .regularExpression) != nil {
        return true
    }

    return false
}

// BUG-965: Validate IPv4 address with proper octet range checking (0-255)
private func isValidIPv4(_ input: String) -> Bool {
    // Strip optional port suffix
    let addressPart: String
    if let colonIndex = input.lastIndex(of: ":") {
        let portPart = input[input.index(after: colonIndex)...]
        // Verify the port is all digits
        guard portPart.allSatisfy({ $0.isNumber }) else { return false }
        addressPart = String(input[..<colonIndex])
    } else {
        addressPart = input
    }

    let octets = addressPart.split(separator: ".")
    guard octets.count == 4 else { return false }

    for octet in octets {
        guard let value = Int(octet), value >= 0 && value <= 255 else {
            return false
        }
        // Reject leading zeros (e.g., "01", "001") unless it's just "0"
        if octet.count > 1 && octet.hasPrefix("0") {
            return false
        }
    }
    return true
}

// BUG-965: Check if string LOOKS like an IPv4 address (all-numeric dotted format)
// Used to prevent invalid IPv4s like "192.168.1.256" from being accepted as domain names
private func looksLikeIPv4(_ input: String) -> Bool {
    // Strip optional port suffix
    let addressPart: String
    if let colonIndex = input.lastIndex(of: ":") {
        addressPart = String(input[..<colonIndex])
    } else {
        addressPart = input
    }

    let parts = addressPart.split(separator: ".")
    // Must have exactly 4 parts (like an IPv4)
    guard parts.count == 4 else { return false }
    // All parts must be purely numeric
    return parts.allSatisfy { part in
        !part.isEmpty && part.allSatisfy { $0.isNumber }
    }
}

// BUG-966: Validate bracketed IPv6 address format
private func isValidIPv6Bracketed(_ input: String) -> Bool {
    guard input.hasPrefix("[") else { return false }

    // Find the closing bracket, optionally followed by :port
    guard let closingBracket = input.firstIndex(of: "]") else { return false }

    let ipv6Part = String(input[input.index(after: input.startIndex)..<closingBracket])

    // Check for optional port after bracket
    let afterBracket = input[input.index(after: closingBracket)...]
    if !afterBracket.isEmpty {
        // Must be :port format
        guard afterBracket.hasPrefix(":") else { return false }
        let portPart = afterBracket.dropFirst()
        guard !portPart.isEmpty && portPart.allSatisfy({ $0.isNumber }) else { return false }
    }

    // Basic IPv6 validation: must contain colons and hex digits
    guard ipv6Part.contains(":") else { return false }

    let groups = ipv6Part.split(separator: ":", omittingEmptySubsequences: false)
    // IPv6 has at most 8 groups, or fewer with :: compression
    guard groups.count <= 8 else { return false }

    // Check each group is valid hex (0-4 chars)
    let hexChars = CharacterSet(charactersIn: "0123456789abcdefABCDEF")
    for group in groups {
        // Empty group is allowed (for :: compression)
        if group.isEmpty { continue }
        guard group.count <= 4 else { return false }
        guard group.unicodeScalars.allSatisfy({ hexChars.contains($0) }) else { return false }
    }

    return true
}
