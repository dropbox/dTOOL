/*
 * VT100 Parser ASCII Fast-Path Benchmark
 *
 * Measures scalar vs SIMD-accelerated scanning for mixed ASCII blocks with
 * CRLF pairs and ANSI escape sequences between runs.
 *
 * Supports both NEON (Apple Silicon) and SSE2 (Intel x86-64).
 *
 * Build (Apple Silicon):
 *   clang -framework Foundation -fobjc-arc -O3 \
 *     benchmarks/vt100_parser_benchmark.m -o benchmarks/vt100_parser_benchmark
 *
 * Build (Intel):
 *   clang -framework Foundation -fobjc-arc -O3 -msse2 \
 *     benchmarks/vt100_parser_benchmark.m -o benchmarks/vt100_parser_benchmark
 *
 * Run:
 *   ./benchmarks/vt100_parser_benchmark
 */

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>

#if defined(__ARM_NEON) || defined(__ARM_NEON__)
#import <arm_neon.h>
#define PARSER_HAS_NEON 1
#else
#define PARSER_HAS_NEON 0
#endif

#if defined(__SSE2__)
#import <emmintrin.h>
#define PARSER_HAS_SSE2 1
#else
#define PARSER_HAS_SSE2 0
#endif

#if defined(__AVX2__)
#import <immintrin.h>
#define PARSER_HAS_AVX2 1
#else
#define PARSER_HAS_AVX2 0
#endif

#if PARSER_HAS_NEON
#define PARSER_SIMD_NAME "NEON"
#elif PARSER_HAS_AVX2
#define PARSER_SIMD_NAME "AVX2"
#elif PARSER_HAS_SSE2
#define PARSER_SIMD_NAME "SSE2"
#else
#define PARSER_SIMD_NAME "Scalar"
#endif

static const unsigned char kEscapeSequence[] = "\x1b[38;5;196m";
static const size_t kEscapeSequenceLength = sizeof(kEscapeSequence) - 1;
static const NSUInteger kPayloadBytes = 8 * 1024 * 1024;
static const NSUInteger kIterations = 200;
static const NSUInteger kWarmupIterations = 5;

typedef struct {
    size_t consumed;
    size_t crlfPairs;
} ParseResult;

typedef ParseResult (*DecodeFunction)(const unsigned char *, size_t);

typedef struct {
    size_t asciiBytes;
    size_t crlfPairs;
} ParseTotals;

static inline uint64_t MachTicksToNanoseconds(uint64_t ticks) {
    static mach_timebase_info_data_t info;
    if (info.denom == 0) {
        mach_timebase_info(&info);
    }
    __uint128_t scaled = (__uint128_t)ticks * (uint64_t)info.numer;
    return (uint64_t)(scaled / info.denom);
}

static inline unsigned char RandomPrintable(void) {
    return (unsigned char)(0x20 + arc4random_uniform(0x7f - 0x20));
}

static NSData *GeneratePayload(void) {
    NSMutableData *data = [NSMutableData dataWithCapacity:kPayloadBytes + 4096];
    while (data.length < kPayloadBytes) {
        uint32_t runLength = 64 + arc4random_uniform(256);
        for (uint32_t i = 0; i < runLength; i++) {
            unsigned char c = RandomPrintable();
            [data appendBytes:&c length:1];
            if ((i % 80 == 79) && (arc4random_uniform(4) == 0)) {
                const unsigned char crlf[] = "\r\n";
                [data appendBytes:crlf length:2];
            }
        }
        if (arc4random_uniform(5) == 0) {
            const unsigned char crlf[] = "\r\n";
            [data appendBytes:crlf length:2];
        }
        [data appendBytes:kEscapeSequence length:kEscapeSequenceLength];
    }
    return data;
}

static ParseResult ScalarDecode(const unsigned char *bytes, size_t length) {
    ParseResult result = {0, 0};
    size_t offset = 0;
    while (offset < length) {
        const unsigned char c = bytes[offset];
        if (c >= 0x20 && c <= 0x7f) {
            offset++;
            continue;
        }
        if (c == '\r' && offset + 1 < length && bytes[offset + 1] == '\n') {
            offset += 2;
            result.crlfPairs++;
            continue;
        }
        break;
    }
    result.consumed = offset;
    return result;
}

static inline size_t PrintableRunLength(const unsigned char *bytes, size_t length) {
    size_t processed = 0;

#if PARSER_HAS_NEON
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
                    return processed + (size_t)i;
                }
            }
            return processed + 16;
        }
    }
#elif PARSER_HAS_AVX2
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
            return processed + (size_t)__builtin_ctz(mask);
        }
    }
    // Fall through to scalar for remaining 0-31 bytes
#elif PARSER_HAS_SSE2
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
            return processed + (size_t)__builtin_ctz(mask);
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

static ParseResult OptimizedDecode(const unsigned char *bytes, size_t length) {
    ParseResult result = {0, 0};
    size_t remaining = length;
    const unsigned char *cursor = bytes;

    while (remaining > 0) {
        size_t asciiRun = PrintableRunLength(cursor, remaining);
        if (asciiRun > 0) {
            cursor += asciiRun;
            remaining -= asciiRun;
            result.consumed += asciiRun;
            continue;
        }
        if (remaining >= 2 && cursor[0] == '\r' && cursor[1] == '\n') {
            cursor += 2;
            remaining -= 2;
            result.consumed += 2;
            result.crlfPairs++;
            continue;
        }
        break;
    }
    return result;
}

static ParseTotals ProcessPayload(NSData *payload, DecodeFunction decode) {
    ParseTotals totals = {0, 0};
    const unsigned char *bytes = payload.bytes;
    const size_t length = payload.length;
    size_t offset = 0;

    while (offset + kEscapeSequenceLength < length) {
        ParseResult res = decode(bytes + offset, length - offset);
        if (res.consumed == 0) {
            break;
        }
        totals.asciiBytes += res.consumed;
        totals.crlfPairs += res.crlfPairs;
        offset += res.consumed;
        offset += kEscapeSequenceLength;
    }

    return totals;
}

static double RunBenchmark(const char *label, NSData *payload, DecodeFunction decode, ParseTotals *outTotals) {
    for (NSUInteger i = 0; i < kWarmupIterations; i++) {
        (void)ProcessPayload(payload, decode);
    }

    uint64_t totalNs = 0;
    ParseTotals totals = {0, 0};

    for (NSUInteger i = 0; i < kIterations; i++) {
        const uint64_t start = mach_absolute_time();
        totals = ProcessPayload(payload, decode);
        const uint64_t delta = mach_absolute_time() - start;
        totalNs += MachTicksToNanoseconds(delta);
    }

    if (outTotals) {
        *outTotals = totals;
    }

    const double avgNs = (double)totalNs / (double)kIterations;
    const double avgMs = avgNs / 1e6;
    const double throughputMBs = ((double)payload.length / (1024.0 * 1024.0)) / (avgNs / 1e9);

    printf("%-10s Avg: %8.3f ms  Throughput: %6.2f MB/s\n", label, avgMs, throughputMBs);
    return avgNs;
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        printf("VT100 Parser ASCII Fast-Path Benchmark\n");
        printf("Payload Size: %.2f MB\n", (double)kPayloadBytes / (1024.0 * 1024.0));
        printf("Iterations:   %u\n\n", (unsigned int)kIterations);

        NSData *payload = GeneratePayload();

        ParseTotals scalarTotals;
        ParseTotals simdTotals;
        double scalarNs = RunBenchmark("Scalar", payload, ScalarDecode, &scalarTotals);
        double simdNs = RunBenchmark(PARSER_SIMD_NAME, payload, OptimizedDecode, &simdTotals);

        if (scalarTotals.asciiBytes != simdTotals.asciiBytes || scalarTotals.crlfPairs != simdTotals.crlfPairs) {
            fprintf(stderr, "ERROR: Totals mismatch between scalar and optimized paths.\n");
            fprintf(stderr, "Scalar: bytes=%zu crlf=%zu\n", scalarTotals.asciiBytes, scalarTotals.crlfPairs);
            fprintf(stderr, "SIMD:   bytes=%zu crlf=%zu\n", simdTotals.asciiBytes, simdTotals.crlfPairs);
            return 1;
        }

#if PARSER_HAS_NEON || PARSER_HAS_AVX2 || PARSER_HAS_SSE2
        double speedup = scalarNs / simdNs;
        printf("\nSpeedup: %.2fx faster ASCII scanning (%s)\n", speedup, PARSER_SIMD_NAME);
#else
        printf("\nSIMD not available on this architecture; optimized path matches scalar baseline.\n");
#endif
    }
    return 0;
}
