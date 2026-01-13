//
//  iTermTexturePageCollection.h
//  DashTerm2
//
//  Created by George Nachman on 12/22/17.
//

#import <Metal/Metal.h>
#import "iTermGlyphEntry.h"
#import "iTermMetalBufferPool.h"
#import "iTermTexturePage.h"
#include <unordered_map>
#include <unordered_set>
#include <list>

namespace DashTerm2 {
    // Holds a collection of DashTerm2::TexturePages. Provides an interface for finding a GlyphEntry
    // for a GlyphKey, adding a new glyph, and pruning disused texture pages. Tries to be fast.
    //
    // LRU Optimization (Batch 2.2):
    // Previously used O(n log n) sort on every prune. Now maintains an LRU list with O(1) operations:
    // - page_was_used() moves page to front of list: O(1)
    // - prune_if_needed() evicts from back of list: O(1) per eviction
    class TexturePageCollection : TexturePageOwner, public TexturePageLRUCallback {
    public:
        TexturePageCollection(id<MTLDevice> device,
                              const vector_uint2 cellSize,
                              const int pageCapacity,
                              const int maximumNumberOfPages) :
        _device(device),
        _cellSize(cellSize),
        _pageCapacity(pageCapacity),
        _maximumNumberOfPages(maximumNumberOfPages),
        _openPage(NULL) { }

        virtual ~TexturePageCollection() {
            // BUG-7220 fix: Handle potential page deletion when releasing ownership.
            // Pages may be deleted when their last owner releases them.
            if (_openPage) {
                _openPage->assert_valid();
                if (_openPage->release(this)) {
                    delete _openPage;
                }
                _openPage = NULL;
            }
            // Modifying _allPages during iteration, so avoid const reference
            for (auto page : _allPages) {
                page->assert_valid();
                if (page->release(this)) {
                    delete page;
                }
            }
            for (auto it = _pages.begin(); it != _pages.end(); it++) {
                std::vector<const GlyphEntry *> *vector = it->second;
                // Use const reference to avoid copying the pointer on each iteration
                for (const auto& glyph_entry : *vector) {
                    delete glyph_entry;
                }
                delete vector;
            }
        }

        // Returns a collection of glyph entries for a glyph key, or NULL if none exists.
        std::vector<const GlyphEntry *> *find(const GlyphKey &glyphKey) const {
            auto const it = _pages.find(glyphKey);
            if (it == _pages.end()) {
                return NULL;
            } else {
                return it->second;
            }
        }

        // Adds a collection of glyph entries for a glyph key, allocating a new texture page if
        // needed.
        std::vector<const GlyphEntry *> *add(int column,
                                             const GlyphKey &glyphKey,
                                             iTermMetalBufferPoolContext *context,
                                             NSDictionary<NSNumber *, iTermCharacterBitmap *> *(^creator)(int, BOOL *)) {
            BOOL emoji;
            NSDictionary<NSNumber *, iTermCharacterBitmap *> *images = creator(column, &emoji);
            std::vector<const GlyphEntry *> *result = new std::vector<const GlyphEntry *>();
            _pages[glyphKey] = result;
            for (NSNumber *partNumber in images) {
                iTermCharacterBitmap *image = images[partNumber];
                const GlyphEntry *entry = internal_add(partNumber.intValue, glyphKey, image, emoji, context);
                result->push_back(entry);
            }
            DLog(@"Added %@. Count is now %@", glyphKey.description(), @(_allPages.size() * _pageCapacity));

            return result;
        }

        const vector_uint2 &get_cell_size() const {
            return _cellSize;
        }

        // Flush all staged glyph uploads across all pages to GPU.
        // Call before rendering to ensure all textures are up to date.
        void flush_all_staged_uploads() {
            if (_openPage && _openPage->get_pending_upload_count() > 0) {
                _openPage->flush_staged_uploads();
            }
            // Note: Non-open pages shouldn't have pending uploads since they
            // get flushed when they become full. But iterate for safety.
            for (auto page : _allPages) {
                if (page->get_pending_upload_count() > 0) {
                    page->flush_staged_uploads();
                }
            }
        }

        // TexturePageLRUCallback implementation: called when a page is used.
        // Moves the page to the front of the LRU list. O(1) operation.
        virtual void page_was_used(TexturePage *page) override {
            auto it = _lruMap.find(page);
            if (it != _lruMap.end()) {
                // Move to front of list (most recently used)
                _lruList.splice(_lruList.begin(), _lruList, it->second);
            }
            // Note: page might not be in map if created but not yet added to LRU list.
            // This can happen briefly during internal_add. That's OK - page gets
            // added to LRU list at end of internal_add.
        }

        // Discard least-recently used texture pages.
        // O(k) where k = number of pages to evict, instead of O(n log n) for sort.
        void prune_if_needed() {
            if (is_over_maximum_size()) {
                ELog(@"Pruning. Have %@ pages. Each page stores up to %@ glyphs. Max pages is %@",
                     @(_allPages.size()),
                     @(_pageCapacity),
                     @(_maximumNumberOfPages));

                // Evict from back of LRU list (least recently used).
                // O(1) per eviction instead of O(n log n) sort.
                while (is_over_maximum_size() && !_lruList.empty()) {
                    TexturePage *pageToPrune = _lruList.back();
                    ITOwnershipLog(@"OWNERSHIP: Begin pruning page %p", pageToPrune);
                    pageToPrune->assert_valid();
                    internal_prune(pageToPrune);
                    ITOwnershipLog(@"OWNERSHIP: Done pruning page %p", pageToPrune);
                }
            } else {
                DLog(@"Not pruning");
            }
        }

        void remove_all() {
            std::vector<TexturePage *> pages;
            std::copy(_allPages.begin(), _allPages.end(), std::back_inserter(pages));
            for (int i = 0; i < pages.size(); i++) {
                TexturePage *pageToPrune = pages[i];
                internal_prune(pageToPrune);
            }
        }

    private:
        const GlyphEntry *internal_add(int part, const GlyphKey &key, iTermCharacterBitmap *image, bool is_emoji, iTermMetalBufferPoolContext *context) {
            if (!_openPage) {
                // Pass 'this' as LRU callback so page can notify us when used
                _openPage = new TexturePage(this, _device, _pageCapacity, _cellSize, this);
                [context didAddTextureOfSize:_cellSize.x * _cellSize.y * _pageCapacity];
                // Add to allPages and retain that reference too
                _allPages.insert(_openPage);
                _openPage->retain(this);
                _openPage->assert_valid();

                // Add new page to front of LRU list (most recently used)
                _lruList.push_front(_openPage);
                _lruMap[_openPage] = _lruList.begin();
            }

            TexturePage *openPage = _openPage;
            openPage->assert_valid();
            ITExtraDebugAssert(_openPage->get_available_count() > 0);
            const GlyphEntry *result = new GlyphEntry(part,
                                                      key,
                                                      openPage,
                                                      openPage->add_image(image, is_emoji),
                                                      is_emoji);
            if (openPage->get_available_count() == 0) {
                // Page is full - flush any staged uploads before releasing
                openPage->flush_staged_uploads();

                // BUG-7220 fix: Check if page should be deleted (shouldn't happen here
                // since allPages and glyph entry still own it, but be defensive).
                if (openPage->release(this)) {
                    delete openPage;
                }
                _openPage = NULL;
            }
            return result;
        }

        // Remove all references to `pageToPrune` and all glyph entries that reference the page.
        // BUG-7220 fix: Defer deletion until after all references are released.
        void internal_prune(TexturePage *pageToPrune) {
            pageToPrune->assert_valid();
            bool shouldDelete = false;

            // Remove from LRU list first (O(1) via iterator lookup)
            auto lruIt = _lruMap.find(pageToPrune);
            if (lruIt != _lruMap.end()) {
                _lruList.erase(lruIt->second);
                _lruMap.erase(lruIt);
            }

            if (pageToPrune == _openPage) {
                shouldDelete = pageToPrune->release(this) || shouldDelete;
                _openPage = NULL;
            }
            _allPages.erase(pageToPrune);
            shouldDelete = pageToPrune->release(this) || shouldDelete;

            // Make all glyph entries remove their references to the page. Remove our
            // references to the glyph entries.
            // Get owners snapshot before we start releasing, as page is still valid here.
            auto owners = pageToPrune->get_owners();  // unordered_map<TexturePageOwner *, int>
            ITOwnershipLog(@"OWNERSHIP: page %p has %d owners", pageToPrune, (int)owners.size());
            // Use const reference to avoid copying the map pair on each iteration
            for (const auto& pair : owners) {
                auto owner = pair.first;  // TexturePageOwner *
                auto count = pair.second;  // int
                ITOwnershipLog(@"OWNERSHIP: remove all %d references by owner %p", count, owner);
                for (int j = 0; j < count; j++) {
                    if (owner->texture_page_owner_is_glyph_entry()) {
                        GlyphEntry *glyph_entry = static_cast<GlyphEntry *>(owner);
                        shouldDelete = pageToPrune->release(glyph_entry) || shouldDelete;
                        auto it = _pages.find(glyph_entry->_key);
                        if (it != _pages.end()) {
                            // Remove from _pages as soon as the first part is found for this glyph
                            // key. Subsequent parts won't need to remove an entry from _pages.
                            std::vector<const GlyphEntry *> *entries = it->second;
                            delete entries;
                            _pages.erase(it);
                        }
                    }
                }
            }

            // Now safe to delete after all references are released
            if (shouldDelete) {
                delete pageToPrune;
            }
        }

        static bool LRUComparison(TexturePage *a, TexturePage *b) {
            return a->get_last_used() < b->get_last_used();
        }

        bool is_over_maximum_size() const {
            return _allPages.size() > _maximumNumberOfPages;
        }

    private:
        TexturePageCollection &operator=(const TexturePageCollection &);
        TexturePageCollection(const TexturePageCollection &);

        id<MTLDevice> _device;
        const vector_uint2 _cellSize;
        const int _pageCapacity;
        const int _maximumNumberOfPages;
        std::unordered_map<GlyphKey, std::vector<const GlyphEntry *> *> _pages;
        std::unordered_set<TexturePage *> _allPages;  // O(1) insert/erase vs O(log n) for std::set
        TexturePage *_openPage;

        // LRU list: front = most recently used, back = least recently used
        // This enables O(1) eviction instead of O(n log n) sort
        std::list<TexturePage *> _lruList;
        // Map from page to its position in LRU list for O(1) move-to-front
        std::unordered_map<TexturePage *, std::list<TexturePage *>::iterator> _lruMap;
    };
}

@interface iTermTexturePageCollectionSharedPointer : NSObject
@property (nonatomic, readonly) DashTerm2::TexturePageCollection *object;

- (instancetype)initWithObject:(DashTerm2::TexturePageCollection *)object;
- (instancetype)init NS_UNAVAILABLE;

@end
