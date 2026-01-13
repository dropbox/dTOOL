/*
 * Multi-Session CoW Benchmark
 *
 * This benchmark measures the performance impact of per-tree mutex vs global mutex
 * for LineBlock copy-on-write operations across multiple simulated terminal sessions.
 *
 * The benchmark creates N independent "sessions" (each with its own LineBlock tree)
 * and performs concurrent CoW operations (copy and mutate) from multiple threads.
 *
 * With a global mutex, all sessions serialize on the same lock.
 * With per-tree mutex, independent sessions can operate in parallel.
 *
 * Compile and run:
 *   clang -framework Foundation -O2 -o Benchmarks/multisession_cow_benchmark \
 *     Benchmarks/multisession_cow_benchmark.m -fobjc-arc -lpthread
 *   ./Benchmarks/multisession_cow_benchmark
 */

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>
#import <pthread.h>
#import <stdatomic.h>

#pragma mark - Configuration

#define NUM_SESSIONS 8           // Number of simulated terminal sessions
#define ITERATIONS_PER_SESSION 10000  // CoW operations per session
#define LINE_BLOCK_SIZE 8192     // Bytes per LineBlock buffer
#define NUM_LINES_PER_BLOCK 200  // Lines per LineBlock
#define WARMUP_ITERATIONS 1000   // Warmup before measurement

#pragma mark - Simulated LineBlock Data Structures

// Simulated character buffer (simplified from iTermCharacterBuffer)
@interface SimulatedCharBuffer : NSObject <NSCopying>
@property (nonatomic, readonly) char *buffer;
@property (nonatomic, readonly) size_t size;
@property (nonatomic, readonly) NSUInteger refCount;
@end

@implementation SimulatedCharBuffer {
    char *_buffer;
    size_t _size;
    atomic_uint _refCount;
}

- (instancetype)initWithSize:(size_t)size {
    self = [super init];
    if (self) {
        _size = size;
        _buffer = (char *)malloc(size);
        memset(_buffer, 'A', size);  // Fill with data
        atomic_store(&_refCount, 1);
    }
    return self;
}

- (void)dealloc {
    free(_buffer);
}

- (char *)buffer { return _buffer; }
- (size_t)size { return _size; }
- (NSUInteger)refCount { return atomic_load(&_refCount); }

- (void)incrementRefCount {
    atomic_fetch_add(&_refCount, 1);
}

- (void)decrementRefCount {
    atomic_fetch_sub(&_refCount, 1);
}

- (id)copyWithZone:(NSZone *)zone {
    SimulatedCharBuffer *copy = [[SimulatedCharBuffer alloc] initWithSize:_size];
    memcpy(copy->_buffer, _buffer, _size);
    return copy;
}

@end

// Simulated LineBlock with CoW semantics
// This simulates the real LineBlock's per-tree mutex behavior
typedef std::shared_ptr<std::recursive_mutex> MutexPtr;

@interface SimulatedLineBlock : NSObject
@property (nonatomic, strong) SimulatedCharBuffer *characterBuffer;
@property (nonatomic, assign) BOOL hasBeenCopied;
@property (nonatomic, weak) SimulatedLineBlock *owner;
@property (nonatomic, strong) NSMutableArray<SimulatedLineBlock *> *clients;
@end

@implementation SimulatedLineBlock {
    MutexPtr _treeMutex;
    int *_cumulativeLineLengths;
    int _cllCapacity;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _characterBuffer = [[SimulatedCharBuffer alloc] initWithSize:LINE_BLOCK_SIZE];
        _treeMutex = std::make_shared<std::recursive_mutex>();
        _cllCapacity = NUM_LINES_PER_BLOCK;
        _cumulativeLineLengths = (int *)malloc(sizeof(int) * _cllCapacity);
        for (int i = 0; i < _cllCapacity; i++) {
            _cumulativeLineLengths[i] = (i + 1) * 40;  // 40 chars per line
        }
        _clients = [NSMutableArray array];
        _hasBeenCopied = NO;
    }
    return self;
}

- (void)dealloc {
    free(_cumulativeLineLengths);
}

// Simulates cowCopy - shallow copy sharing the buffer
- (SimulatedLineBlock *)cowCopy {
    std::lock_guard<std::recursive_mutex> lock(*_treeMutex);

    _hasBeenCopied = YES;

    SimulatedLineBlock *copy = [[SimulatedLineBlock alloc] init];
    // Share the buffer (shallow copy)
    copy->_characterBuffer = _characterBuffer;
    [_characterBuffer incrementRefCount];

    // Share the tree mutex
    copy->_treeMutex = _treeMutex;

    // Copy CLL data (also shallow - would be shared in real impl)
    memcpy(copy->_cumulativeLineLengths, _cumulativeLineLengths, sizeof(int) * _cllCapacity);

    // Set up ownership
    SimulatedLineBlock *root = self;
    while (root.owner != nil) {
        root = root.owner;
    }
    copy.owner = root;
    [root.clients addObject:copy];

    return copy;
}

// Simulates validMutationCertificate - triggers CoW if needed
- (void)prepareForMutation {
    if (!_hasBeenCopied) {
        return;  // No CoW needed
    }

    std::lock_guard<std::recursive_mutex> lock(*_treeMutex);

    if (_owner == nil && _clients.count == 0) {
        return;  // No CoW needed
    }

    // Perform CoW - deep copy the buffer
    _characterBuffer = [_characterBuffer copy];

    // After CoW, allocate new mutex for this independent tree
    _treeMutex = std::make_shared<std::recursive_mutex>();

    // Remove from owner's client list
    if (_owner != nil) {
        [_owner.clients removeObject:self];
        _owner = nil;
    }

    // Transfer clients to first client (if any)
    if (_clients.count > 0) {
        SimulatedLineBlock *newOwner = _clients[0];
        newOwner.owner = nil;

        for (NSUInteger i = 1; i < _clients.count; i++) {
            SimulatedLineBlock *client = _clients[i];
            client.owner = newOwner;
            [newOwner.clients addObject:client];
        }
        [_clients removeAllObjects];
    }

    _hasBeenCopied = NO;
}

// Simulates a mutation (append data)
- (void)appendData {
    [self prepareForMutation];
    // Simulate writing to the buffer
    if (_characterBuffer.buffer != NULL && _characterBuffer.size > 0) {
        _characterBuffer.buffer[0] = 'Z';  // Just touch the buffer
    }
}

@end

#pragma mark - Global Mutex Version (for comparison)

static std::recursive_mutex gGlobalMutex;

@interface GlobalMutexLineBlock : NSObject
@property (nonatomic, strong) SimulatedCharBuffer *characterBuffer;
@property (nonatomic, assign) BOOL hasBeenCopied;
@property (nonatomic, weak) GlobalMutexLineBlock *owner;
@property (nonatomic, strong) NSMutableArray<GlobalMutexLineBlock *> *clients;
@end

@implementation GlobalMutexLineBlock {
    int *_cumulativeLineLengths;
    int _cllCapacity;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _characterBuffer = [[SimulatedCharBuffer alloc] initWithSize:LINE_BLOCK_SIZE];
        _cllCapacity = NUM_LINES_PER_BLOCK;
        _cumulativeLineLengths = (int *)malloc(sizeof(int) * _cllCapacity);
        for (int i = 0; i < _cllCapacity; i++) {
            _cumulativeLineLengths[i] = (i + 1) * 40;
        }
        _clients = [NSMutableArray array];
        _hasBeenCopied = NO;
    }
    return self;
}

- (void)dealloc {
    free(_cumulativeLineLengths);
}

- (GlobalMutexLineBlock *)cowCopy {
    std::lock_guard<std::recursive_mutex> lock(gGlobalMutex);  // GLOBAL MUTEX

    _hasBeenCopied = YES;

    GlobalMutexLineBlock *copy = [[GlobalMutexLineBlock alloc] init];
    copy->_characterBuffer = _characterBuffer;
    [_characterBuffer incrementRefCount];
    memcpy(copy->_cumulativeLineLengths, _cumulativeLineLengths, sizeof(int) * _cllCapacity);

    GlobalMutexLineBlock *root = self;
    while (root.owner != nil) {
        root = root.owner;
    }
    copy.owner = root;
    [root.clients addObject:copy];

    return copy;
}

- (void)prepareForMutation {
    if (!_hasBeenCopied) {
        return;
    }

    std::lock_guard<std::recursive_mutex> lock(gGlobalMutex);  // GLOBAL MUTEX

    if (_owner == nil && _clients.count == 0) {
        return;
    }

    _characterBuffer = [_characterBuffer copy];

    if (_owner != nil) {
        [_owner.clients removeObject:self];
        _owner = nil;
    }

    if (_clients.count > 0) {
        GlobalMutexLineBlock *newOwner = _clients[0];
        newOwner.owner = nil;

        for (NSUInteger i = 1; i < _clients.count; i++) {
            GlobalMutexLineBlock *client = _clients[i];
            client.owner = newOwner;
            [newOwner.clients addObject:client];
        }
        [_clients removeAllObjects];
    }

    _hasBeenCopied = NO;
}

- (void)appendData {
    [self prepareForMutation];
    if (_characterBuffer.buffer != NULL && _characterBuffer.size > 0) {
        _characterBuffer.buffer[0] = 'Z';
    }
}

@end

#pragma mark - Benchmark Infrastructure

static mach_timebase_info_data_t sTimebaseInfo;

static uint64_t nanosFromMachTime(uint64_t machTime) {
    return machTime * sTimebaseInfo.numer / sTimebaseInfo.denom;
}

typedef struct {
    int sessionId;
    int iterations;
    uint64_t totalTime;
    int copyCount;
    int mutateCount;
} SessionResult;

// Per-tree mutex benchmark thread
typedef struct {
    SimulatedLineBlock *rootBlock;
    int sessionId;
    int iterations;
    SessionResult *result;
    dispatch_semaphore_t startSemaphore;
} PerTreeThreadArg;

void *perTreeBenchmarkThread(void *arg) {
    PerTreeThreadArg *targ = (PerTreeThreadArg *)arg;

    @autoreleasepool {
        // Wait for all threads to be ready
        dispatch_semaphore_wait(targ->startSemaphore, DISPATCH_TIME_FOREVER);

        uint64_t start = mach_absolute_time();

        SimulatedLineBlock *currentBlock = targ->rootBlock;
        int copyCount = 0;
        int mutateCount = 0;

        for (int i = 0; i < targ->iterations; i++) {
            // Alternate between copy and mutate operations
            if (i % 3 == 0) {
                // Create a CoW copy
                currentBlock = [currentBlock cowCopy];
                copyCount++;
            } else {
                // Mutate (triggers CoW if copied)
                [currentBlock appendData];
                mutateCount++;
            }
        }

        uint64_t end = mach_absolute_time();

        targ->result->sessionId = targ->sessionId;
        targ->result->iterations = targ->iterations;
        targ->result->totalTime = nanosFromMachTime(end - start);
        targ->result->copyCount = copyCount;
        targ->result->mutateCount = mutateCount;
    }

    return NULL;
}

// Global mutex benchmark thread
typedef struct {
    GlobalMutexLineBlock *rootBlock;
    int sessionId;
    int iterations;
    SessionResult *result;
    dispatch_semaphore_t startSemaphore;
} GlobalMutexThreadArg;

void *globalMutexBenchmarkThread(void *arg) {
    GlobalMutexThreadArg *targ = (GlobalMutexThreadArg *)arg;

    @autoreleasepool {
        dispatch_semaphore_wait(targ->startSemaphore, DISPATCH_TIME_FOREVER);

        uint64_t start = mach_absolute_time();

        GlobalMutexLineBlock *currentBlock = targ->rootBlock;
        int copyCount = 0;
        int mutateCount = 0;

        for (int i = 0; i < targ->iterations; i++) {
            if (i % 3 == 0) {
                currentBlock = [currentBlock cowCopy];
                copyCount++;
            } else {
                [currentBlock appendData];
                mutateCount++;
            }
        }

        uint64_t end = mach_absolute_time();

        targ->result->sessionId = targ->sessionId;
        targ->result->iterations = targ->iterations;
        targ->result->totalTime = nanosFromMachTime(end - start);
        targ->result->copyCount = copyCount;
        targ->result->mutateCount = mutateCount;
    }

    return NULL;
}

#pragma mark - Main Benchmark

void runPerTreeBenchmark(int numSessions, int iterationsPerSession, SessionResult *results) {
    @autoreleasepool {
        // Create independent LineBlock trees (one per session)
        NSMutableArray<SimulatedLineBlock *> *rootBlocks = [NSMutableArray arrayWithCapacity:numSessions];
        for (int i = 0; i < numSessions; i++) {
            [rootBlocks addObject:[[SimulatedLineBlock alloc] init]];
        }

        // Create threads
        pthread_t threads[numSessions];
        PerTreeThreadArg args[numSessions];
        dispatch_semaphore_t startSemaphore = dispatch_semaphore_create(0);

        for (int i = 0; i < numSessions; i++) {
            args[i].rootBlock = rootBlocks[i];
            args[i].sessionId = i;
            args[i].iterations = iterationsPerSession;
            args[i].result = &results[i];
            args[i].startSemaphore = startSemaphore;

            pthread_create(&threads[i], NULL, perTreeBenchmarkThread, &args[i]);
        }

        // Signal all threads to start simultaneously
        for (int i = 0; i < numSessions; i++) {
            dispatch_semaphore_signal(startSemaphore);
        }

        // Wait for all threads to complete
        for (int i = 0; i < numSessions; i++) {
            pthread_join(threads[i], NULL);
        }
    }
}

void runGlobalMutexBenchmark(int numSessions, int iterationsPerSession, SessionResult *results) {
    @autoreleasepool {
        // Create independent LineBlock trees (one per session)
        NSMutableArray<GlobalMutexLineBlock *> *rootBlocks = [NSMutableArray arrayWithCapacity:numSessions];
        for (int i = 0; i < numSessions; i++) {
            [rootBlocks addObject:[[GlobalMutexLineBlock alloc] init]];
        }

        // Create threads
        pthread_t threads[numSessions];
        GlobalMutexThreadArg args[numSessions];
        dispatch_semaphore_t startSemaphore = dispatch_semaphore_create(0);

        for (int i = 0; i < numSessions; i++) {
            args[i].rootBlock = rootBlocks[i];
            args[i].sessionId = i;
            args[i].iterations = iterationsPerSession;
            args[i].result = &results[i];
            args[i].startSemaphore = startSemaphore;

            pthread_create(&threads[i], NULL, globalMutexBenchmarkThread, &args[i]);
        }

        for (int i = 0; i < numSessions; i++) {
            dispatch_semaphore_signal(startSemaphore);
        }

        for (int i = 0; i < numSessions; i++) {
            pthread_join(threads[i], NULL);
        }
    }
}

void printResults(const char *name, SessionResult *results, int numSessions) {
    uint64_t totalTime = 0;
    uint64_t maxTime = 0;
    uint64_t minTime = UINT64_MAX;

    for (int i = 0; i < numSessions; i++) {
        totalTime += results[i].totalTime;
        if (results[i].totalTime > maxTime) maxTime = results[i].totalTime;
        if (results[i].totalTime < minTime) minTime = results[i].totalTime;
    }

    double avgTimeMs = (double)totalTime / numSessions / 1000000.0;
    double maxTimeMs = (double)maxTime / 1000000.0;
    double minTimeMs = (double)minTime / 1000000.0;
    double wallClockMs = maxTimeMs;  // Wall clock is determined by slowest thread

    printf("  %s:\n", name);
    printf("    Avg per-session time: %.3f ms\n", avgTimeMs);
    printf("    Min session time:     %.3f ms\n", minTimeMs);
    printf("    Max session time:     %.3f ms\n", maxTimeMs);
    printf("    Wall clock time:      %.3f ms\n", wallClockMs);
    printf("    Throughput:           %.0f ops/sec\n",
           (double)(numSessions * results[0].iterations) / (wallClockMs / 1000.0));
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        mach_timebase_info(&sTimebaseInfo);

        printf("=================================================================\n");
        printf("Multi-Session CoW Benchmark\n");
        printf("=================================================================\n");
        printf("\nConfiguration:\n");
        printf("  Sessions:           %d\n", NUM_SESSIONS);
        printf("  Iterations/session: %d\n", ITERATIONS_PER_SESSION);
        printf("  LineBlock size:     %d bytes\n", LINE_BLOCK_SIZE);
        printf("  Lines per block:    %d\n", NUM_LINES_PER_BLOCK);
        printf("\nThis benchmark compares:\n");
        printf("  - Global Mutex: All sessions serialize on a single mutex\n");
        printf("  - Per-Tree Mutex: Each session has its own mutex\n");
        printf("\n");

        // Warmup
        printf("Warming up...\n");
        SessionResult warmupResults[NUM_SESSIONS];
        runPerTreeBenchmark(NUM_SESSIONS, WARMUP_ITERATIONS, warmupResults);
        runGlobalMutexBenchmark(NUM_SESSIONS, WARMUP_ITERATIONS, warmupResults);

        // Run benchmarks multiple times for stability
        const int numRuns = 5;
        double perTreeWallClocks[numRuns];
        double globalMutexWallClocks[numRuns];

        printf("\nRunning benchmarks (%d runs each)...\n\n", numRuns);

        for (int run = 0; run < numRuns; run++) {
            SessionResult perTreeResults[NUM_SESSIONS];
            SessionResult globalResults[NUM_SESSIONS];

            // Run per-tree first
            runPerTreeBenchmark(NUM_SESSIONS, ITERATIONS_PER_SESSION, perTreeResults);

            // Then global mutex
            runGlobalMutexBenchmark(NUM_SESSIONS, ITERATIONS_PER_SESSION, globalResults);

            // Calculate wall clock times
            uint64_t maxPerTree = 0, maxGlobal = 0;
            for (int i = 0; i < NUM_SESSIONS; i++) {
                if (perTreeResults[i].totalTime > maxPerTree) {
                    maxPerTree = perTreeResults[i].totalTime;
                }
                if (globalResults[i].totalTime > maxGlobal) {
                    maxGlobal = globalResults[i].totalTime;
                }
            }
            perTreeWallClocks[run] = (double)maxPerTree / 1000000.0;
            globalMutexWallClocks[run] = (double)maxGlobal / 1000000.0;

            printf("Run %d: Per-Tree=%.2fms, Global=%.2fms, Speedup=%.2fx\n",
                   run + 1, perTreeWallClocks[run], globalMutexWallClocks[run],
                   globalMutexWallClocks[run] / perTreeWallClocks[run]);
        }

        // Calculate averages
        double avgPerTree = 0, avgGlobal = 0;
        for (int i = 0; i < numRuns; i++) {
            avgPerTree += perTreeWallClocks[i];
            avgGlobal += globalMutexWallClocks[i];
        }
        avgPerTree /= numRuns;
        avgGlobal /= numRuns;

        // Final detailed run
        printf("\n-----------------------------------------------------------------\n");
        printf("Final Benchmark Results (detailed):\n");
        printf("-----------------------------------------------------------------\n");

        SessionResult perTreeResults[NUM_SESSIONS];
        SessionResult globalResults[NUM_SESSIONS];

        runPerTreeBenchmark(NUM_SESSIONS, ITERATIONS_PER_SESSION, perTreeResults);
        printResults("Per-Tree Mutex (NEW)", perTreeResults, NUM_SESSIONS);

        runGlobalMutexBenchmark(NUM_SESSIONS, ITERATIONS_PER_SESSION, globalResults);
        printResults("Global Mutex (OLD)", globalResults, NUM_SESSIONS);

        // Summary
        printf("\n=================================================================\n");
        printf("SUMMARY\n");
        printf("=================================================================\n");
        printf("  Average Per-Tree Wall Clock:  %.2f ms\n", avgPerTree);
        printf("  Average Global Wall Clock:    %.2f ms\n", avgGlobal);
        printf("  Speedup:                      %.2fx\n", avgGlobal / avgPerTree);
        printf("\n");

        if (avgGlobal / avgPerTree > 1.1) {
            printf("  RESULT: Per-tree mutex provides significant speedup\n");
            printf("  The optimization is validated for multi-session workloads.\n");
        } else if (avgGlobal / avgPerTree > 0.9) {
            printf("  RESULT: Per-tree mutex is similar to global mutex\n");
            printf("  This is expected for low-contention workloads.\n");
        } else {
            printf("  RESULT: Per-tree mutex is slower (unexpected)\n");
            printf("  This may indicate measurement variance or issues.\n");
        }

        // Output JSON for integration with benchmark suite
        printf("\n-----------------------------------------------------------------\n");
        printf("JSON Output:\n");
        printf("-----------------------------------------------------------------\n");
        printf("{\n");
        printf("  \"benchmark\": \"multisession_cow\",\n");
        printf("  \"timestamp\": \"%s\",\n", [[[NSDate date] description] UTF8String]);
        printf("  \"config\": {\n");
        printf("    \"num_sessions\": %d,\n", NUM_SESSIONS);
        printf("    \"iterations_per_session\": %d,\n", ITERATIONS_PER_SESSION);
        printf("    \"line_block_size\": %d,\n", LINE_BLOCK_SIZE);
        printf("    \"num_runs\": %d\n", numRuns);
        printf("  },\n");
        printf("  \"results\": {\n");
        printf("    \"per_tree_mutex_ms\": %.3f,\n", avgPerTree);
        printf("    \"global_mutex_ms\": %.3f,\n", avgGlobal);
        printf("    \"speedup\": %.3f\n", avgGlobal / avgPerTree);
        printf("  }\n");
        printf("}\n");

        return 0;
    }
}
