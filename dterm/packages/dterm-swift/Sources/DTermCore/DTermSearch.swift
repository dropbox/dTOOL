/*
 * DTermSearch.swift - Trigram-indexed search for DTermCore
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 */

import Foundation
import CDTermCore

// MARK: - Search Direction

/// Direction for search iteration.
public enum DTermSearchDirection {
    /// Search forward (oldest to newest).
    case forward
    /// Search backward (newest to oldest).
    case backward

    var ffiValue: dterm_search_direction_t {
        switch self {
        case .forward:
            return DTERM_SEARCH_DIRECTION_T_FORWARD
        case .backward:
            return DTERM_SEARCH_DIRECTION_T_BACKWARD
        }
    }
}

// MARK: - Search Match

/// A match found during search.
public struct DTermSearchMatch: Equatable {
    /// Line number (0-indexed from oldest).
    public let line: Int
    /// Starting column of the match (0-indexed).
    public let startCol: Int
    /// Ending column of the match (exclusive).
    public let endCol: Int

    /// Get the length of the match in columns.
    public var length: Int {
        endCol - startCol
    }

    /// Check if this is an empty match (zero length).
    public var isEmpty: Bool {
        endCol <= startCol
    }

    /// Check if a column is within this match.
    public func containsColumn(_ col: Int) -> Bool {
        col >= startCol && col < endCol
    }

    init(from ffi: dterm_search_match_t) {
        self.line = Int(ffi.line)
        self.startCol = Int(ffi.start_col)
        self.endCol = Int(ffi.end_col)
    }

    public init(line: Int, startCol: Int, endCol: Int) {
        self.line = line
        self.startCol = startCol
        self.endCol = endCol
    }
}

// MARK: - Search Index

/// Trigram-indexed search with bloom filter acceleration.
///
/// ## Performance
///
/// | Operation | Time Complexity |
/// |-----------|-----------------|
/// | Negative lookup | O(1) via bloom filter |
/// | Positive search | O(k) where k = matching lines |
/// | Index line | O(n) where n = line length |
///
/// ## Usage
///
/// ```swift
/// let search = DTermSearch()
///
/// // Index content
/// search.indexLine("hello world")
/// search.indexLine("goodbye world")
///
/// // Search
/// let matches = search.find("world")
/// for match in matches {
///     print("Found at line \(match.line), columns \(match.startCol)..<\(match.endCol)")
/// }
/// ```
public final class DTermSearch {
    /// Opaque handle to the underlying dterm-core search index.
    private var handle: OpaquePointer?

    // MARK: - Lifecycle

    /// Create a new search index.
    public init() {
        self.handle = dterm_search_new()
    }

    /// Create a new search index with expected capacity.
    ///
    /// - Parameter expectedLines: Expected number of lines to index.
    public init(expectedLines: Int) {
        self.handle = dterm_search_with_capacity(UInt(expectedLines))
    }

    deinit {
        if let handle = handle {
            dterm_search_free(handle)
        }
    }

    // MARK: - Indexing

    /// Index a line of text.
    ///
    /// Call this for each line of content you want to search.
    /// Lines are assigned sequential line numbers starting from 0.
    ///
    /// - Parameter text: The text content of the line.
    public func indexLine(_ text: String) {
        guard let handle = handle else { return }
        text.withCString { ptr in
            dterm_search_index_line(handle, ptr, UInt(text.utf8.count))
        }
    }

    /// Index multiple lines of text.
    ///
    /// - Parameter lines: Array of text lines to index.
    public func indexLines(_ lines: [String]) {
        for line in lines {
            indexLine(line)
        }
    }

    // MARK: - Query

    /// Check if a query might have matches (bloom filter check).
    ///
    /// Returns `false` if definitely no matches exist.
    /// Returns `true` if matches are possible (verify with actual search).
    ///
    /// This is a fast O(1) operation useful for early rejection.
    ///
    /// - Parameter query: The search query.
    /// - Returns: True if matches are possible.
    public func mightContain(_ query: String) -> Bool {
        guard let handle = handle else { return false }
        return query.withCString { ptr in
            dterm_search_might_contain(handle, ptr, UInt(query.utf8.count))
        }
    }

    /// Search for a query string.
    ///
    /// Returns actual matches with line and column positions.
    ///
    /// - Parameters:
    ///   - query: The search query (3+ characters for trigram indexing).
    ///   - maxMatches: Maximum number of matches to return (default 1000).
    /// - Returns: Array of matches found.
    public func find(_ query: String, maxMatches: Int = 1000) -> [DTermSearchMatch] {
        guard let handle = handle else { return [] }

        var matches = [dterm_search_match_t](repeating: dterm_search_match_t(), count: maxMatches)
        let count = query.withCString { ptr in
            matches.withUnsafeMutableBufferPointer { buffer in
                dterm_search_find(handle, ptr, UInt(query.utf8.count), buffer.baseAddress, UInt(maxMatches))
            }
        }

        return matches.prefix(Int(count)).map { DTermSearchMatch(from: $0) }
    }

    /// Search for a query string in the specified direction.
    ///
    /// - Parameters:
    ///   - query: The search query.
    ///   - direction: Search direction (forward or backward).
    ///   - maxMatches: Maximum number of matches to return.
    /// - Returns: Array of matches sorted by direction.
    public func findOrdered(_ query: String, direction: DTermSearchDirection, maxMatches: Int = 1000) -> [DTermSearchMatch] {
        guard let handle = handle else { return [] }

        var matches = [dterm_search_match_t](repeating: dterm_search_match_t(), count: maxMatches)
        let count = query.withCString { ptr in
            matches.withUnsafeMutableBufferPointer { buffer in
                dterm_search_find_ordered(handle, ptr, UInt(query.utf8.count), direction.ffiValue, buffer.baseAddress, UInt(maxMatches))
            }
        }

        return matches.prefix(Int(count)).map { DTermSearchMatch(from: $0) }
    }

    /// Find the next match after the given position.
    ///
    /// Uses O(log n) range queries to efficiently skip earlier lines.
    ///
    /// - Parameters:
    ///   - query: The search query.
    ///   - afterLine: Start searching after this line.
    ///   - afterCol: Start searching after this column on the starting line.
    /// - Returns: The next match, or nil if none found.
    public func findNext(_ query: String, afterLine: Int, afterCol: Int) -> DTermSearchMatch? {
        guard let handle = handle else { return nil }

        var match = dterm_search_match_t()
        let found = query.withCString { ptr in
            dterm_search_find_next(handle, ptr, UInt(query.utf8.count), UInt(afterLine), UInt(afterCol), &match)
        }

        return found ? DTermSearchMatch(from: match) : nil
    }

    /// Find the previous match before the given position.
    ///
    /// Uses O(log n) range queries to efficiently skip later lines.
    ///
    /// - Parameters:
    ///   - query: The search query.
    ///   - beforeLine: Search before this line.
    ///   - beforeCol: Search before this column on the ending line.
    /// - Returns: The previous match, or nil if none found.
    public func findPrev(_ query: String, beforeLine: Int, beforeCol: Int) -> DTermSearchMatch? {
        guard let handle = handle else { return nil }

        var match = dterm_search_match_t()
        let found = query.withCString { ptr in
            dterm_search_find_prev(handle, ptr, UInt(query.utf8.count), UInt(beforeLine), UInt(beforeCol), &match)
        }

        return found ? DTermSearchMatch(from: match) : nil
    }

    // MARK: - State

    /// Number of indexed lines.
    public var lineCount: Int {
        guard let handle = handle else { return 0 }
        return Int(dterm_search_line_count(handle))
    }

    /// Whether the index is empty.
    public var isEmpty: Bool {
        lineCount == 0
    }

    /// Clear the search index.
    public func clear() {
        guard let handle = handle else { return }
        dterm_search_clear(handle)
    }
}
