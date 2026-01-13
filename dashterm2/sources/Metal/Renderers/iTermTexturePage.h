//
//  iTermTexturePage.h
//  DashTerm2
//
//  Created by George Nachman on 12/22/17.
//

#import "iTermTextureArray.h"

#import <Metal/Metal.h>
#import <simd/simd.h>

extern "C" {
#import "DebugLogging.h"
}
#include <unordered_map>
#include <vector>
#include <list>

// This is useful when we over-release a texture page.
// TODO: Use shared_ptr or the like, if it can do the job.
#define ENABLE_OWNERSHIP_LOG 0
#if ENABLE_OWNERSHIP_LOG
#define ITOwnershipLog(args...) NSLog
#else
#define ITOwnershipLog(args...)
#endif

namespace DashTerm2 {
    enum dontcare {
        magic = 0xdeadbeef
    };
    class TexturePage;

    class TexturePageOwner {
    public:
        virtual bool texture_page_owner_is_glyph_entry() {
            return false;
        }
    };

    // Callback interface for LRU tracking. Implemented by TexturePageCollection.
    class TexturePageLRUCallback {
    public:
        virtual ~TexturePageLRUCallback() {}
        virtual void page_was_used(TexturePage *page) = 0;
    };

    class TexturePage {
    public:
        // Make this public so the optimizer can't make any assumptions about it.
        int _magic;

        TexturePage(TexturePageOwner *owner,
                    id<MTLDevice> device,
                    int capacity,
                    vector_uint2 cellSize,
                    TexturePageLRUCallback *lruCallback = nullptr) :
        _magic(magic),
        _capacity(capacity),
        _cell_size(cellSize),
        _count(0),
        _emoji(capacity),
        _lruCallback(lruCallback) {
            retain(owner);
            _textureArray = [[iTermTextureArray alloc] initWithTextureWidth:cellSize.x
                                                              textureHeight:cellSize.y
                                                                arrayLength:capacity
                                                                pixelFormat:MTLPixelFormatBGRA8Unorm
                                                                     device:device];
            _atlas_size = simd_make_uint2(_textureArray.atlasSize.width,
                                          _textureArray.atlasSize.height);
            _reciprocal_atlas_size = 1.0f / simd_make_float2(_atlas_size.x, _atlas_size.y);
        }

        virtual ~TexturePage() {
            _magic = 0;
            ITOwnershipLog(@"OWNERSHIP: Destructor for page %p", this);
        }

        void assert_valid() const {
            assert(_magic == magic);
        }

        int get_available_count() const {
            return _capacity - _count;
        }

        int add_image(iTermCharacterBitmap *image, bool is_emoji) {
            ITExtraDebugAssert(_count < _capacity);
            // Use batched upload API - stages bitmap instead of immediate upload
            [_textureArray stageBitmapForSlice:_count withBitmap:image];
            _emoji[_count] = is_emoji;
            return _count++;
        }

        // Flush any staged glyph uploads to GPU.
        // Call before rendering to ensure all staged bitmaps are available.
        void flush_staged_uploads() {
            [_textureArray flushStagedBitmaps];
        }

        // Returns number of staged but not-yet-uploaded glyphs
        NSUInteger get_pending_upload_count() const {
            return _textureArray.pendingUploadCount;
        }

        id<MTLTexture> get_texture() const {
            return _textureArray.texture;
        }

        iTermTextureArray *get_texture_array() const {
            return _textureArray;
        }

        bool get_is_emoji(const int index) const {
            return _emoji[index];
        }

        const vector_uint2 &get_cell_size() const {
            return _cell_size;
        }

        const vector_uint2 &get_atlas_size() const {
            return _atlas_size;
        }

        const vector_float2 &get_reciprocal_atlas_size() const {
            return _reciprocal_atlas_size;
        }

        void retain(TexturePageOwner *owner) {
            _owners[owner]++;
            ITOwnershipLog(@"OWNERSHIP: retain %p as owner of %p with refcount %d", owner, this, (int)_owners[owner]);
        }

        // Returns true if this page should be deleted by the caller (no more owners).
        // BUG-7220 fix: No longer calls `delete this` internally - caller is responsible
        // for deletion if this returns true to avoid use-after-free.
        bool release(TexturePageOwner *owner) {
            ITOwnershipLog(@"OWNERSHIP: release %p as owner of %p. New refcount for this owner will be %d", owner, this, (int)_owners[owner]-1);
            ITExtraDebugAssert(_owners[owner] > 0);

            auto it = _owners.find(owner);
#if ENABLE_OWNERSHIP_LOG
            if (it == _owners.end()) {
                ITOwnershipLog(@"I have %d owners", (int)_owners.size());
                // Use const reference to avoid copying the map pair on each iteration
                for (const auto& pair : _owners) {
                    ITOwnershipLog(@"%p is owner", pair.first);
                }
                ITExtraDebugAssert(it != _owners.end());
            }
#endif
            it->second--;
            if (it->second == 0) {
                _owners.erase(it);
                if (_owners.empty()) {
                    ITOwnershipLog(@"OWNERSHIP: should delete page %p", this);
                    return true;
                }
            }
            return false;
        }

        void record_use() {
            static long long use_count;
            _last_used = use_count++;
            // Notify collection for O(1) LRU list update
            if (_lruCallback) {
                _lruCallback->page_was_used(this);
            }
        }

        long long get_last_used() const {
            return _last_used;
        }

        // Returns a copy of the owners map for iteration.
        // Returns unordered_map since callers only need to iterate (no ordering needed),
        // avoiding O(n log n) cost of converting to std::map.
        std::unordered_map<TexturePageOwner *, int> get_owners() const {
#if ENABLE_OWNERSHIP_LOG
            // Use const reference to avoid copying the map pair on each iteration
            for (const auto& pair : _owners) {
                ITExtraDebugAssert(pair.second > 0);
            }
#endif
            return _owners;  // O(n) copy vs O(n log n) for std::map conversion
        }

        // This is for debugging purposes only.
        int get_retain_count() const {
            int sum = 0;
            // Use const reference to avoid copying the map pair on each iteration
            for (const auto& pair : _owners) {
                sum += pair.second;
            }
            return sum;
        }

    private:
        TexturePage();
        TexturePage &operator=(const TexturePage &);
        TexturePage(const TexturePage &);

        iTermTextureArray *_textureArray;
        int _capacity;
        vector_uint2 _cell_size;
        vector_uint2 _atlas_size;
        int _count;
        std::vector<bool> _emoji;
        vector_float2 _reciprocal_atlas_size;
        // Use unordered_map for O(1) lookup instead of std::map's O(log n).
        // This is a performance optimization since retain/release are called
        // frequently during rendering for every glyph that uses a texture page.
        std::unordered_map<TexturePageOwner *, int> _owners;
        long long _last_used;
        // Callback for LRU tracking - not owned, may be null.
        TexturePageLRUCallback *_lruCallback;
    };
}

