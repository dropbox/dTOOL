//
//  MetalBugRegressionTests.mm
//  DashTerm2
//
//  Created by DashTerm2 Worker on 2025-12-21.
//  Regression tests for Metal rendering bugs BUG-1107 to BUG-1129
//

#import <XCTest/XCTest.h>
#import <vector>

// Include the PIUArray header for template testing
#import "iTermPIUArray.h"
#import "iTermASCIITexture.h"
#import "iTermTextureArray.h"
#import "iTermMetalRowDataPool.h"
#import "iTermMetalRowData.h"

@interface MetalBugRegressionTests : XCTestCase
@end

@implementation MetalBugRegressionTests

#pragma mark - BUG-1107/1108: iTermPIUArray bounds checking

/// BUG-1107: start_of_segment and size_of_segment should handle invalid segment index
/// BUG-1108: get() should handle invalid index and segment
/// Fix: Added bounds validation to all accessor methods
- (void)test_BUG_1107_1108_PIUArrayInvalidSegmentShouldNotCrash {
    // Create a simple test struct for PIUArray
    struct TestPIU {
        int value;
    };

    DashTerm2::PIUArray<TestPIU> array;

    // Add a few elements
    for (int i = 0; i < 5; i++) {
        TestPIU *piu = array.get_next();
        piu->value = i;
    }

    // Valid access should work
    XCTAssertEqual(array.size(), 5, @"Array should have 5 elements");
    XCTAssertEqual(array.get_number_of_segments(), 1, @"Should have 1 segment");
    XCTAssertEqual(array.size_of_segment(0), 5, @"Segment 0 should have 5 elements");

    // Invalid segment access should return safe values (not crash)
    // BUG-1107: These used to crash before the fix
    size_t invalidSegmentSize = array.size_of_segment(999);
    XCTAssertEqual(invalidSegmentSize, 0, @"Invalid segment should return size 0");

    const TestPIU *invalidSegmentStart = array.start_of_segment(999);
    XCTAssertTrue(invalidSegmentStart == nullptr, @"Invalid segment should return nullptr");

    // Empty segment access
    const TestPIU *emptyStart = array.start_of_segment(0);
    XCTAssertTrue(emptyStart != nullptr, @"Valid segment should return non-null");
}

- (void)test_BUG_1108_PIUArrayGetInvalidIndexShouldNotCrash {
    struct TestPIU {
        int value;
    };

    DashTerm2::PIUArray<TestPIU> array;

    // Add one element
    TestPIU *piu = array.get_next();
    piu->value = 42;

    // Valid access
    XCTAssertEqual(array.get(0).value, 42, @"Valid index should return correct value");
    XCTAssertEqual(array.get(0, 0).value, 42, @"Valid segment+index should return correct value");

    // Invalid index access - should return dummy value (not crash)
    // These used to cause undefined behavior before the fix
    TestPIU &invalidRef = array.get(1000);
    (void)invalidRef; // Access is safe now

    TestPIU &invalidSegmentRef = array.get(999, 0);
    (void)invalidSegmentRef; // Access is safe now
}

#pragma mark - BUG-1126: iTermTextureArray integer overflow prevention

/// BUG-1126: atlasSizeForUnitSize should handle zero/negative inputs
/// Fix: Added input validation to prevent division by zero and overflow
- (void)test_BUG_1126_TextureArrayZeroSizeShouldNotCrash {
    // Zero dimensions should return minimal safe values
    NSInteger cellsPerRow = 0;

    // Zero width should be handled safely
    CGSize result = [iTermTextureArray atlasSizeForUnitSize:CGSizeMake(0, 10)
                                                arrayLength:10
                                                cellsPerRow:&cellsPerRow];
    XCTAssertGreaterThan(result.width, 0, @"Width should be positive even with zero input");
    XCTAssertGreaterThan(result.height, 0, @"Height should be positive even with zero input");
    XCTAssertGreaterThan(cellsPerRow, 0, @"Cells per row should be positive");

    // Zero height should be handled safely
    result = [iTermTextureArray atlasSizeForUnitSize:CGSizeMake(10, 0)
                                         arrayLength:10
                                         cellsPerRow:&cellsPerRow];
    XCTAssertGreaterThan(result.width, 0, @"Width should be positive even with zero height");

    // Zero length should be handled safely
    result = [iTermTextureArray atlasSizeForUnitSize:CGSizeMake(10, 10)
                                         arrayLength:0
                                         cellsPerRow:&cellsPerRow];
    XCTAssertGreaterThan(result.width, 0, @"Width should be positive even with zero length");
}

#pragma mark - BUG-1109: ASCII texture parts array bounds

/// BUG-1109: ASCII code validation before parts array access
/// Fix: Added bounds check before accessing parts array
- (void)test_BUG_1109_ASCIITextureCodeBoundsValidation {
    // Verify the constants are as expected
    XCTAssertEqual(iTermASCIITextureMinimumCharacter, 32, @"Minimum ASCII char should be space (32)");
    XCTAssertEqual(iTermASCIITextureMaximumCharacter, 126, @"Maximum ASCII char should be tilde (126)");

    // Valid character codes should be in range
    XCTAssertTrue(iTermASCIITextureMinimumCharacter >= 0, @"Min char should be non-negative");
    XCTAssertTrue(iTermASCIITextureMaximumCharacter < 128, @"Max char should fit in parts array (128 elements)");
}

#pragma mark - Edge case validation for texture calculations

/// Test that texture index calculation handles boundary values correctly
- (void)test_TextureIndexOfCodeBoundaryValues {
    // Test that valid codes produce valid indices
    NSInteger indexMin = iTermASCIITextureIndexOfCode(iTermASCIITextureMinimumCharacter, iTermASCIITextureOffsetCenter);
    XCTAssertGreaterThanOrEqual(indexMin, 0, @"Index for min char should be non-negative");

    NSInteger indexMax = iTermASCIITextureIndexOfCode(iTermASCIITextureMaximumCharacter, iTermASCIITextureOffsetCenter);
    XCTAssertGreaterThanOrEqual(indexMax, 0, @"Index for max char should be non-negative");
    XCTAssertGreaterThan(indexMax, indexMin, @"Max char index should be greater than min char index");
}

#pragma mark - iTermMetalRowDataPool Tests

/// Test basic pool acquire/return functionality
- (void)test_RowDataPool_AcquireAndReturn {
    iTermMetalRowDataPool *pool = [[iTermMetalRowDataPool alloc] init];

    // Initial state
    XCTAssertEqual(pool.pooledCount, 0, @"New pool should be empty");
    XCTAssertEqual(pool.inUseCount, 0, @"No objects should be in use");

    // Acquire row data
    iTermMetalRowData *rowData = [pool acquireRowDataWithColumns:80];
    XCTAssertNotNil(rowData, @"Should return valid row data");
    XCTAssertEqual(pool.inUseCount, 1, @"One object should be in use");
    XCTAssertEqual(pool.pooledCount, 0, @"Pool should still be empty");
    XCTAssertEqual(pool.totalAllocations, 1, @"Should have allocated one object");

    // Row data should have pre-allocated buffers
    XCTAssertNotNil(rowData.keysData, @"Should have keysData allocated");
    XCTAssertNotNil(rowData.attributesData, @"Should have attributesData allocated");
    XCTAssertNotNil(rowData.backgroundColorRLEData, @"Should have backgroundColorRLEData allocated");

    // Return to pool
    [pool returnRowData:rowData];
    XCTAssertEqual(pool.inUseCount, 0, @"No objects should be in use after return");
    XCTAssertEqual(pool.pooledCount, 1, @"Pool should have one object");
}

/// Test pool reuse (acquire returns pooled object)
- (void)test_RowDataPool_Reuse {
    iTermMetalRowDataPool *pool = [[iTermMetalRowDataPool alloc] init];

    // Acquire and return
    iTermMetalRowData *rowData1 = [pool acquireRowDataWithColumns:80];
    [pool returnRowData:rowData1];

    // Acquire again - should reuse
    iTermMetalRowData *rowData2 = [pool acquireRowDataWithColumns:80];
    XCTAssertEqual(rowData1, rowData2, @"Should reuse pooled object");
    XCTAssertEqual(pool.totalAllocations, 1, @"Should only have 1 total allocation");
    XCTAssertEqual(pool.totalReuses, 1, @"Should have 1 reuse");
}

/// Test pool max size enforcement
- (void)test_RowDataPool_MaxSizeEnforcement {
    NSUInteger maxSize = 4;
    iTermMetalRowDataPool *pool = [[iTermMetalRowDataPool alloc] initWithMaxPoolSize:maxSize];

    // Acquire more than max size
    NSMutableArray<iTermMetalRowData *> *rowDatas = [NSMutableArray array];
    for (int i = 0; i < 10; i++) {
        [rowDatas addObject:[pool acquireRowDataWithColumns:80]];
    }
    XCTAssertEqual(pool.inUseCount, 10, @"10 objects should be in use");

    // Return all - only maxSize should be kept
    for (iTermMetalRowData *rd in rowDatas) {
        [pool returnRowData:rd];
    }
    XCTAssertEqual(pool.pooledCount, maxSize, @"Pool should only keep maxSize objects");
    XCTAssertEqual(pool.inUseCount, 0, @"No objects should be in use");
}

/// Test returnRowDataArray batch return
- (void)test_RowDataPool_BatchReturn {
    iTermMetalRowDataPool *pool = [[iTermMetalRowDataPool alloc] init];

    // Acquire several
    NSMutableArray<iTermMetalRowData *> *rowDatas = [NSMutableArray array];
    for (int i = 0; i < 5; i++) {
        [rowDatas addObject:[pool acquireRowDataWithColumns:80]];
    }
    XCTAssertEqual(pool.inUseCount, 5, @"5 objects should be in use");

    // Batch return
    [pool returnRowDataArray:rowDatas];
    XCTAssertEqual(pool.inUseCount, 0, @"No objects should be in use after batch return");
    XCTAssertEqual(pool.pooledCount, 5, @"All 5 should be in pool");
}

/// Test drain clears pool
- (void)test_RowDataPool_Drain {
    iTermMetalRowDataPool *pool = [[iTermMetalRowDataPool alloc] init];

    // Fill pool
    iTermMetalRowData *rowData = [pool acquireRowDataWithColumns:80];
    [pool returnRowData:rowData];
    XCTAssertEqual(pool.pooledCount, 1, @"Pool should have 1 object");

    // Drain
    [pool drain];
    XCTAssertEqual(pool.pooledCount, 0, @"Pool should be empty after drain");
}

/// Test that nil input is handled safely
- (void)test_RowDataPool_NilInputHandling {
    iTermMetalRowDataPool *pool = [[iTermMetalRowDataPool alloc] init];

    // Should not crash with nil
    [pool returnRowData:nil];
    XCTAssertEqual(pool.pooledCount, 0, @"Pool should remain empty");
    XCTAssertEqual(pool.inUseCount, 0, @"In-use count should remain 0");

    [pool returnRowDataArray:@[]];
    XCTAssertEqual(pool.pooledCount, 0, @"Pool should remain empty with empty array");
}

/// Test buffer resizing when column count changes
- (void)test_RowDataPool_BufferResizing {
    iTermMetalRowDataPool *pool = [[iTermMetalRowDataPool alloc] init];

    // Acquire with small columns
    iTermMetalRowData *rowData = [pool acquireRowDataWithColumns:40];
    NSUInteger smallSize = rowData.keysData.length;
    [pool returnRowData:rowData];

    // Re-acquire with larger columns
    rowData = [pool acquireRowDataWithColumns:200];
    NSUInteger largeSize = rowData.keysData.length;

    XCTAssertGreaterThan(largeSize, smallSize, @"Buffers should be resized for larger column count");
}

@end
