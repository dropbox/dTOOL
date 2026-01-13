// DTermCoreParserAdapter.swift
// Adapter from dterm-core parser events to VT100Token objects
//
// This allows running the dterm-core parser in parallel with iTerm2's VT100Parser
// for comparison and gradual migration.
//
// The adapter translates dterm_action_t events (PRINT, EXECUTE, CSI, ESC, OSC)
// into VT100Token objects that iTerm2's VT100Terminal can process.
//
// ## FFI Capabilities (dterm-core commit #150+)
//
// The dterm_action_t struct contains:
// - action_type: PRINT, EXECUTE, CSI, ESC, OSC
// - byte: Character for Print, control byte for Execute
// - final_byte: Final byte for CSI/ESC sequences
// - param_count: Number of CSI parameters
// - params[16]: CSI parameter values
//
// OSC and DCS payloads are handled via separate callbacks:
// - dterm_terminal_set_clipboard_callback for OSC 52
// - dterm_terminal_set_dcs_callback for DCS sequences (Sixel, DECRQSS, etc.)
//
// ## Usage
//
// This adapter can be used for:
// - Terminal output (printing, cursor movement, SGR, modes)
// - Comparison validation with logging of mismatches
// - Performance benchmarking of parser throughput

import Foundation

/// Adapter that converts dterm-core parser events to VT100Token objects.
///
/// Usage:
/// ```swift
/// let adapter = DTermCoreParserAdapter()
/// let tokens = adapter.parse(data: ptyData)
/// // tokens can be compared with VT100Parser output
/// ```
@objc public class DTermCoreParserAdapter: NSObject {
    private var parser: OpaquePointer?

    /// Accumulated tokens from the last parse operation.
    private var parsedTokens: [VT100Token] = []

    /// Buffer for accumulating ASCII string data.
    private var asciiBuffer: [UInt8] = []

    /// Whether the adapter is enabled.
    @objc public var enabled: Bool = true

    public override init() {
        super.init()
        parser = dterm_parser_new()
    }

    deinit {
        if let parser = parser {
            dterm_parser_free(parser)
        }
    }

    /// Reset the parser to initial state.
    @objc public func reset() {
        if let parser = parser {
            dterm_parser_reset(parser)
        }
        parsedTokens.removeAll()
        asciiBuffer.removeAll()
    }

    /// Parse data and return VT100Token objects.
    ///
    /// - Parameter data: Raw PTY data bytes
    /// - Returns: Array of VT100Token objects representing parsed sequences
    @objc public func parse(data: Data) -> [VT100Token] {
        guard enabled, let parser = parser else { return [] }

        parsedTokens.removeAll(keepingCapacity: true)
        asciiBuffer.removeAll(keepingCapacity: true)

        data.withUnsafeBytes { ptr in
            guard let baseAddress = ptr.baseAddress else { return }
            let bytePtr = baseAddress.assumingMemoryBound(to: UInt8.self)

            // Create an unmanaged reference to self for the callback
            let context = Unmanaged.passUnretained(self).toOpaque()

            dterm_parser_feed(
                parser,
                bytePtr,
                UInt(ptr.count),
                context,
                parserCallback
            )
        }

        // Flush any remaining ASCII buffer
        flushASCIIBuffer()

        return parsedTokens
    }

    /// Parse bytes directly (for ObjC compatibility).
    @objc public func parse(bytes: UnsafePointer<UInt8>, length: Int) -> [VT100Token] {
        guard enabled, let parser = parser else { return [] }

        parsedTokens.removeAll(keepingCapacity: true)
        asciiBuffer.removeAll(keepingCapacity: true)

        let context = Unmanaged.passUnretained(self).toOpaque()
        dterm_parser_feed(parser, bytes, UInt(length), context, parserCallback)

        flushASCIIBuffer()
        return parsedTokens
    }

    // MARK: - Private

    /// Flush accumulated ASCII characters as a VT100_ASCIISTRING token.
    private func flushASCIIBuffer() {
        guard !asciiBuffer.isEmpty else { return }

        let token = VT100Token()
        token.type = VT100_ASCIISTRING

        // Set ASCII bytes on the token
        asciiBuffer.withUnsafeMutableBufferPointer { buffer in
            token.setAsciiBytes(buffer.baseAddress, length: Int32(buffer.count))
        }

        parsedTokens.append(token)
        asciiBuffer.removeAll(keepingCapacity: true)
    }

    /// Handle a parsed action from dterm-core.
    fileprivate func handleAction(_ action: dterm_action_t) {
        switch action.action_type {
        case DTERM_ACTION_TYPE_T_PRINT:
            handlePrint(action)

        case DTERM_ACTION_TYPE_T_EXECUTE:
            handleExecute(action)

        case DTERM_ACTION_TYPE_T_CSI:
            // Flush any pending ASCII before CSI
            flushASCIIBuffer()
            handleCSI(action)

        case DTERM_ACTION_TYPE_T_ESC:
            flushASCIIBuffer()
            handleESC(action)

        case DTERM_ACTION_TYPE_T_OSC:
            // dterm-core commit #150+: OSC payloads handled via separate callback
            flushASCIIBuffer()

        default:
            // DCS and other sequences handled via separate dterm callbacks
            break
        }
    }

    /// Handle PRINT action (printable character).
    private func handlePrint(_ action: dterm_action_t) {
        let codepoint = action.byte

        // For ASCII characters, accumulate them
        if codepoint < 128 {
            asciiBuffer.append(UInt8(codepoint))
        } else {
            // Flush ASCII buffer first
            flushASCIIBuffer()

            // Create a VT100_STRING token for non-ASCII
            let token = VT100Token()
            token.type = VT100_STRING

            if let scalar = UnicodeScalar(codepoint) {
                token.string = String(Character(scalar))
            }

            parsedTokens.append(token)
        }
    }

    /// Handle EXECUTE action (control character).
    private func handleExecute(_ action: dterm_action_t) {
        // Flush any pending ASCII
        flushASCIIBuffer()

        let byte = UInt8(action.byte & 0xFF)

        // Control characters map directly to VT100CC_* types
        // The token type IS the control character value for C0 codes
        guard let token = VT100Token.newToken(forControlCharacter: byte) else {
            return
        }
        parsedTokens.append(token)
    }

    /// Handle CSI action.
    private func handleCSI(_ action: dterm_action_t) {
        let token = VT100Token()

        // dterm-core commit #150+: prefix_byte and intermediate_byte removed from action struct.
        // dterm-core handles these internally now. We pass 0 for compatibility.
        let finalByte = action.final_byte
        let paramCount = Int(action.param_count)

        // Get the CSI param structure
        let csi = token.csi!

        // Copy parameters from dterm-core array to CSI param array
        let params = action.params
        let maxParams = min(paramCount, Int(VT100CSIPARAM_MAX))
        if maxParams > 0 { csi.pointee.p.0 = Int32(params.0) }
        if maxParams > 1 { csi.pointee.p.1 = Int32(params.1) }
        if maxParams > 2 { csi.pointee.p.2 = Int32(params.2) }
        if maxParams > 3 { csi.pointee.p.3 = Int32(params.3) }
        if maxParams > 4 { csi.pointee.p.4 = Int32(params.4) }
        if maxParams > 5 { csi.pointee.p.5 = Int32(params.5) }
        if maxParams > 6 { csi.pointee.p.6 = Int32(params.6) }
        if maxParams > 7 { csi.pointee.p.7 = Int32(params.7) }
        if maxParams > 8 { csi.pointee.p.8 = Int32(params.8) }
        if maxParams > 9 { csi.pointee.p.9 = Int32(params.9) }
        if maxParams > 10 { csi.pointee.p.10 = Int32(params.10) }
        if maxParams > 11 { csi.pointee.p.11 = Int32(params.11) }
        if maxParams > 12 { csi.pointee.p.12 = Int32(params.12) }
        if maxParams > 13 { csi.pointee.p.13 = Int32(params.13) }
        if maxParams > 14 { csi.pointee.p.14 = Int32(params.14) }
        if maxParams > 15 { csi.pointee.p.15 = Int32(params.15) }
        csi.pointee.count = Int32(paramCount)

        // Map final byte to token type (prefix/intermediate handled by dterm-core)
        token.type = mapCSIToTokenType(
            finalByte: finalByte,
            prefixByte: 0,
            intermediateByte: 0,
            params: action.params,
            paramCount: paramCount
        )

        parsedTokens.append(token)
    }

    /// Handle ESC action.
    private func handleESC(_ action: dterm_action_t) {
        let token = VT100Token()
        let finalByte = action.final_byte

        // Map ESC final byte to token type
        token.type = mapESCToTokenType(finalByte: finalByte)

        // For SCS (Select Character Set) commands, store the character set designator
        // in the codeValue property. This allows VT100Terminal to determine if line
        // drawing mode should be enabled ('0') or disabled ('B' for USASCII).
        switch finalByte {
        case 0x30, 0x41, 0x42, 0x3C:  // '0', 'A', 'B', '<'
            // These are character set designators - store in codeValue
            token.codeValue = finalByte
        default:
            break
        }

        parsedTokens.append(token)
    }

    // NOTE: handleOSC and handleDCS removed in dterm-core commit #150+
    // OSC payloads: Use dterm_terminal_set_clipboard_callback
    // DCS payloads: Use dterm_terminal_set_dcs_callback

    /// Map CSI final byte to VT100Token type.
    ///
    /// - Parameters:
    ///   - finalByte: The final byte of the CSI sequence
    ///   - prefixByte: The prefix byte ('?' for DEC private, '>' for DA2, '=' for DA3, 0 for none)
    ///   - intermediateByte: The intermediate byte (' ' for DECSCUSR, '"' for DECSCA, etc., 0 for none)
    ///   - params: The CSI parameters tuple
    ///   - paramCount: Number of valid parameters
    /// - Returns: The corresponding VT100Token type
    private func mapCSIToTokenType(
        finalByte: UInt8,
        prefixByte: UInt8,
        intermediateByte: UInt8,
        params: (UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16),
        paramCount: Int
    ) -> VT100TerminalTokenType {
        // Handle DEC private mode sequences (prefix '?')
        if prefixByte == 0x3F {  // '?'
            return mapDECPrivateCSI(finalByte: finalByte, intermediateByte: intermediateByte)
        }

        // Handle secondary device attributes (prefix '>')
        if prefixByte == 0x3E && finalByte == 0x63 {  // '>' + 'c'
            return VT100CSI_DA2
        }

        // Handle tertiary device attributes (prefix '=')
        if prefixByte == 0x3D && finalByte == 0x63 {  // '=' + 'c'
            return VT100CSI_DA3
        }

        // Handle sequences with intermediate bytes
        if intermediateByte != 0 {
            return mapIntermediateCSI(finalByte: finalByte, intermediateByte: intermediateByte)
        }

        // Standard CSI sequences (no prefix, no intermediate)
        switch finalByte {
        case 0x41: // 'A' - Cursor Up
            return VT100CSI_CUU
        case 0x42: // 'B' - Cursor Down
            return VT100CSI_CUD
        case 0x43: // 'C' - Cursor Forward
            return VT100CSI_CUF
        case 0x44: // 'D' - Cursor Backward
            return VT100CSI_CUB
        case 0x45: // 'E' - Cursor Next Line
            return VT100CSI_CNL
        case 0x46: // 'F' - Cursor Preceding Line
            return VT100CSI_CPL
        case 0x47: // 'G' - Cursor Character Absolute
            return ANSICSI_CHA
        case 0x48: // 'H' - Cursor Position
            return VT100CSI_CUP
        case 0x49: // 'I' - Cursor Forward Tabulation
            return VT100CSI_CHT
        case 0x4A: // 'J' - Erase in Display
            return VT100CSI_ED
        case 0x4B: // 'K' - Erase in Line
            return VT100CSI_EL
        case 0x4C: // 'L' - Insert Lines
            return XTERMCC_INSLN
        case 0x4D: // 'M' - Delete Lines
            return XTERMCC_DELLN
        case 0x50: // 'P' - Delete Characters
            return XTERMCC_DELCH
        case 0x53: // 'S' - Scroll Up
            return XTERMCC_SU
        case 0x54: // 'T' - Scroll Down
            return XTERMCC_SD
        case 0x58: // 'X' - Erase Characters
            return ANSICSI_ECH
        case 0x5A: // 'Z' - Cursor Backward Tabulation
            return ANSICSI_CBT
        case 0x60: // '`' - Character Position Absolute
            return ANSICSI_CHA
        case 0x61: // 'a' - Character Position Relative
            return VT100CSI_HPR
        case 0x62: // 'b' - Repeat
            return VT100CSI_REP
        case 0x63: // 'c' - Device Attributes (primary)
            return VT100CSI_DA
        case 0x64: // 'd' - Line Position Absolute
            return ANSICSI_VPA
        case 0x65: // 'e' - Line Position Relative
            return ANSICSI_VPR
        case 0x66: // 'f' - Horizontal and Vertical Position
            return VT100CSI_HVP
        case 0x67: // 'g' - Tab Clear
            return VT100CSI_TBC
        case 0x68: // 'h' - Set Mode (ANSI)
            return VT100CSI_SM
        case 0x69: // 'i' - Media Copy (print)
            return ANSICSI_PRINT
        case 0x6C: // 'l' - Reset Mode (ANSI)
            return VT100CSI_RM
        case 0x6D: // 'm' - Select Graphic Rendition
            return VT100CSI_SGR
        case 0x6E: // 'n' - Device Status Report
            return VT100CSI_DSR
        case 0x72: // 'r' - Set Scrolling Region
            return VT100CSI_DECSTBM
        case 0x73: // 's' - Save Cursor (ANSI.SYS) or Set Left/Right Margin
            return VT100CSI_DECSLRM_OR_ANSICSI_SCP
        case 0x74: // 't' - Window manipulation
            return XTERMCC_WINDOWSIZE
        case 0x75: // 'u' - Restore Cursor (ANSI.SYS)
            return ANSICSI_RCP
        case 0x40: // '@' - Insert Characters
            return VT100CSI_ICH
        default:
            return VT100_NOTSUPPORT
        }
    }

    /// Map DEC private mode CSI sequences (CSI ? ...).
    ///
    /// These sequences use the '?' prefix to indicate DEC-specific functionality.
    private func mapDECPrivateCSI(finalByte: UInt8, intermediateByte: UInt8) -> VT100TerminalTokenType {
        switch finalByte {
        case 0x68: // 'h' - DEC Private Mode Set (DECSET)
            return VT100CSI_DECSET
        case 0x6C: // 'l' - DEC Private Mode Reset (DECRST)
            return VT100CSI_DECRST
        case 0x4A: // 'J' - Selective Erase in Display (DECSED)
            return VT100CSI_DECSED
        case 0x4B: // 'K' - Selective Erase in Line (DECSEL)
            return VT100CSI_DECSEL
        case 0x6E: // 'n' - DEC Device Status Report (DEC DSR)
            return VT100CSI_DECDSR
        default:
            return VT100_NOTSUPPORT
        }
    }

    /// Map CSI sequences with intermediate bytes.
    ///
    /// Intermediate bytes modify the meaning of the final byte.
    private func mapIntermediateCSI(finalByte: UInt8, intermediateByte: UInt8) -> VT100TerminalTokenType {
        switch intermediateByte {
        case 0x20: // ' ' (space)
            switch finalByte {
            case 0x71: // 'q' - Set Cursor Style (DECSCUSR)
                return VT100CSI_DECSCUSR
            default:
                return VT100_NOTSUPPORT
            }

        case 0x21: // '!'
            switch finalByte {
            case 0x70: // 'p' - Soft Terminal Reset (DECSTR)
                return VT100CSI_DECSTR
            default:
                return VT100_NOTSUPPORT
            }

        case 0x22: // '"'
            switch finalByte {
            case 0x71: // 'q' - Set Character Protection Attribute (DECSCA)
                return VT100CSI_DECSCA
            default:
                return VT100_NOTSUPPORT
            }

        case 0x24: // '$'
            switch finalByte {
            case 0x70: // 'p' - Request DEC Private Mode (DECRQM)
                // Note: Could be DECRQM_DEC or DECRQM_ANSI depending on context
                // Using DEC variant as this is in the $ intermediate handling
                return VT100CSI_DECRQM_DEC
            case 0x7A: // 'z' - Erase Rectangular Area (DECERA)
                return VT100CSI_DECERA
            case 0x78: // 'x' - Fill Rectangular Area (DECFRA)
                return VT100CSI_DECFRA
            case 0x76: // 'v' - Copy Rectangular Area (DECCRA)
                return VT100CSI_DECCRA
            case 0x7B: // '{' - Selective Erase Rectangular Area (DECSERA)
                return VT100CSI_DECSERA
            default:
                return VT100_NOTSUPPORT
            }

        default:
            return VT100_NOTSUPPORT
        }
    }

    /// Map ESC final byte to VT100Token type.
    ///
    /// - Note: Character set designation sequences (ESC ( X, ESC ) X) are mapped
    ///   to SCS0/SCS1 with the character set stored in the `code` field.
    private func mapESCToTokenType(finalByte: UInt8) -> VT100TerminalTokenType {
        switch finalByte {
        // Cursor save/restore
        case 0x37: // '7' - Save Cursor (DECSC)
            return VT100CSI_DECSC
        case 0x38: // '8' - Restore Cursor (DECRC)
            return VT100CSI_DECRC

        // Keypad modes
        case 0x3D: // '=' - Application Keypad Mode (DECKPAM)
            return VT100CSI_DECKPAM
        case 0x3E: // '>' - Normal Keypad Mode (DECKPNM)
            return VT100CSI_DECKPNM

        // Index/line operations
        case 0x44: // 'D' - Index (IND) - move down, scroll if at bottom
            return VT100CSI_IND
        case 0x45: // 'E' - Next Line (NEL) - CR+LF with scroll
            return VT100CSI_NEL
        case 0x4D: // 'M' - Reverse Index (RI) - move up, scroll if at top
            return VT100CSI_RI

        // Tab operations
        case 0x48: // 'H' - Horizontal Tab Set (HTS)
            return VT100CSI_HTS

        // Full reset
        case 0x63: // 'c' - Reset to Initial State (RIS)
            return VT100CSI_RIS

        // Double-width/height (DEC specific)
        case 0x23: // '#' - Various DEC line attributes (DECDHL, DECSWL, DECDWL)
            // Note: The actual attribute depends on the next byte which we don't have
            // The full sequence is ESC # N where N determines the attribute
            return VT100_NOTSUPPORT

        // Character set designation - these depend on the intermediate byte
        // ESC ( X -> designate G0 to character set X
        // ESC ) X -> designate G1 to character set X
        // The intermediate byte ('(' or ')') determines the slot
        // The final byte determines the character set
        // Common character sets:
        //   '0' = DEC Special Character and Line Drawing
        //   'A' = UK
        //   'B' = USASCII
        //   '<' = DEC Supplemental
        case 0x30: // '0' - DEC Special Graphics (line drawing)
            return VT100CSI_SCS0  // G0
        case 0x41: // 'A' - UK character set
            return VT100CSI_SCS0
        case 0x42: // 'B' - USASCII
            return VT100CSI_SCS0
        case 0x3C: // '<' - DEC Supplemental
            return VT100CSI_SCS0

        // Shift in/out (invoke G0/G1 into GL)
        case 0x0E: // SO - Shift Out (invoke G1)
            return VT100_NOTSUPPORT  // Handled as control character
        case 0x0F: // SI - Shift In (invoke G0)
            return VT100_NOTSUPPORT  // Handled as control character

        // Single shift (SS2, SS3 - invoke G2/G3 for next character)
        case 0x4E: // 'N' - Single Shift 2 (SS2)
            return VT100_NOTSUPPORT
        case 0x4F: // 'O' - Single Shift 3 (SS3)
            return VT100_NOTSUPPORT

        // Locking shifts (invoke G2/G3 into GL/GR)
        case 0x6E: // 'n' - LS2 (Locking Shift 2)
            return VT100_NOTSUPPORT
        case 0x6F: // 'o' - LS3 (Locking Shift 3)
            return VT100_NOTSUPPORT
        case 0x7C: // '|' - LS3R (Locking Shift 3 Right)
            return VT100_NOTSUPPORT
        case 0x7D: // '}' - LS2R (Locking Shift 2 Right)
            return VT100_NOTSUPPORT
        case 0x7E: // '~' - LS1R (Locking Shift 1 Right)
            return VT100_NOTSUPPORT

        default:
            return VT100_NOTSUPPORT
        }
    }
}

// MARK: - Comparison Logging

extension DTermCoreParserAdapter {
    /// Compare dterm-core tokens with iTerm2 tokens and log mismatches.
    ///
    /// This is useful for validating the adapter during development.
    ///
    /// - Parameters:
    ///   - dtermTokens: Tokens produced by this adapter
    ///   - iterm2Tokens: Tokens produced by VT100Parser
    ///   - inputData: The original input data (for context in logs)
    /// - Returns: true if tokens match, false if there are mismatches
    @objc public static func compareTokens(_ dtermTokens: [VT100Token],
                                           with iterm2Tokens: [VT100Token],
                                           inputData: Data) -> Bool {
        var matches = true

        // Quick count check
        if dtermTokens.count != iterm2Tokens.count {
            NSLog("[DTermCore] Token count mismatch: dterm=%d, iterm2=%d for %d bytes input",
                  dtermTokens.count, iterm2Tokens.count, inputData.count)
            matches = false
        }

        // Compare token types
        let minCount = min(dtermTokens.count, iterm2Tokens.count)
        for i in 0..<minCount {
            let dtermToken = dtermTokens[i]
            let iterm2Token = iterm2Tokens[i]

            if dtermToken.type != iterm2Token.type {
                NSLog("[DTermCore] Token type mismatch at index %d: dterm=%d, iterm2=%d",
                      i, dtermToken.type.rawValue, iterm2Token.type.rawValue)
                matches = false
            }
        }

        return matches
    }
}

// MARK: - C Callback

/// C callback function for dterm_parser_feed.
/// dterm-core commit #150+: The action is now passed by value (not pointer).
private func parserCallback(context: UnsafeMutableRawPointer?, action: dterm_action_t) {
    guard let context = context else { return }
    let adapter = Unmanaged<DTermCoreParserAdapter>.fromOpaque(context).takeUnretainedValue()
    adapter.handleAction(action)
}
