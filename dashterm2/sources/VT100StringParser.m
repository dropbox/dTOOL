//
//  VT100StringParser.m
//  iTerm
//
//  Created by George Nachman on 3/2/14.
//
//

#import "VT100StringParser.h"

#import "DebugLogging.h"
#import "NSStringITerm.h"
#import "ScreenChar.h"

#if defined(__ARM_NEON) || defined(__ARM_NEON__)
#import <arm_neon.h>
#define VT100_HAS_NEON 1
#else
#define VT100_HAS_NEON 0
#endif

#if defined(__SSE2__)
#import <emmintrin.h>
#define VT100_HAS_SSE2 1
#else
#define VT100_HAS_SSE2 0
#endif

#if defined(__AVX2__)
#import <immintrin.h>
#define VT100_HAS_AVX2 1
#else
#define VT100_HAS_AVX2 0
#endif

NS_INLINE int VT100PrintableASCIIRunLength(const unsigned char *bytes, int length) {
    int processed = 0;

#if VT100_HAS_NEON
    if (length >= 16) {
        const uint8x16_t lowerBound = vdupq_n_u8(0x20);
        const uint8x16_t upperBound = vdupq_n_u8(0x80);

        while (length - processed >= 16) {
            const uint8x16_t chunk = vld1q_u8(bytes + processed);
            const uint8x16_t below = vcltq_u8(chunk, lowerBound);
            const uint8x16_t above = vcgeq_u8(chunk, upperBound);
            const uint8x16_t invalid = vorrq_u8(below, above);
            const uint64x2_t invalid64 = vreinterpretq_u64_u8(invalid);
            const uint64_t lanes = vgetq_lane_u64(invalid64, 0) | vgetq_lane_u64(invalid64, 1);
            if (lanes == 0) {
                processed += 16;
                continue;
            }

            uint8_t invalidBytes[16];
            vst1q_u8(invalidBytes, invalid);
            for (int i = 0; i < 16; i++) {
                if (invalidBytes[i]) {
                    return processed + i;
                }
            }
            return processed + 16;
        }
    }
#elif VT100_HAS_AVX2
    // AVX2 vectorized path for Intel: process 32 bytes at a time.
    // Valid printable ASCII is in range [0x20, 0x7f].
    // We detect invalid bytes as: (byte < 0x20) OR (byte >= 0x80).
    if (length >= 32) {
        const __m256i lowerBound = _mm256_set1_epi8(0x20);
        const __m256i highBit = _mm256_set1_epi8((char)0x80);

        while (length - processed >= 32) {
            const __m256i chunk = _mm256_loadu_si256((const __m256i *)(bytes + processed));
            // Check byte < 0x20 (signed compare)
            const __m256i belowMin = _mm256_cmpgt_epi8(lowerBound, chunk);
            // Check byte >= 0x80 by testing if high bit is set
            const __m256i aboveMax = _mm256_cmpeq_epi8(_mm256_and_si256(chunk, highBit), highBit);
            // Combine: invalid if below min OR above max
            const __m256i invalid = _mm256_or_si256(belowMin, aboveMax);
            const int mask = _mm256_movemask_epi8(invalid);

            if (mask == 0) {
                processed += 32;
                continue;
            }

            // Find first invalid byte using bit scan
            return processed + __builtin_ctz(mask);
        }
    }
    // Fall through to scalar for remaining 0-31 bytes
#elif VT100_HAS_SSE2
    // SSE2 vectorized path for Intel: process 16 bytes at a time.
    // Valid printable ASCII is in range [0x20, 0x7f].
    // We detect invalid bytes as: (byte < 0x20) OR (byte >= 0x80).
    if (length >= 16) {
        const __m128i lowerBound = _mm_set1_epi8(0x20);
        const __m128i highBit = _mm_set1_epi8((char)0x80);

        while (length - processed >= 16) {
            const __m128i chunk = _mm_loadu_si128((const __m128i *)(bytes + processed));
            // Check byte < 0x20 (signed compare works since 0x00-0x1f and 0x20 are all positive)
            const __m128i belowMin = _mm_cmplt_epi8(chunk, lowerBound);
            // Check byte >= 0x80 by testing if high bit is set
            const __m128i aboveMax = _mm_cmpeq_epi8(_mm_and_si128(chunk, highBit), highBit);
            // Combine: invalid if below min OR above max
            const __m128i invalid = _mm_or_si128(belowMin, aboveMax);
            const int mask = _mm_movemask_epi8(invalid);

            if (mask == 0) {
                processed += 16;
                continue;
            }

            // Find first invalid byte using bit scan
            return processed + __builtin_ctz(mask);
        }
    }
#endif

    while (processed < length) {
        unsigned char c = bytes[processed];
        if (c >= 0x20 && c <= 0x7f) {
            processed++;
            continue;
        }
        break;
    }

    return processed;
}

static void DecodeUTF8Bytes(VT100ByteStreamConsumer *consumer, VT100Token *token) {
    int utf8DecodeResult;
    int consumed = 0;

    VT100ByteStreamCursor cursor = VT100ByteStreamConsumerGetCursor(consumer);

    while (true) {
        int codePoint = 0;
        utf8DecodeResult = decode_utf8_char(VT100ByteStreamCursorGetPointer(&cursor),
                                            VT100ByteStreamCursorGetSize(&cursor), &codePoint);
        // Stop on error or end of stream.
        if (utf8DecodeResult <= 0) {
            break;
        }
        // Intentionally break out at ASCII characters. They are
        // processed separately, e.g. they might get converted into
        // line drawing characters.
        if (codePoint < 0x80) {
            break;
        }
        VT100ByteStreamCursorAdvance(&cursor, utf8DecodeResult);
        consumed += utf8DecodeResult;
    }

    if (consumed > 0) {
        // If some characters were successfully decoded, just return them
        // and ignore the error or end of stream for now.
        VT100ByteStreamConsumerSetConsumed(consumer, consumed);
        // BUG-f1378: Removed redundant assert - consumed > 0 implies consumed >= 0
        token->type = VT100_STRING;
    } else {
        // Report error or waiting state.
        if (utf8DecodeResult == 0) {
            token->type = VT100_WAIT;
        } else {
            VT100ByteStreamConsumerSetConsumed(consumer, -utf8DecodeResult);
            token->type = VT100_INVALID_SEQUENCE;
        }
    }
}


static void DecodeEUCCNBytes(VT100ByteStreamConsumer *consumer, VT100Token *token) {
    VT100ByteStreamCursor cursor = VT100ByteStreamConsumerGetCursor(consumer);
    int consumed = 0;
    int size;
    while ((size = VT100ByteStreamCursorGetSize(&cursor)) > 0) {
        unsigned char c1 = VT100ByteStreamCursorPeek(&cursor);

        if (iseuccn(c1) && size > 1) {
            unsigned char c2 = VT100ByteStreamCursorPeekOffset(&cursor, 1);
            if ((c2 >= 0x40 && c2 <= 0x7e) || (c2 >= 0x80 && c2 <= 0xfe)) {
                VT100ByteStreamCursorAdvance(&cursor, 2);
                consumed += 2;
            } else {
                // replace invalid second byte
                VT100ByteStreamCursorWrite(&cursor, ONECHAR_UNKNOWN);
                VT100ByteStreamCursorAdvance(&cursor, 1);
                consumed += 1;
            }
        } else {
            break;
        }
    }

    if (consumed == 0) {
        VT100ByteStreamConsumerSetConsumed(consumer, 0);
        token->type = VT100_WAIT;
    } else {
        VT100ByteStreamConsumerSetConsumed(consumer, consumed);
        token->type = VT100_STRING;
    }
}

static void DecodeBIG5Bytes(VT100ByteStreamConsumer *consumer, VT100Token *token) {
    VT100ByteStreamCursor cursor = VT100ByteStreamConsumerGetCursor(consumer);
    int consumed = 0;

    int size;
    while ((size = VT100ByteStreamCursorGetSize(&cursor)) > 0) {
        unsigned char c1 = VT100ByteStreamCursorPeek(&cursor);

        if (isbig5(c1) && size > 1) {
            unsigned char c2 = VT100ByteStreamCursorPeekOffset(&cursor, 1);
            if ((c2 >= 0x40 && c2 <= 0x7e) || (c2 >= 0xa1 && c2 <= 0xfe)) {
                VT100ByteStreamCursorAdvance(&cursor, 2);
                consumed += 2;
            } else {
                VT100ByteStreamCursorWrite(&cursor, ONECHAR_UNKNOWN);
                VT100ByteStreamCursorAdvance(&cursor, 1);
                consumed += 1;
            }
        } else {
            break;
        }
    }

    if (consumed == 0) {
        VT100ByteStreamConsumerSetConsumed(consumer, 0);
        token->type = VT100_WAIT;
    } else {
        VT100ByteStreamConsumerSetConsumed(consumer, consumed);
        token->type = VT100_STRING;
    }
}

static void DecodeEUCJPBytes(VT100ByteStreamConsumer *consumer, VT100Token *token) {
    VT100ByteStreamCursor cursor = VT100ByteStreamConsumerGetCursor(consumer);
    int consumed = 0;

    int size;
    while ((size = VT100ByteStreamCursorGetSize(&cursor)) > 0) {
        unsigned char c1 = VT100ByteStreamCursorPeek(&cursor);

        if (size > 1 && c1 == 0x8e) {
            VT100ByteStreamCursorAdvance(&cursor, 2);
            consumed += 2;
        } else if (size > 2 && c1 == 0x8f) {
            VT100ByteStreamCursorAdvance(&cursor, 3);
            consumed += 3;
        } else if (size > 1 && c1 >= 0xa1 && c1 <= 0xfe) {
            VT100ByteStreamCursorAdvance(&cursor, 2);
            consumed += 2;
        } else {
            break;
        }
    }

    if (consumed == 0) {
        VT100ByteStreamConsumerSetConsumed(consumer, 0);
        token->type = VT100_WAIT;
    } else {
        VT100ByteStreamConsumerSetConsumed(consumer, consumed);
        token->type = VT100_STRING;
    }
}


static void DecodeSJISBytes(VT100ByteStreamConsumer *consumer, VT100Token *token) {
    VT100ByteStreamCursor cursor = VT100ByteStreamConsumerGetCursor(consumer);
    int consumed = 0;

    while (VT100ByteStreamCursorGetSize(&cursor) > 0) {
        unsigned char c1 = VT100ByteStreamCursorPeek(&cursor);
        int size = VT100ByteStreamCursorGetSize(&cursor);

        if (issjiskanji(c1) && size > 1) {
            VT100ByteStreamCursorAdvance(&cursor, 2);
            consumed += 2;
        } else if (c1 >= 0x80) {
            VT100ByteStreamCursorAdvance(&cursor, 1);
            consumed += 1;
        } else {
            break;
        }
    }

    if (consumed == 0) {
        VT100ByteStreamConsumerSetConsumed(consumer, 0);
        token->type = VT100_WAIT;
    } else {
        VT100ByteStreamConsumerSetConsumed(consumer, consumed);
        token->type = VT100_STRING;
    }
}

static void DecodeEUCKRBytes(VT100ByteStreamConsumer *consumer, VT100Token *token) {
    VT100ByteStreamCursor cursor = VT100ByteStreamConsumerGetCursor(consumer);
    int consumed = 0;

    int size;
    while ((size = VT100ByteStreamCursorGetSize(&cursor)) > 0) {
        unsigned char c1 = VT100ByteStreamCursorPeek(&cursor);

        if (iseuckr(c1) && size > 1) {
            VT100ByteStreamCursorAdvance(&cursor, 2);
            consumed += 2;
        } else {
            break;
        }
    }

    if (consumed == 0) {
        VT100ByteStreamConsumerSetConsumed(consumer, 0);
        token->type = VT100_WAIT;
    } else {
        VT100ByteStreamConsumerSetConsumed(consumer, consumed);
        token->type = VT100_STRING;
    }
}

static void DecodeCP949Bytes(VT100ByteStreamConsumer *consumer, VT100Token *token) {
    VT100ByteStreamCursor cursor = VT100ByteStreamConsumerGetCursor(consumer);
    int consumed = 0;

    int size;
    while ((size = VT100ByteStreamCursorGetSize(&cursor)) > 0) {
        unsigned char c1 = VT100ByteStreamCursorPeek(&cursor);

        if (iscp949(c1) && size > 1) {
            VT100ByteStreamCursorAdvance(&cursor, 2);
            consumed += 2;
        } else {
            break;
        }
    }

    if (consumed == 0) {
        VT100ByteStreamConsumerSetConsumed(consumer, 0);
        token->type = VT100_WAIT;
    } else {
        VT100ByteStreamConsumerSetConsumed(consumer, consumed);
        token->type = VT100_STRING;
    }
}

static void DecodeOtherBytes(VT100ByteStreamConsumer *consumer, VT100Token *token) {
    VT100ByteStreamCursor cursor = VT100ByteStreamConsumerGetCursor(consumer);
    int consumed = 0;

    while (VT100ByteStreamCursorGetSize(&cursor) > 0) {
        unsigned char c = VT100ByteStreamCursorPeek(&cursor);
        if (c >= 0x80) {
            VT100ByteStreamCursorAdvance(&cursor, 1);
            consumed++;
        } else {
            break;
        }
    }

    if (consumed == 0) {
        VT100ByteStreamConsumerSetConsumed(consumer, 0);
        token->type = VT100_WAIT;
    } else {
        VT100ByteStreamConsumerSetConsumed(consumer, consumed);
        token->type = VT100_STRING;
    }
}

// Mixed ASCII ascii with CRLFs.
// This is a huge performance win for handling big files of mostly plain ascii text.
static void DecodeMixedASCIIBytes(VT100ByteStreamConsumer *consumer, VT100Token *token) {
    int consumed = 0;

    // Skip large printable ASCII runs quickly. On Apple Silicon the helper uses NEON to examine
    // 16 bytes at a time and falls back to scalar logic for Intel or when CRLF pairs terminate the
    // run.
    VT100ByteStreamCursor cursor = VT100ByteStreamConsumerGetCursor(consumer);
    CTVector(int) *crlfs = nil;
    while (VT100ByteStreamCursorGetSize(&cursor) > 0) {
        const int remaining = VT100ByteStreamCursorGetSize(&cursor);
        const unsigned char *bytes = VT100ByteStreamCursorGetPointer(&cursor);

        const int asciiRun = VT100PrintableASCIIRunLength(bytes, remaining);
        if (asciiRun > 0) {
            VT100ByteStreamCursorAdvance(&cursor, asciiRun);
            consumed += asciiRun;
            continue;
        }

        if (remaining >= 2 && bytes[0] == 13 && bytes[1] == 10) {
            if (!crlfs) {
                [token realizeCRLFsWithCapacity:40]; // This is a wild-ass guess
                crlfs = token.crlfs;
            }
            VT100ByteStreamCursorAdvance(&cursor, 2);
            CTVectorAppend(crlfs, consumed);
            consumed++;
            CTVectorAppend(crlfs, consumed);
            consumed++;
        } else {
            break;
        }
    }

    if (consumed == 0) {
        VT100ByteStreamConsumerReset(consumer);
        token->type = VT100_WAIT;
    } else {
        VT100ByteStreamConsumerSetConsumed(consumer, consumed);
        if (!crlfs) {
            token->type = VT100_ASCIISTRING;
        } else {
            token->type = VT100_MIXED_ASCII_CR_LF;
        }
    }
}

void ParseString(VT100ByteStreamConsumer *consumer, VT100Token *result, NSStringEncoding encoding) {
    VT100ByteStreamConsumerReset(consumer);

    result->type = VT100_UNKNOWNCHAR;
    result->code = VT100ByteStreamConsumerPeek(consumer);

    BOOL isAscii = NO;
    if (isMixedAsciiString(VT100ByteStreamConsumerPeek(consumer), VT100ByteStreamConsumerDoublePeek(consumer))) {
        isAscii = YES;
        DecodeMixedASCIIBytes(consumer, result);
        encoding = NSASCIIStringEncoding;
    } else if (encoding == NSUTF8StringEncoding) {
        DecodeUTF8Bytes(consumer, result);
    } else if (isEUCCNEncoding(encoding)) {
        // Chinese-GB
        DecodeEUCCNBytes(consumer, result);
    } else if (isBig5Encoding(encoding)) {
        DecodeBIG5Bytes(consumer, result);
    } else if (isJPEncoding(encoding)) {
        DecodeEUCJPBytes(consumer, result);
    } else if (isSJISEncoding(encoding)) {
        DecodeSJISBytes(consumer, result);
    } else if (isEUCKREncoding(encoding)) {
        // korean
        DecodeEUCKRBytes(consumer, result);
    } else if (isCP949Encoding(encoding)) {
        // korean
        DecodeCP949Bytes(consumer, result);
    } else {
        DecodeOtherBytes(consumer, result);
    }

    const int consumedCount = VT100ByteStreamConsumerGetConsumed(consumer);
    if (result->type == VT100_INVALID_SEQUENCE) {
        // Output only one replacement symbol, even if rmlen is higher.
        DLog(@"Parsed an invalid sequence of length %d for encoding %@: %@", consumedCount, @(encoding),
             VT100ByteStreamConsumerDescription(consumer));
        VT100ByteStreamConsumerWriteHead(consumer, ONECHAR_UNKNOWN);
        result.string = ReplacementString();
        result->type = VT100_STRING;
    } else if (result->type != VT100_WAIT && !isAscii) {
        VT100ByteStreamCursor cursor = VT100ByteStreamConsumerGetCursor(consumer);
        result.string = VT100ByteStreamCursorMakeString(&cursor, consumedCount, encoding);

        if (result.string == nil) {
            // Invalid bytes, can't encode.
            int i;
            if (encoding == NSUTF8StringEncoding) {
                // I am 98% sure this is unreachable because the UTF-8 decoder isn't buggy enough
                // to claim success but then leave us unable to create an NSString from it.
                result.string = [@"\uFFFD" stringRepeatedTimes:consumedCount];
            } else {
                // Replace every byte with ?, the replacement char for non-unicode encodings.
                for (i = consumedCount - 1; i >= 0 && !result.string; i--) {
                    VT100ByteStreamCursorWrite(&cursor, ONECHAR_UNKNOWN);
                    result.string = VT100ByteStreamCursorMakeString(&cursor, consumedCount, encoding);
                }
            }
        }
    }
}
