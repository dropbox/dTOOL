//
//  pius_map_benchmark.mm
//  DashTerm2
//
//  Benchmark comparing std::map vs std::unordered_map for pointer key lookups.
//  This measures the optimization from iteration #135 where _pius was converted
//  from std::map to std::unordered_map for O(1) lookup instead of O(log n).
//
//  Build and run:
//    clang++ -std=c++17 -O3 -framework Foundation \
//      benchmarks/pius_map_benchmark.mm -o benchmarks/pius_map_benchmark
//    ./benchmarks/pius_map_benchmark
//

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>
#include <map>
#include <unordered_map>
#include <vector>
#include <random>

// Simulate TexturePage pointer (just using void* for simplicity)
typedef void *TexturePagePtr;

// Simulate PIUArray (simplified)
struct PIUArray_sim {
    float *data;
    size_t count;
    size_t capacity;

    PIUArray_sim(size_t cap) : count(0), capacity(cap) {
        data = new float[cap * 4];
    }
    ~PIUArray_sim() {
        delete[] data;
    }

    float *get_next() {
        if (count >= capacity)
            return &data[(capacity - 1) * 4];
        return &data[count++ * 4];
    }
};

static mach_timebase_info_data_t timebaseInfo;

static uint64_t getNanoseconds(uint64_t machTime) {
    if (timebaseInfo.denom == 0) {
        mach_timebase_info(&timebaseInfo);
    }
    return machTime * timebaseInfo.numer / timebaseInfo.denom;
}

// Benchmark std::map find and insert operations
static double benchmarkStdMap(std::vector<TexturePagePtr> &pages, int numLookups, int warmupIterations,
                              int measureIterations) {
    std::map<TexturePagePtr, PIUArray_sim *> map;

    // Warmup
    for (int iter = 0; iter < warmupIterations; iter++) {
        map.clear();
        for (int i = 0; i < numLookups; i++) {
            TexturePagePtr page = pages[i % pages.size()];
            auto it = map.find(page);
            if (it == map.end()) {
                map[page] = new PIUArray_sim(100);
            }
        }
        // Cleanup
        for (auto &pair : map) {
            delete pair.second;
        }
    }

    // Measure
    uint64_t totalTime = 0;
    for (int iter = 0; iter < measureIterations; iter++) {
        map.clear();

        uint64_t start = mach_absolute_time();
        for (int i = 0; i < numLookups; i++) {
            TexturePagePtr page = pages[i % pages.size()];
            auto it = map.find(page);
            if (it == map.end()) {
                map[page] = new PIUArray_sim(100);
            } else {
                // Simulate getting next PIU
                it->second->get_next();
            }
        }
        uint64_t end = mach_absolute_time();
        totalTime += getNanoseconds(end - start);

        // Cleanup
        for (auto &pair : map) {
            delete pair.second;
        }
    }

    return (double)totalTime / measureIterations / 1e6; // Return ms
}

// Benchmark std::unordered_map find and insert operations
static double benchmarkUnorderedMap(std::vector<TexturePagePtr> &pages, int numLookups, int warmupIterations,
                                    int measureIterations) {
    std::unordered_map<TexturePagePtr, PIUArray_sim *> map;

    // Warmup
    for (int iter = 0; iter < warmupIterations; iter++) {
        map.clear();
        for (int i = 0; i < numLookups; i++) {
            TexturePagePtr page = pages[i % pages.size()];
            auto it = map.find(page);
            if (it == map.end()) {
                map[page] = new PIUArray_sim(100);
            }
        }
        // Cleanup
        for (auto &pair : map) {
            delete pair.second;
        }
    }

    // Measure
    uint64_t totalTime = 0;
    for (int iter = 0; iter < measureIterations; iter++) {
        map.clear();

        uint64_t start = mach_absolute_time();
        for (int i = 0; i < numLookups; i++) {
            TexturePagePtr page = pages[i % pages.size()];
            auto it = map.find(page);
            if (it == map.end()) {
                map[page] = new PIUArray_sim(100);
            } else {
                // Simulate getting next PIU
                it->second->get_next();
            }
        }
        uint64_t end = mach_absolute_time();
        totalTime += getNanoseconds(end - start);

        // Cleanup
        for (auto &pair : map) {
            delete pair.second;
        }
    }

    return (double)totalTime / measureIterations / 1e6; // Return ms
}

// Benchmark with pre-populated map (cache hits only)
static double benchmarkStdMapCacheHits(std::vector<TexturePagePtr> &pages, int numLookups, int warmupIterations,
                                       int measureIterations) {
    std::map<TexturePagePtr, PIUArray_sim *> map;

    // Pre-populate
    for (auto page : pages) {
        map[page] = new PIUArray_sim(numLookups / (int)pages.size() + 100);
    }

    // Warmup
    for (int iter = 0; iter < warmupIterations; iter++) {
        for (int i = 0; i < numLookups; i++) {
            TexturePagePtr page = pages[i % pages.size()];
            auto it = map.find(page);
            if (it != map.end()) {
                it->second->get_next();
            }
        }
        // Reset counts
        for (auto &pair : map) {
            pair.second->count = 0;
        }
    }

    // Measure
    uint64_t totalTime = 0;
    for (int iter = 0; iter < measureIterations; iter++) {
        // Reset counts
        for (auto &pair : map) {
            pair.second->count = 0;
        }

        uint64_t start = mach_absolute_time();
        for (int i = 0; i < numLookups; i++) {
            TexturePagePtr page = pages[i % pages.size()];
            auto it = map.find(page);
            if (it != map.end()) {
                it->second->get_next();
            }
        }
        uint64_t end = mach_absolute_time();
        totalTime += getNanoseconds(end - start);
    }

    // Cleanup
    for (auto &pair : map) {
        delete pair.second;
    }

    return (double)totalTime / measureIterations / 1e6; // Return ms
}

static double benchmarkUnorderedMapCacheHits(std::vector<TexturePagePtr> &pages, int numLookups, int warmupIterations,
                                             int measureIterations) {
    std::unordered_map<TexturePagePtr, PIUArray_sim *> map;

    // Pre-populate
    for (auto page : pages) {
        map[page] = new PIUArray_sim(numLookups / (int)pages.size() + 100);
    }

    // Warmup
    for (int iter = 0; iter < warmupIterations; iter++) {
        for (int i = 0; i < numLookups; i++) {
            TexturePagePtr page = pages[i % pages.size()];
            auto it = map.find(page);
            if (it != map.end()) {
                it->second->get_next();
            }
        }
        // Reset counts
        for (auto &pair : map) {
            pair.second->count = 0;
        }
    }

    // Measure
    uint64_t totalTime = 0;
    for (int iter = 0; iter < measureIterations; iter++) {
        // Reset counts
        for (auto &pair : map) {
            pair.second->count = 0;
        }

        uint64_t start = mach_absolute_time();
        for (int i = 0; i < numLookups; i++) {
            TexturePagePtr page = pages[i % pages.size()];
            auto it = map.find(page);
            if (it != map.end()) {
                it->second->get_next();
            }
        }
        uint64_t end = mach_absolute_time();
        totalTime += getNanoseconds(end - start);
    }

    // Cleanup
    for (auto &pair : map) {
        delete pair.second;
    }

    return (double)totalTime / measureIterations / 1e6; // Return ms
}

int main(int argc, char *argv[]) {
    @autoreleasepool {
        printf("================================================================\n");
        printf("PIU Map Lookup Benchmark - std::map vs std::unordered_map\n");
        printf("================================================================\n\n");
        printf("This benchmark measures the performance improvement from converting\n");
        printf("the _pius map from std::map to std::unordered_map in iteration #135.\n");
        printf("Key type is TexturePage* (pointer), which is trivially hashable.\n\n");

        const int warmupIterations = 5;
        const int measureIterations = 20;

        // Test configurations: varying number of texture pages (typical is 1-16)
        int pageCounts[] = {1, 2, 4, 8, 16, 32, 64};
        int lookupCounts[] = {1920, 6000, 20000, 50000}; // Different screen sizes

        printf("Configuration: %d warmup iterations, %d measurement iterations\n\n", warmupIterations,
               measureIterations);

        printf("%-15s %-15s %-15s %-15s %-15s\n", "Pages", "Lookups", "std::map (ms)", "unordered (ms)", "Speedup");
        printf("-------------------------------------------------------------------------------\n");

        for (int pageCount : pageCounts) {
            // Create simulated texture page pointers
            std::vector<TexturePagePtr> pages;
            for (int i = 0; i < pageCount; i++) {
                // Simulate pointer addresses (just unique values)
                pages.push_back((void *)(uintptr_t)(0x1000 + i * 0x100));
            }

            for (int lookupCount : lookupCounts) {
                double mapTime = benchmarkStdMap(pages, lookupCount, warmupIterations, measureIterations);
                double unorderedTime = benchmarkUnorderedMap(pages, lookupCount, warmupIterations, measureIterations);
                double speedup = mapTime / unorderedTime;

                printf("%-15d %-15d %-15.4f %-15.4f %-15.2fx\n", pageCount, lookupCount, mapTime, unorderedTime,
                       speedup);
            }
        }

        printf("\n");
        printf("================================================================\n");
        printf("Cache Hit Only Benchmark (pre-populated map, pure lookup cost)\n");
        printf("================================================================\n\n");

        printf("%-15s %-15s %-15s %-15s %-15s\n", "Pages", "Lookups", "std::map (ms)", "unordered (ms)", "Speedup");
        printf("-------------------------------------------------------------------------------\n");

        for (int pageCount : pageCounts) {
            std::vector<TexturePagePtr> pages;
            for (int i = 0; i < pageCount; i++) {
                pages.push_back((void *)(uintptr_t)(0x1000 + i * 0x100));
            }

            for (int lookupCount : lookupCounts) {
                double mapTime = benchmarkStdMapCacheHits(pages, lookupCount, warmupIterations, measureIterations);
                double unorderedTime =
                    benchmarkUnorderedMapCacheHits(pages, lookupCount, warmupIterations, measureIterations);
                double speedup = mapTime / unorderedTime;

                printf("%-15d %-15d %-15.4f %-15.4f %-15.2fx\n", pageCount, lookupCount, mapTime, unorderedTime,
                       speedup);
            }
        }

        printf("\n");
        printf("Analysis:\n");
        printf("---------\n");
        printf("- std::map uses a red-black tree with O(log n) lookup complexity\n");
        printf("- std::unordered_map uses a hash table with O(1) average lookup\n");
        printf("- For pointer keys, hashing is trivial (identity hash)\n");
        printf("- The speedup is most noticeable with larger maps and more lookups\n");
        printf("- Typical terminal rendering uses 1-16 texture pages per frame\n");
        printf("\n");
    }
    return 0;
}
