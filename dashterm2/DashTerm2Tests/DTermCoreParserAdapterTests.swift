// DTermCoreParserAdapterTests.swift
// DashTerm2Tests
//
// Tests for DTermCoreParserAdapter - verifying dterm-core parser to VT100Token conversion.
// These tests compare parser output between dterm-core and iTerm2's VT100Parser.

import XCTest
@testable import DashTerm2SharedARC

/// Tests for DTermCoreParserAdapter functionality.
///
/// These tests verify that the dterm-core parser produces VT100Token objects
/// that match (or are compatible with) iTerm2's VT100Parser output.
final class DTermCoreParserAdapterTests: XCTestCase {

    // MARK: - ASAN Detection

    /// Check if AddressSanitizer is enabled at runtime.
    /// ASAN instruments memory access in ways that cause false positives when
    /// Swift dereferences large FFI struct pointers across language boundaries.
    private static let isASANEnabled: Bool = {
        // ASAN runtime exports this symbol when active
        let asanSymbol = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "__asan_address_is_poisoned")
        return asanSymbol != nil
    }()

    // MARK: - Test Infrastructure

    private var adapter: DTermCoreParserAdapter!

    override func setUpWithError() throws {
        try super.setUpWithError()

        // Skip all DTermCoreParserAdapter tests under ASAN.
        // The dterm_action_t FFI struct is ~6KB (2KB osc_payload + 4KB dcs_payload)
        // and ASAN interferes with how Swift accesses the struct via pointer in
        // the parserCallback function, causing false positive memory errors.
        if Self.isASANEnabled {
            throw XCTSkip("DTermCoreParserAdapter tests skipped under AddressSanitizer due to large FFI struct")
        }

        adapter = DTermCoreParserAdapter()
    }

    override func tearDown() {
        adapter = nil
        super.tearDown()
    }

    // MARK: - Basic ASCII Parsing

    func test_asciiString_parsesCorrectly() {
        let data = "Hello, World!".data(using: .utf8)!
        let tokens = adapter.parse(data: data)

        // Should produce at least one ASCII string token
        XCTAssertFalse(tokens.isEmpty, "Should produce at least one token for ASCII text")

        // First token should be ASCII string type
        if let firstToken = tokens.first {
            XCTAssertEqual(firstToken.type, VT100_ASCIISTRING,
                          "ASCII text should produce VT100_ASCIISTRING token")
        }
    }

    func test_emptyData_returnsNoTokens() {
        let data = Data()
        let tokens = adapter.parse(data: data)

        XCTAssertTrue(tokens.isEmpty, "Empty data should produce no tokens")
    }

    // MARK: - Control Character Tests

    func test_carriageReturn_producesControlToken() {
        let data = Data([0x0D])  // CR
        let tokens = adapter.parse(data: data)

        XCTAssertEqual(tokens.count, 1, "CR should produce one token")
        if let token = tokens.first {
            // Control character tokens have type equal to the control code
            XCTAssertEqual(token.type.rawValue, UInt32(VT100CC_CR.rawValue),
                          "CR should produce VT100CC_CR token type")
        }
    }

    func test_lineFeed_producesControlToken() {
        let data = Data([0x0A])  // LF
        let tokens = adapter.parse(data: data)

        XCTAssertEqual(tokens.count, 1, "LF should produce one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type.rawValue, UInt32(VT100CC_LF.rawValue),
                          "LF should produce VT100CC_LF token type")
        }
    }

    func test_backspace_producesControlToken() {
        let data = Data([0x08])  // BS
        let tokens = adapter.parse(data: data)

        XCTAssertEqual(tokens.count, 1, "BS should produce one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type.rawValue, UInt32(VT100CC_BS.rawValue),
                          "BS should produce VT100CC_BS token type")
        }
    }

    func test_tab_producesControlToken() {
        let data = Data([0x09])  // HT (horizontal tab)
        let tokens = adapter.parse(data: data)

        XCTAssertEqual(tokens.count, 1, "TAB should produce one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type.rawValue, UInt32(VT100CC_HT.rawValue),
                          "TAB should produce VT100CC_HT token type")
        }
    }

    func test_bell_producesControlToken() {
        let data = Data([0x07])  // BEL
        let tokens = adapter.parse(data: data)

        XCTAssertEqual(tokens.count, 1, "BEL should produce one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type.rawValue, UInt32(VT100CC_BEL.rawValue),
                          "BEL should produce VT100CC_BEL token type")
        }
    }

    // MARK: - CSI Sequence Tests

    func test_cursorUp_CSI_A() {
        // ESC [ A - Cursor Up (default 1)
        let data = Data([0x1B, 0x5B, 0x41])  // ESC [ A
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI A should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_CUU,
                          "CSI A should produce VT100CSI_CUU token")
        }
    }

    func test_cursorDown_CSI_B() {
        // ESC [ B - Cursor Down
        let data = Data([0x1B, 0x5B, 0x42])  // ESC [ B
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI B should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_CUD,
                          "CSI B should produce VT100CSI_CUD token")
        }
    }

    func test_cursorForward_CSI_C() {
        // ESC [ C - Cursor Forward
        let data = Data([0x1B, 0x5B, 0x43])  // ESC [ C
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI C should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_CUF,
                          "CSI C should produce VT100CSI_CUF token")
        }
    }

    func test_cursorBackward_CSI_D() {
        // ESC [ D - Cursor Backward
        let data = Data([0x1B, 0x5B, 0x44])  // ESC [ D
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI D should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_CUB,
                          "CSI D should produce VT100CSI_CUB token")
        }
    }

    func test_cursorPosition_CSI_H() {
        // ESC [ H - Cursor Position (home)
        let data = Data([0x1B, 0x5B, 0x48])  // ESC [ H
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI H should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_CUP,
                          "CSI H should produce VT100CSI_CUP token")
        }
    }

    func test_cursorPosition_withParams_CSI_H() {
        // ESC [ 10 ; 20 H - Cursor Position (row 10, col 20)
        let data = Data([0x1B, 0x5B, 0x31, 0x30, 0x3B, 0x32, 0x30, 0x48])
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI with params should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_CUP,
                          "CSI H with params should produce VT100CSI_CUP token")
        }
    }

    func test_eraseDisplay_CSI_J() {
        // ESC [ J - Erase in Display (default: from cursor to end)
        let data = Data([0x1B, 0x5B, 0x4A])  // ESC [ J
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI J should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_ED,
                          "CSI J should produce VT100CSI_ED token")
        }
    }

    func test_eraseLine_CSI_K() {
        // ESC [ K - Erase in Line (default: from cursor to end)
        let data = Data([0x1B, 0x5B, 0x4B])  // ESC [ K
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI K should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_EL,
                          "CSI K should produce VT100CSI_EL token")
        }
    }

    func test_sgr_CSI_m() {
        // ESC [ m - Select Graphic Rendition (reset)
        let data = Data([0x1B, 0x5B, 0x6D])  // ESC [ m
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI m should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_SGR,
                          "CSI m should produce VT100CSI_SGR token")
        }
    }

    func test_sgr_bold_CSI_1m() {
        // ESC [ 1 m - Bold
        let data = Data([0x1B, 0x5B, 0x31, 0x6D])  // ESC [ 1 m
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI 1m should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_SGR,
                          "CSI 1m should produce VT100CSI_SGR token")
        }
    }

    // MARK: - ESC Sequence Tests

    func test_saveCursor_ESC_7() {
        // ESC 7 - Save Cursor
        let data = Data([0x1B, 0x37])  // ESC 7
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "ESC 7 should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_DECSC,
                          "ESC 7 should produce VT100CSI_DECSC token")
        }
    }

    func test_restoreCursor_ESC_8() {
        // ESC 8 - Restore Cursor
        let data = Data([0x1B, 0x38])  // ESC 8
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "ESC 8 should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_DECRC,
                          "ESC 8 should produce VT100CSI_DECRC token")
        }
    }

    func test_index_ESC_D() {
        // ESC D - Index (move cursor down, scroll if at bottom)
        let data = Data([0x1B, 0x44])  // ESC D
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "ESC D should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_IND,
                          "ESC D should produce VT100CSI_IND token")
        }
    }

    func test_reverseIndex_ESC_M() {
        // ESC M - Reverse Index (move cursor up, scroll if at top)
        let data = Data([0x1B, 0x4D])  // ESC M
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "ESC M should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_RI,
                          "ESC M should produce VT100CSI_RI token")
        }
    }

    // MARK: - Mixed Content Tests

    func test_textWithControlChars() {
        // "Hello\r\nWorld"
        let data = "Hello\r\nWorld".data(using: .utf8)!
        let tokens = adapter.parse(data: data)

        // Should have: ASCII "Hello", CR, LF, ASCII "World"
        XCTAssertTrue(tokens.count >= 3, "Mixed content should produce multiple tokens")
    }

    func test_textWithCSI() {
        // "Hello" ESC [ 2 J "World"
        var data = "Hello".data(using: .utf8)!
        data.append(contentsOf: [0x1B, 0x5B, 0x32, 0x4A])  // ESC [ 2 J
        data.append("World".data(using: .utf8)!)
        let tokens = adapter.parse(data: data)

        // Should have: ASCII "Hello", CSI ED, ASCII "World"
        XCTAssertTrue(tokens.count >= 3, "Text with CSI should produce multiple tokens")

        // Find the ED token
        let edToken = tokens.first { $0.type == VT100CSI_ED }
        XCTAssertNotNil(edToken, "Should contain VT100CSI_ED token")
    }

    // MARK: - Reset Tests

    func test_reset_clearsState() {
        // Parse some data
        let data = "Hello".data(using: .utf8)!
        _ = adapter.parse(data: data)

        // Reset
        adapter.reset()

        // Parse again
        let tokens = adapter.parse(data: data)

        // Should still work after reset
        XCTAssertFalse(tokens.isEmpty, "Parser should work after reset")
    }

    // MARK: - Disabled State Tests

    func test_disabled_returnsNoTokens() {
        adapter.enabled = false

        let data = "Hello".data(using: .utf8)!
        let tokens = adapter.parse(data: data)

        XCTAssertTrue(tokens.isEmpty, "Disabled adapter should return no tokens")
    }

    // MARK: - Unicode Tests

    func test_unicodeCharacters() {
        // UTF-8 encoded emoji
        let data = "Hello 👋".data(using: .utf8)!
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "Unicode text should produce tokens")
    }

    func test_multibyte_utf8() {
        // Chinese characters "中文"
        let data = "中文".data(using: .utf8)!
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "Multi-byte UTF-8 should produce tokens")
    }

    // MARK: - Additional ESC Sequence Tests

    func test_nextLine_ESC_E() {
        // ESC E - Next Line (NEL)
        let data = Data([0x1B, 0x45])  // ESC E
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "ESC E should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_NEL,
                          "ESC E should produce VT100CSI_NEL token")
        }
    }

    func test_horizontalTabSet_ESC_H() {
        // ESC H - Horizontal Tab Set (HTS)
        let data = Data([0x1B, 0x48])  // ESC H
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "ESC H should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_HTS,
                          "ESC H should produce VT100CSI_HTS token")
        }
    }

    func test_applicationKeypad_ESC_equals() {
        // ESC = - Application Keypad Mode (DECKPAM)
        let data = Data([0x1B, 0x3D])  // ESC =
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "ESC = should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_DECKPAM,
                          "ESC = should produce VT100CSI_DECKPAM token")
        }
    }

    func test_normalKeypad_ESC_gt() {
        // ESC > - Normal Keypad Mode (DECKPNM)
        let data = Data([0x1B, 0x3E])  // ESC >
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "ESC > should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_DECKPNM,
                          "ESC > should produce VT100CSI_DECKPNM token")
        }
    }

    func test_resetToInitialState_ESC_c() {
        // ESC c - Reset to Initial State (RIS)
        let data = Data([0x1B, 0x63])  // ESC c
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "ESC c should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_RIS,
                          "ESC c should produce VT100CSI_RIS token")
        }
    }

    // MARK: - Additional CSI Sequence Tests

    func test_cursorNextLine_CSI_E() {
        // ESC [ E - Cursor Next Line
        let data = Data([0x1B, 0x5B, 0x45])  // ESC [ E
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI E should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_CNL,
                          "CSI E should produce VT100CSI_CNL token")
        }
    }

    func test_cursorPrecedingLine_CSI_F() {
        // ESC [ F - Cursor Preceding Line
        let data = Data([0x1B, 0x5B, 0x46])  // ESC [ F
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI F should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_CPL,
                          "CSI F should produce VT100CSI_CPL token")
        }
    }

    func test_cursorCharacterAbsolute_CSI_G() {
        // ESC [ G - Cursor Character Absolute
        let data = Data([0x1B, 0x5B, 0x47])  // ESC [ G
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI G should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, ANSICSI_CHA,
                          "CSI G should produce ANSICSI_CHA token")
        }
    }

    func test_insertLines_CSI_L() {
        // ESC [ L - Insert Lines
        let data = Data([0x1B, 0x5B, 0x4C])  // ESC [ L
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI L should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, XTERMCC_INSLN,
                          "CSI L should produce XTERMCC_INSLN token")
        }
    }

    func test_deleteLines_CSI_M() {
        // ESC [ M - Delete Lines
        let data = Data([0x1B, 0x5B, 0x4D])  // ESC [ M
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI M should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, XTERMCC_DELLN,
                          "CSI M should produce XTERMCC_DELLN token")
        }
    }

    func test_deleteCharacters_CSI_P() {
        // ESC [ P - Delete Characters
        let data = Data([0x1B, 0x5B, 0x50])  // ESC [ P
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI P should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, XTERMCC_DELCH,
                          "CSI P should produce XTERMCC_DELCH token")
        }
    }

    func test_scrollUp_CSI_S() {
        // ESC [ S - Scroll Up
        let data = Data([0x1B, 0x5B, 0x53])  // ESC [ S
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI S should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, XTERMCC_SU,
                          "CSI S should produce XTERMCC_SU token")
        }
    }

    func test_scrollDown_CSI_T() {
        // ESC [ T - Scroll Down
        let data = Data([0x1B, 0x5B, 0x54])  // ESC [ T
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI T should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, XTERMCC_SD,
                          "CSI T should produce XTERMCC_SD token")
        }
    }

    func test_insertCharacters_CSI_at() {
        // ESC [ @ - Insert Characters
        let data = Data([0x1B, 0x5B, 0x40])  // ESC [ @
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI @ should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_ICH,
                          "CSI @ should produce VT100CSI_ICH token")
        }
    }

    func test_setScrollingRegion_CSI_r() {
        // ESC [ r - Set Scrolling Region (DECSTBM)
        let data = Data([0x1B, 0x5B, 0x72])  // ESC [ r
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI r should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_DECSTBM,
                          "CSI r should produce VT100CSI_DECSTBM token")
        }
    }

    func test_tabClear_CSI_g() {
        // ESC [ g - Tab Clear (TBC)
        let data = Data([0x1B, 0x5B, 0x67])  // ESC [ g
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI g should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_TBC,
                          "CSI g should produce VT100CSI_TBC token")
        }
    }

    func test_deviceStatusReport_CSI_n() {
        // ESC [ n - Device Status Report (DSR)
        let data = Data([0x1B, 0x5B, 0x6E])  // ESC [ n
        let tokens = adapter.parse(data: data)

        XCTAssertFalse(tokens.isEmpty, "CSI n should produce at least one token")
        if let token = tokens.first {
            XCTAssertEqual(token.type, VT100CSI_DSR,
                          "CSI n should produce VT100CSI_DSR token")
        }
    }

    // MARK: - Comparison Function Tests

    func test_compareTokens_matchingTokens_returnsTrue() {
        // Parse the same data
        let data = "Hello".data(using: .utf8)!
        let tokens1 = adapter.parse(data: data)

        // Reset and parse again
        adapter.reset()
        let tokens2 = adapter.parse(data: data)

        let result = DTermCoreParserAdapter.compareTokens(tokens1, with: tokens2, inputData: data)
        XCTAssertTrue(result, "Identical tokens should match")
    }

    func test_compareTokens_differentCount_returnsFalse() {
        // Create token arrays with different counts:
        // "Hello" produces 1 ASCII token
        // "Hello\n" produces 2 tokens: ASCII + control (LF)
        let data1 = "Hello".data(using: .utf8)!
        let tokens1 = adapter.parse(data: data1)

        adapter.reset()
        let data2 = "Hello\n".data(using: .utf8)!
        let tokens2 = adapter.parse(data: data2)

        let result = DTermCoreParserAdapter.compareTokens(tokens1, with: tokens2, inputData: data1)
        XCTAssertFalse(result, "Different token counts should not match")
    }
}
