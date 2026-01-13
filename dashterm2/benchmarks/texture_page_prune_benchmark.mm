// texture_page_prune_benchmark.mm
// Benchmarks texture page collection pruning performance
//
// Build:
//   clang++ -std=c++17 -framework Foundation -fobjc-arc -O3 \
//     benchmarks/texture_page_prune_benchmark.mm -o benchmarks/texture_page_prune_benchmark
//
// Run:
//   ./benchmarks/texture_page_prune_benchmark

#import <Foundation/Foundation.h>
#include <chrono>
#include <vector>
#include <set>
#include <list>
#include <algorithm>
#include <random>

// Simulate TexturePage's timestamp-based LRU tracking
struct MockTexturePage {
    long long lastUsed;
    int id;

    MockTexturePage(int pageId) : id(pageId), lastUsed(0) {}
};

// Current implementation: copy + sort
class CurrentApproach {
public:
    std::set<MockTexturePage*> allPages;

    void addPage(MockTexturePage* page) {
        allPages.insert(page);
    }

    void recordUse(MockTexturePage* page) {
        static long long counter = 0;
        page->lastUsed = counter++;
    }

    // O(n log n) sort for LRU ordering
    std::vector<MockTexturePage*> getLRUSorted() {
        std::vector<MockTexturePage*> pages;
        pages.reserve(allPages.size());
        std::copy(allPages.begin(), allPages.end(), std::back_inserter(pages));
        std::sort(pages.begin(), pages.end(), [](MockTexturePage* a, MockTexturePage* b) {
            return a->lastUsed < b->lastUsed;
        });
        return pages;
    }
};

// Optimized: maintain LRU order with doubly-linked list
class OptimizedApproach {
public:
    std::set<MockTexturePage*> allPages;
    std::list<MockTexturePage*> lruOrder;  // front = oldest, back = newest
    std::unordered_map<MockTexturePage*, std::list<MockTexturePage*>::iterator> lruMap;

    void addPage(MockTexturePage* page) {
        allPages.insert(page);
        lruOrder.push_back(page);
        lruMap[page] = std::prev(lruOrder.end());
    }

    void recordUse(MockTexturePage* page) {
        static long long counter = 0;
        page->lastUsed = counter++;

        // Move to end of LRU list
        auto it = lruMap.find(page);
        if (it != lruMap.end()) {
            lruOrder.erase(it->second);
            lruOrder.push_back(page);
            lruMap[page] = std::prev(lruOrder.end());
        }
    }

    // O(1) access to LRU ordered list
    std::vector<MockTexturePage*> getLRUSorted() {
        std::vector<MockTexturePage*> pages;
        pages.reserve(lruOrder.size());
        for (auto page : lruOrder) {
            pages.push_back(page);
        }
        return pages;
    }
};

// Partial-sort approach: only sort what we need to prune
class PartialSortApproach {
public:
    std::set<MockTexturePage*> allPages;

    void addPage(MockTexturePage* page) {
        allPages.insert(page);
    }

    void recordUse(MockTexturePage* page) {
        static long long counter = 0;
        page->lastUsed = counter++;
    }

    // O(n log k) where k = number to prune
    std::vector<MockTexturePage*> getLRUSorted(int numToPrune) {
        std::vector<MockTexturePage*> pages;
        pages.reserve(allPages.size());
        std::copy(allPages.begin(), allPages.end(), std::back_inserter(pages));

        // Only partially sort - get the k oldest pages
        std::partial_sort(pages.begin(), pages.begin() + numToPrune, pages.end(),
            [](MockTexturePage* a, MockTexturePage* b) {
                return a->lastUsed < b->lastUsed;
            });
        return pages;
    }
};

void runBenchmark(int numPages, int numToPrune, int numUsesPerFrame, int numFrames) {
    printf("\nConfiguration: %d pages, prune %d, %d uses/frame, %d frames\n",
           numPages, numToPrune, numUsesPerFrame, numFrames);

    std::random_device rd;
    std::mt19937 gen(rd());

    // Setup: Create pages
    std::vector<MockTexturePage*> pages;
    for (int i = 0; i < numPages; i++) {
        pages.push_back(new MockTexturePage(i));
    }

    // Current approach
    {
        CurrentApproach current;
        for (auto page : pages) {
            current.addPage(page);
        }

        // Simulate usage over frames
        std::uniform_int_distribution<> dist(0, numPages - 1);
        for (int frame = 0; frame < numFrames / 2; frame++) {
            for (int i = 0; i < numUsesPerFrame; i++) {
                current.recordUse(pages[dist(gen)]);
            }
        }

        // Time the pruning operation
        auto start = std::chrono::high_resolution_clock::now();
        for (int i = 0; i < numFrames; i++) {
            auto sorted = current.getLRUSorted();
            (void)sorted[0];  // Prevent optimization
        }
        auto end = std::chrono::high_resolution_clock::now();
        double ms = std::chrono::duration<double, std::milli>(end - start).count();
        printf("  Current (full sort):    %.3f ms total, %.4f ms/prune\n", ms, ms / numFrames);
    }

    // Optimized approach (doubly-linked list)
    {
        OptimizedApproach optimized;
        for (auto page : pages) {
            optimized.addPage(page);
        }

        std::uniform_int_distribution<> dist(0, numPages - 1);
        for (int frame = 0; frame < numFrames / 2; frame++) {
            for (int i = 0; i < numUsesPerFrame; i++) {
                optimized.recordUse(pages[dist(gen)]);
            }
        }

        auto start = std::chrono::high_resolution_clock::now();
        for (int i = 0; i < numFrames; i++) {
            auto sorted = optimized.getLRUSorted();
            (void)sorted[0];
        }
        auto end = std::chrono::high_resolution_clock::now();
        double ms = std::chrono::duration<double, std::milli>(end - start).count();
        printf("  Optimized (linked list): %.3f ms total, %.4f ms/prune\n", ms, ms / numFrames);
    }

    // Partial sort approach
    {
        PartialSortApproach partial;
        for (auto page : pages) {
            partial.addPage(page);
        }

        std::uniform_int_distribution<> dist(0, numPages - 1);
        for (int frame = 0; frame < numFrames / 2; frame++) {
            for (int i = 0; i < numUsesPerFrame; i++) {
                partial.recordUse(pages[dist(gen)]);
            }
        }

        auto start = std::chrono::high_resolution_clock::now();
        for (int i = 0; i < numFrames; i++) {
            auto sorted = partial.getLRUSorted(numToPrune);
            (void)sorted[0];
        }
        auto end = std::chrono::high_resolution_clock::now();
        double ms = std::chrono::duration<double, std::milli>(end - start).count();
        printf("  Partial sort (k=%d):    %.3f ms total, %.4f ms/prune\n", numToPrune, ms, ms / numFrames);
    }

    // Now benchmark recordUse overhead
    printf("\n  recordUse overhead comparison:\n");
    {
        CurrentApproach current;
        for (auto page : pages) {
            current.addPage(page);
        }
        std::uniform_int_distribution<> dist(0, numPages - 1);

        auto start = std::chrono::high_resolution_clock::now();
        for (int frame = 0; frame < numFrames * 100; frame++) {
            for (int i = 0; i < numUsesPerFrame; i++) {
                current.recordUse(pages[dist(gen)]);
            }
        }
        auto end = std::chrono::high_resolution_clock::now();
        double ms = std::chrono::duration<double, std::milli>(end - start).count();
        printf("    Current recordUse:   %.3f ms for %d calls\n", ms, numFrames * 100 * numUsesPerFrame);
    }

    {
        OptimizedApproach optimized;
        for (auto page : pages) {
            optimized.addPage(page);
        }
        std::uniform_int_distribution<> dist(0, numPages - 1);

        auto start = std::chrono::high_resolution_clock::now();
        for (int frame = 0; frame < numFrames * 100; frame++) {
            for (int i = 0; i < numUsesPerFrame; i++) {
                optimized.recordUse(pages[dist(gen)]);
            }
        }
        auto end = std::chrono::high_resolution_clock::now();
        double ms = std::chrono::duration<double, std::milli>(end - start).count();
        printf("    Optimized recordUse: %.3f ms for %d calls (list maintenance)\n", ms, numFrames * 100 * numUsesPerFrame);
    }

    // Cleanup
    for (auto page : pages) {
        delete page;
    }
}

int main(int argc, const char* argv[]) {
    @autoreleasepool {
        printf("Texture Page Pruning Benchmark\n");
        printf("===============================\n");
        printf("\nThis benchmark measures the cost of texture page pruning.\n");
        printf("Pruning only occurs when texture page count exceeds 4096.\n");
        printf("In practice, this is rare (requires 64K+ unique non-ASCII glyphs).\n");

        // Realistic scenarios
        printf("\n--- Scenario 1: Typical CJK usage (100 pages) ---\n");
        runBenchmark(100, 10, 50, 100);

        printf("\n--- Scenario 2: Heavy Unicode (500 pages) ---\n");
        runBenchmark(500, 50, 100, 100);

        printf("\n--- Scenario 3: Near limit (4000 pages) ---\n");
        runBenchmark(4000, 100, 200, 100);

        printf("\n--- Scenario 4: At limit, single prune (4097 pages, prune 1) ---\n");
        runBenchmark(4097, 1, 200, 100);

        printf("\n--- Scenario 5: Extreme case (4096 pages) ---\n");
        runBenchmark(4096, 512, 200, 100);

        printf("\n\nConclusion:\n");
        printf("===========\n");
        printf("Pruning is rare (only when >4096 pages with >64K unique glyphs).\n");
        printf("The cost tradeoffs are:\n");
        printf("  - Current: O(1) recordUse, O(n log n) prune\n");
        printf("  - Linked list: O(1) prune, but slower recordUse (list maintenance)\n");
        printf("  - Partial sort: O(n log k) prune, O(1) recordUse\n");
        printf("\nSince recordUse is called thousands of times per frame and\n");
        printf("pruning is rare, the current approach may already be optimal.\n");
    }
    return 0;
}
