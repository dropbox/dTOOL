/**
 * TTVT100Emulator.decodeData:
 *
 * Decompiled from Terminal.app using radare2 + r2ghidra
 * Address: 0x10001b388
 *
 * This is the main parser entry point that processes incoming PTY data.
 * Uses a translation table-based state machine for O(1) byte classification.
 */

ulong method_TTVT100Emulator_decodeData_(int64_t self, uchar data[16], ulong arg3)
{
    uint8_t *pData;
    uint32_t *paramBuffer;
    uint16_t *puVar4;
    int64_t *translationTable;
    uint8_t byte;
    uint8_t nextByte;
    int state;
    uint64_t offset;
    int64_t screenObj;
    uint32_t paramCount;

    // Get data pointer and length from NSData object
    pData = /* data bytes */;
    uint64_t dataLength = /* data length */;

    // Main parsing loop
    offset = 0;
    do {
        int currentOffset = offset;
        offset = currentOffset + 1;

        // UTF-8 multi-byte sequence handling
        if (/* UTF-8 flag set */) {
            byte = pData[currentOffset];
            if (0xffffffcc < byte - 0xf5 && offset != dataLength) {
                nextByte = pData[offset];

                // 3-byte UTF-8 (0xE0-0xEF)
                if ((byte & 0xf0) == 0xe0) {
                    // Validate continuation bytes
                    if (((" 000000000000\x1000"[byte & 0xf] >> (nextByte >> 5) & 1) != 0) &&
                        (currentOffset + 2 != dataLength)) {
                        byte = pData[currentOffset + 2];
                        offset = currentOffset + 3;
                        if (-0x41 < byte) {
                            offset = currentOffset + 2;
                        }
                    }
                }
                // 2-byte UTF-8 (0xC0-0xDF)
                else if (byte < 0xe0) {
                    offset = currentOffset + 2;
                    if (-0x41 < nextByte) {
                        offset = currentOffset + 1;
                    }
                }
                // 4-byte UTF-8 (0xF0-0xF7)
                else if (/* validation table lookup */) {
                    if (pData[currentOffset + 2] < -0x40 &&
                        currentOffset + 3 != dataLength) {
                        byte = pData[currentOffset + 3];
                        offset = currentOffset + 4;
                        if (-0x41 < byte) {
                            offset = currentOffset + 3;
                        }
                    }
                }
            }
        }

        currentOffset = offset;
        if (dataLength <= currentOffset) break;

        // State table lookup - the core of the parser
        byte = pData[currentOffset];
        translationTable = *(self + 0x30);  // Get translation table pointer

        // Direct lookup for ASCII (< 0x81)
        if (pData[currentOffset] < 0x81) {
            translationTable = translationTable + byte * 8;
        } else {
            // Extended character handling
            translationTable = translationTable + 0x400;
        }

        state = *translationTable;
        *(self + 0x8) = state;  // Store current state

    } while (state == 0x5d);  // Continue while in OSC state

    // State machine switch - handle escape sequences
    switch (state) {
        case 0x59:  // 'Y' - Direct cursor addressing (VT52)
            if (*(self + 0x38) == 0x1000eabc8) {
                paramCount = *(self + 0x4984);
                // Handle VT52 cursor positioning
            }
            break;

        case 0x5a:  // 'Z' - Identify terminal
            fcn_100094980(self, /* args */);
            break;

        case 0x5b:  // '[' - CSI (Control Sequence Introducer)
            if (*(self + 0x44) != 0) {
                // Clear parameter buffer
                memset(paramBuffer, 0xff, *(self + 0x44) << 2);
            }
            *(self + 0x44) = 1;  // Reset param count

            if (*(self + 200) != 0) {
                objc_release(/* previous string */);
                *(self + 200) = 0;
            }
            // Set state to CSI parameter collection
            state = 0x1000e9fb0;
            break;

        case 0x5c:  // '\' - ST (String Terminator)
            // Allocate string for collected data
            *(self + 200) = objc_alloc_init(NSMutableString);
            state = 0x1000ea3b8;
            break;

        case 0x5d:  // ']' - OSC (Operating System Command)
            // Parse OSC sequence
            int oscLength = 0;
            do {
                int i = oscLength;
                oscLength = i + 1;

                // Continue until ST or BEL
                if (/* UTF-8 validation */) {
                    byte = pData[i];
                    // Multi-byte handling...
                }

                i = oscLength;
                if (dataLength <= i) break;

                byte = pData[i];
                translationTable = *(self + 0x30) + 0x400;
                if (pData[i] < 0x81) {
                    translationTable = *(self + 0x30) + byte * 8;
                }
                state = *translationTable;
                *(self + 0x8) = state;

            } while (state == 0x5d);

            if (0 < oscLength) {
                // Create NSString from collected OSC data
                NSData *oscData = objc_alloc(NSData);
                // Initialize with UTF-8 encoding
                fcn_100088bc0(oscData, pData, oscLength, 4 /* NSUTF8StringEncoding */);
                oscData = objc_autorelease(oscData);

                // Append to OSC string buffer
                fcn_100083a00(*(self + 200), oscData);
            }
            break;

        case 0x5e:  // '^' - PM (Privacy Message)
            state = 0x1000ea7c0;
            break;

        case 0x5f:  // '_' - APC (Application Program Command)
            fcn_10008c320(self);
            break;

        case 0x60:  // '`' - HPA (Horizontal Position Absolute)
            screenObj = *(self + 0x10);
            fcn_10008f700(screenObj, /* args */, 1);
            break;

        case 0x61:  // 'a' - HPR (Horizontal Position Relative)
            screenObj = *(self + 0x10);
            fcn_10008f700(screenObj, /* args */, 2);
            break;

        case 0x62:  // 'b' - REP (Repeat)
            screenObj = *(self + 0x10);
            fcn_10008f700(screenObj, /* args */, 0);
            break;

        case 99:    // 'c' - DA (Device Attributes)
            screenObj = *(self + 0x10);
            fcn_10008f700(screenObj, /* args */, 3);
            break;

        case 100:   // 'd' - VPA (Vertical Position Absolute)
            fcn_100092900(*(self + 0x28));
            // Complex CSI parameter handling...
            int paramIndex = *(self + 0x48);

            switch (paramIndex) {
                case 1:  // Cursor up
                    fcn_100086720(/* args */);
                    break;
                case 2:  // Cursor down
                    fcn_10008b360(/* args */);
                    break;
                case 3:  // Cursor forward
                    if (2 < *(self + 0x44)) {
                        fcn_100086400(/* args */);
                        // Calculate new position...
                    }
                    break;
                case 4:  // Cursor backward
                    if (2 < *(self + 0x44)) {
                        // Handle cursor movement with bounds...
                    }
                    break;
                case 5:  // CNL (Cursor Next Line)
                    fcn_10008bda0(/* args */);
                    break;
                case 6:  // CPL (Cursor Previous Line)
                    fcn_10008bd80(/* args */);
                    break;
                case 8:  // CUP (Cursor Position)
                    if (2 < *(self + 0x44)) {
                        fcn_100086400(/* args */);
                        // Position cursor at row, col
                    }
                    break;
                case 9:  // CHT (Cursor Horizontal Tab)
                    if (1 < *(self + 0x44)) {
                        if (*(self + 0x4c) == 1) {
                            fcn_10008a180(/* args */);
                        }
                    }
                    break;
                case 0xb:  // CTC (Cursor Tab Control)
                    fcn_10008a120(/* args */);
                    break;
                case 0xd:  // Report cursor position
                    fcn_100086400(/* args */);
                    // Send position report...
                    break;
                case 0xe:  // Scroll region
                    fcn_100083060(*(self + 0x28));
                    break;
                case 0x12: // Other CSI
                    fcn_10008d9e0(*(self + 0x10));
                    fcn_100085000(*(self + 0x10));
                    break;
            }
            break;

        case 0x65:  // 'e' - VPR (Vertical Position Relative)
            // Handle with bounds checking
            uint32_t value = *paramBuffer;
            uint32_t newValue = value + 2;

            if (0x9fffff < value) {
                value = 0xa00000;  // Clamp to max
            }
            if (newValue < 3) {
                value = 1;  // Minimum value
            }
            *paramBuffer = value;

            // Allocate buffer if needed
            if (-1 < *(self + 0x49a0)) {
                void *buf = malloc_type_malloc(value, 0x100004077774924);
                memset(buf, *(self + 0x49a0), *(self + 0x48));
                // Process buffer...
            }
            break;

        // ... additional cases for other escape sequences
    }

    // Update translation table pointer
    *(self + 0x30) = state;

    return /* result */;
}

/*
 * Key observations:
 *
 * 1. Translation table at offset 0x30 provides O(1) lookup
 * 2. Table has 256 entries for ASCII, extended section at +0x400
 * 3. Each entry is 8 bytes (pointer to state/handler)
 * 4. UTF-8 validation done inline during parse
 * 5. OSC strings collected in NSMutableString at offset 200
 * 6. CSI parameters stored in buffer, count at offset 0x44
 * 7. Current state stored at offset 0x8
 */
