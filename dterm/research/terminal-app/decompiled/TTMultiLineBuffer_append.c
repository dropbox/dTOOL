/**
 * TTMultiLineBuffer.appendUTF8Characters:length:withAttributes:doesBeginLine:
 *
 * Decompiled from Terminal.app using radare2 + r2ghidra
 * Address: 0x10001182c
 *
 * Appends UTF-8 character data to the buffer with associated attributes.
 * Uses run-length encoding - adjacent chars with same attributes share one run.
 */

void method_TTMultiLineBuffer_appendUTF8Characters_length_withAttributes_doesBeginLine_(
    int64_t self,
    uchar noname_1[16],
    ulong utf8Chars,      // Pointer to UTF-8 bytes
    int64_t length,       // Number of bytes
    int64_t *attributes,  // Attribute struct pointer
    uint32_t beginLine    // Flag: does this start a new line?
)
{
    int64_t charsArray;
    ulong *lineOffsets;
    ulong destPtr;
    int64_t *runsArray;
    int64_t *prevRun;
    uint16_t prevFlags;
    uint8_t newFlags;

    // Early exit if no data
    if (length == 0) {
        return;
    }

    // Get internal arrays
    charsArray = *(self + 0x08);  // Raw UTF-8 bytes storage
    runsArray = *(self + 0x10);   // Attribute runs array

    // If beginning a new line OR no runs exist yet, record line offset
    if ((beginLine & 1) != 0 || *(charsArray + 0x10) == 0) {
        // Ensure capacity in lineOffsets array
        lineOffsets = ensureCapacity(*(self + 0x18), 1);

        // Store current char position as line start
        charsArray = *(self + 0x08);
        *lineOffsets = *(charsArray + 0x10);  // Current length = new line offset
    }

    // Append UTF-8 bytes to chars array
    destPtr = ensureCapacity(charsArray, length);
    memcpy(destPtr, utf8Chars, length);

    // Try to merge with previous run if attributes match
    prevRun = getLastRun(*(self + 0x10));

    if (prevRun != NULL) {
        newFlags = *attributes;
        prevFlags = prevRun[1];  // Previous run's flags

        // Check if we can merge (complex attribute comparison)
        // Bit 6 is some special flag that prevents merging
        if ((((newFlags >> 6 & 1) == 0) || ((prevFlags >> 6 & 1) == 0)) &&
            (*attributes == prevRun[1] &&          // Flags match
             attributes[1] == prevRun[2] &&        // Foreground matches
             attributes[2] == prevRun[3]))         // Background matches
        {
            // MERGE: Just extend the previous run's length
            *prevRun = *prevRun + length;
            goto done;
        }
    }

    // Cannot merge - create new attribute run
    prevRun = ensureCapacity(*(self + 0x10), 1);

    // Initialize new run
    *prevRun = length;              // Run length
    prevRun[1] = *attributes;       // Flags (bold, italic, etc.)
    prevRun[2] = attributes[1];     // Foreground color
    prevRun[3] = attributes[2];     // Background color

done:
    // Invalidate unichar offset cache
    *(self + 0x30) = 0x7fffffffffffffff;
    return;
}

/**
 * Helper function: ensureCapacity
 *
 * Ensures the dynamic array has room for 'count' more elements.
 * Returns pointer to the first new slot.
 *
 * Address: 0x100008ccc (sym.func.100008ccc)
 */
void* ensureCapacity(int64_t array, uint64_t count)
{
    uint64_t currentCount = *(array + 0x10);
    uint64_t capacity = *(array + 0x08);
    uint64_t elemSize = *(array + 0x18);

    uint64_t needed = currentCount + count;

    if (needed > capacity) {
        // Grow array - typically doubles capacity
        uint64_t newCapacity = capacity * 2;
        if (newCapacity < needed) {
            newCapacity = needed;
        }

        void *newBuffer = realloc(*(array), newCapacity * elemSize);
        *(array) = newBuffer;
        *(array + 0x08) = newCapacity;
    }

    // Update count
    *(array + 0x10) = needed;

    // Return pointer to first new element
    return *(array) + (currentCount * elemSize);
}

/**
 * Helper function: getLastRun
 *
 * Returns pointer to the last attribute run, or NULL if empty.
 *
 * Address: 0x1000110e0 (sym.func.1000110e0)
 */
int64_t* getLastRun(int64_t runsArray)
{
    uint64_t count = *(runsArray + 0x10);

    if (count == 0) {
        return NULL;
    }

    uint64_t elemSize = *(runsArray + 0x18);  // Size of one run struct
    void *data = *(runsArray);

    return (int64_t*)(data + ((count - 1) * elemSize));
}

/*
 * TTMultiLineBuffer Object Layout:
 *
 * Offset  Size  Field
 * 0x00    8     isa (class pointer)
 * 0x08    8     chars - pointer to dynamic array of UTF-8 bytes
 * 0x10    8     runs - pointer to dynamic array of AttributeRun
 * 0x18    8     lineOffsets - pointer to dynamic array of uint64_t
 * 0x20    8     columnCount
 * 0x28    1     isTextWrapped
 * 0x30    8     unicharCacheGeneration (MAX_INT64 = invalid)
 *
 *
 * Dynamic Array Structure (used for chars, runs, lineOffsets):
 *
 * Offset  Size  Field
 * 0x00    8     data - pointer to raw storage
 * 0x08    8     capacity - allocated slots
 * 0x10    8     count - used slots
 * 0x18    8     elementSize - bytes per element
 *
 *
 * AttributeRun Structure (~32 bytes):
 *
 * Offset  Size  Field
 * 0x00    8     length - number of bytes this run covers
 * 0x08    8     flags - packed attributes (bold, italic, etc.)
 * 0x10    8     fgColor - foreground color (index or RGB)
 * 0x18    8     bgColor - background color (index or RGB)
 *
 *
 * Key insights:
 *
 * 1. Run-length encoding is used - adjacent chars with same style share one run
 * 2. This is a major memory optimization vs per-character attributes
 * 3. The merge check at lines 45-52 is critical for efficiency
 * 4. Cache invalidation (line 64) triggers recomputation of UTF-16 offsets
 * 5. Line offsets are byte positions in the chars array, not character indices
 */
