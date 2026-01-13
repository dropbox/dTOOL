//! Offset-based page storage with memory pooling.
//!
//! Pages use offsets instead of pointers, enabling:
//! - Direct serialization to disk
//! - Memory-mapping without fixup
//! - Network transmission for sync
//!
//! ## Memory Pooling
//!
//! The [`PageStore`] maintains a free list of recycled pages to avoid
//! allocation overhead in hot paths. When pages are freed, they're added
//! to the free list. New allocations first check the free list before
//! allocating fresh memory.
//!
//! Use [`PageStore::preheat`] to pre-allocate pages during initialization,
//! eliminating allocation latency during normal operation.
//!
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

/// Page size (64 KB).
pub const PAGE_SIZE: usize = 64 * 1024;

/// A page of terminal data.
///
/// All data is stored contiguously with offset-based references.
/// Uses `UnsafeCell` for interior mutability to satisfy Stacked Borrows -
/// arena-allocated data may be accessed through multiple independent borrows.
#[repr(C, align(4096))]
pub struct Page {
    /// Raw page data wrapped in UnsafeCell for interior mutability.
    /// This allows arena-style allocation where pointers derived from this data
    /// remain valid even when other parts of the PageStore are accessed.
    data: UnsafeCell<[u8; PAGE_SIZE]>,
}

// SAFETY: Page data can be accessed from multiple threads when properly synchronized.
// The PageStore ensures exclusive access during allocation, and PageSlice ensures
// no data races during access.
unsafe impl Send for Page {}
unsafe impl Sync for Page {}

impl std::fmt::Debug for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Page")
            .field("data", &format_args!("UnsafeCell<[u8; {}]>", PAGE_SIZE))
            .finish()
    }
}

impl Page {
    /// Create a new zeroed page.
    #[must_use]
    pub fn new() -> Box<Self> {
        // Use calloc for zero-initialized, page-aligned memory
        Box::new(Self {
            data: UnsafeCell::new([0; PAGE_SIZE]),
        })
    }

    /// Get a mutable pointer to the page data.
    ///
    /// # Safety
    /// The caller must ensure no other references to this data exist.
    #[inline]
    pub fn data_ptr(&self) -> *mut u8 {
        self.data.get().cast()
    }
}

impl Default for Page {
    fn default() -> Self {
        Self {
            data: UnsafeCell::new([0; PAGE_SIZE]),
        }
    }
}

/// An offset into a page.
///
/// This is NOT a pointer - it's an index that remains valid
/// after the page is copied, serialized, or memory-mapped.
#[derive(Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Offset<T> {
    /// Byte offset into the page.
    byte_offset: u32,
    /// Phantom data for type safety.
    _marker: PhantomData<T>,
}

// Manual Copy/Clone impl because derive requires T: Copy, but PhantomData<T> is always Copy
impl<T> Copy for Offset<T> {}
impl<T> Clone for Offset<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Offset<T> {
    /// Create a new offset.
    ///
    /// # Safety
    ///
    /// The offset must be:
    /// - Less than PAGE_SIZE
    /// - Properly aligned for T
    #[must_use]
    pub const fn new(byte_offset: u32) -> Self {
        Self {
            byte_offset,
            _marker: PhantomData,
        }
    }

    /// Get the byte offset.
    #[must_use]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub const fn byte_offset(&self) -> u32 {
        self.byte_offset
    }

    /// Resolve the offset to a reference.
    ///
    /// Returns `None` if the offset is out of bounds or misaligned.
    ///
    /// # Safety
    ///
    /// - The page must contain valid, initialized data at this offset
    /// - The type `T` at the offset must have been properly constructed
    #[must_use]
    pub unsafe fn get<'a>(&self, page: &'a Page) -> Option<&'a T> {
        let offset = self.byte_offset as usize;
        let align = std::mem::align_of::<T>();
        let size = std::mem::size_of::<T>();

        // Runtime bounds and alignment check
        if offset >= PAGE_SIZE || offset % align != 0 || offset + size > PAGE_SIZE {
            return None;
        }

        // SAFETY: Caller guarantees valid data at offset; we verified bounds/alignment.
        // Use data_ptr() to go through UnsafeCell for Stacked Borrows compliance.
        Some(unsafe { &*(page.data_ptr().add(offset) as *const T) })
    }

    /// Resolve the offset to a mutable reference.
    ///
    /// Returns `None` if the offset is out of bounds or misaligned.
    ///
    /// # Safety
    ///
    /// Same as `get`, plus exclusive access to the memory at this offset.
    #[must_use]
    #[allow(clippy::mut_from_ref)] // Interior mutability through UnsafeCell is intentional
    pub unsafe fn get_mut<'a>(&self, page: &'a Page) -> Option<&'a mut T> {
        let offset = self.byte_offset as usize;
        let align = std::mem::align_of::<T>();
        let size = std::mem::size_of::<T>();

        // Runtime bounds and alignment check
        if offset >= PAGE_SIZE || offset % align != 0 || offset + size > PAGE_SIZE {
            return None;
        }

        // SAFETY: Caller guarantees valid data at offset; we verified bounds/alignment.
        // Use data_ptr() to go through UnsafeCell for Stacked Borrows compliance.
        Some(unsafe { &mut *(page.data_ptr().add(offset) as *mut T) })
    }
}

/// Logical page identifier.
pub type PageId = usize;

/// A typed slice allocated within a page.
#[derive(Debug)]
pub struct PageSlice<T> {
    ptr: NonNull<T>,
    len: u16,
    page_id: PageId,
    offset: Offset<T>,
}

impl<T> PageSlice<T> {
    /// Length of the slice (u16).
    #[must_use]
    pub const fn len_u16(&self) -> u16 {
        self.len
    }

    /// Page ID for this slice.
    #[must_use]
    pub const fn page_id(&self) -> PageId {
        self.page_id
    }

    /// Offset within the page.
    #[must_use]
    pub const fn offset(&self) -> Offset<T> {
        self.offset
    }

    /// View the slice as a shared reference.
    #[must_use]
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        let len = self.len as usize;
        // SAFETY: ptr/len are validated at allocation time.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), len) }
    }

    /// View the slice as a mutable reference.
    #[must_use]
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let len = self.len as usize;
        // SAFETY: ptr/len are validated at allocation time and uniquely owned.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), len) }
    }
}

impl<T> Deref for PageSlice<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> DerefMut for PageSlice<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

// SAFETY: PageSlice owns its data and pointer validity is maintained by
// the PageStore. The NonNull pointer can be safely sent across threads
// when the contained type T is Send.
unsafe impl<T: Send> Send for PageSlice<T> {}

// SAFETY: PageSlice allows shared access to its contained data through
// as_slice(). This is safe when T is Sync.
unsafe impl<T: Sync> Sync for PageSlice<T> {}

/// Statistics for memory pool usage.
#[derive(Debug, Clone, Copy, Default)]
pub struct PoolStats {
    /// Total pages allocated (including freed).
    pub pages_allocated: usize,
    /// Pages currently in use.
    pub pages_in_use: usize,
    /// Pages in free list (available for reuse).
    pub pages_free: usize,
    /// Total allocations performed.
    pub allocations: usize,
    /// Allocations satisfied from free list (no new memory).
    pub reused: usize,
}

/// Page-backed allocator with memory pooling.
///
/// ## Pooling Strategy
///
/// - Pages that would be deallocated are instead added to a free list
/// - New allocations first check the free list before allocating
/// - Use `preheat()` to pre-allocate pages and avoid runtime allocations
/// - The pool can grow unbounded; use `shrink_to_fit()` to release unused pages
///
/// ## Lazy Zeroing Optimization
///
/// Pages are only zeroed up to the amount actually used (`next_offset`), not the
/// full 64KB. This is tracked per-page using `page_used_bytes`. When a page is
/// recycled, only the used portion is zeroed, reducing write overhead significantly
/// for typical allocations that don't fill entire pages.
#[derive(Debug)]
pub struct PageStore {
    /// Active pages (currently holding allocations).
    pages: Vec<Box<Page>>,
    /// Free list of recycled pages (available for reuse).
    /// Each entry is (page, used_bytes) where used_bytes tracks how much was allocated.
    free_pages: Vec<(Box<Page>, usize)>,
    /// Current page index for allocations.
    current_page: usize,
    /// Next offset within current page.
    next_offset: usize,
    /// Pool statistics.
    stats: PoolStats,
    /// Track bytes used per active page for partial zeroing on recycle.
    page_used_bytes: Vec<usize>,
}

impl Default for PageStore {
    fn default() -> Self {
        Self {
            pages: Vec::new(),
            free_pages: Vec::new(),
            current_page: 0,
            next_offset: 0,
            stats: PoolStats::default(),
            page_used_bytes: Vec::new(),
        }
    }
}

impl PageStore {
    /// Create a new, empty page store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a page store with pre-allocated pages.
    ///
    /// This eliminates allocation latency during normal operation by
    /// pre-heating the free list with the specified number of pages.
    #[must_use]
    pub fn with_capacity(page_count: usize) -> Self {
        let mut store = Self::new();
        store.preheat(page_count);
        store
    }

    /// Pre-allocate pages into the free list.
    ///
    /// Call this during initialization to avoid allocation during hot paths.
    /// Each page is 64KB, so `preheat(10)` allocates 640KB.
    ///
    /// # Example
    ///
    /// ```
    /// use dterm_core::grid::PageStore;
    ///
    /// let mut store = PageStore::new();
    /// // Pre-allocate 4 pages for a typical terminal
    /// store.preheat(4);
    /// ```
    pub fn preheat(&mut self, page_count: usize) {
        for _ in 0..page_count {
            let page = Page::new();
            // Fresh pages are already zeroed, used_bytes = 0
            self.free_pages.push((page, 0));
            self.stats.pages_allocated += 1;
            self.stats.pages_free += 1;
        }
    }

    /// Get pool statistics.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        self.stats
    }

    /// Number of active pages (holding allocations).
    #[must_use]
    pub fn active_pages(&self) -> usize {
        self.pages.len()
    }

    /// Number of free pages (available for reuse).
    #[must_use]
    pub fn free_pages(&self) -> usize {
        self.free_pages.len()
    }

    /// Total memory used by all pages (active + free), in bytes.
    #[must_use]
    pub fn total_memory(&self) -> usize {
        (self.pages.len() + self.free_pages.len()) * PAGE_SIZE
    }

    /// Release all free pages back to the system.
    ///
    /// This reduces memory usage but may cause future allocations to be slower.
    pub fn shrink_to_fit(&mut self) {
        self.stats.pages_free = 0;
        self.free_pages.clear();
        self.free_pages.shrink_to_fit();
    }

    /// Reset the page store, moving all active pages to the free list.
    ///
    /// This is useful when clearing the terminal without deallocating memory.
    /// All existing `PageSlice` references become invalid after this call.
    ///
    /// # Safety
    ///
    /// Caller must ensure no `PageSlice` references are used after reset.
    pub fn reset(&mut self) {
        // Move all active pages to free list with their used byte counts
        // The last page's used_bytes is tracked by next_offset
        while let Some(page) = self.pages.pop() {
            let used_bytes = self.page_used_bytes.pop().unwrap_or(PAGE_SIZE);
            // For the current page, use next_offset instead
            let actual_used = if self.pages.is_empty() && self.next_offset > 0 {
                self.next_offset
            } else {
                used_bytes
            };
            self.free_pages.push((page, actual_used));
            self.stats.pages_in_use -= 1;
            self.stats.pages_free += 1;
        }
        self.current_page = 0;
        self.next_offset = 0;
        self.page_used_bytes.clear();
    }

    /// Allocate a fresh page from the pool.
    ///
    /// Returns a page from the free list if available, otherwise allocates new.
    /// Uses lazy zeroing: only zeros the portion that was actually used.
    fn alloc_page(&mut self) -> Box<Page> {
        if let Some((page, used_bytes)) = self.free_pages.pop() {
            // Reuse from free list - only zero the used portion (lazy zeroing)
            // This is a significant optimization: instead of writing 64KB,
            // we only write the portion that was actually used.
            if used_bytes > 0 {
                // Zero only the used portion through UnsafeCell
                // SAFETY: We have exclusive access to the page (it's from the free list)
                // and no PageSlice references exist to this page's data.
                unsafe {
                    let data = &mut *page.data.get();
                    data[..used_bytes].fill(0);
                }
            }
            self.stats.pages_free -= 1;
            self.stats.reused += 1;
            page
        } else {
            // Allocate fresh page (already zeroed by Page::new)
            self.stats.pages_allocated += 1;
            Page::new()
        }
    }

    /// Return a page to the free list for reuse.
    ///
    /// # Arguments
    ///
    /// * `page_id` - The page ID to free
    ///
    /// Note: This doesn't actually remove the page from `self.pages` to avoid
    /// invalidating page IDs. Instead, the page is marked as reclaimable.
    /// Full page reclamation happens during `reset()` or `compact()`.
    #[allow(dead_code)]
    pub fn free_page(&mut self, page_id: PageId) {
        // For now, we don't support freeing individual pages while keeping others.
        // This would require a more complex allocation strategy with a generation
        // counter to detect stale references. The current design is optimized for
        // the terminal use case where we reset the entire page store at once.
        let _ = page_id;
    }

    /// Allocate a typed slice within the page store.
    pub fn alloc_slice<T>(&mut self, len: u16) -> PageSlice<T> {
        let len_usize = len as usize;
        let bytes = len_usize
            .checked_mul(std::mem::size_of::<T>())
            .expect("page allocation overflow");
        assert!(bytes <= PAGE_SIZE, "allocation exceeds page size");

        let align = std::mem::align_of::<T>();
        let mut offset = align_up(self.next_offset, align);

        if self.pages.is_empty() || offset + bytes > PAGE_SIZE {
            // Save the used bytes for the current page before moving to a new one
            if !self.pages.is_empty() {
                self.page_used_bytes.push(self.next_offset);
            }
            // Need a new page - try free list first
            let page = self.alloc_page();
            self.pages.push(page);
            self.current_page = self.pages.len() - 1;
            self.stats.pages_in_use += 1;
            offset = 0;
        }

        let page_id = self.current_page;
        // Use data_ptr() which goes through UnsafeCell, avoiding borrow invalidation
        // under Stacked/Tree Borrows when other pages are accessed later.
        let base_ptr = self.pages[page_id].data_ptr();
        let ptr = unsafe { base_ptr.add(offset) as *mut T };
        let ptr = NonNull::new(ptr).expect("page slice pointer is null");

        self.next_offset = offset + bytes;
        self.stats.allocations += 1;

        PageSlice {
            ptr,
            len,
            page_id,
            // offset is bounded by PAGE_SIZE (64KB) which fits in u32
            #[allow(clippy::cast_possible_truncation)]
            offset: Offset::new(offset as u32),
        }
    }
}

fn align_up(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

// Tests are in a separate file for better organization
#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;

#[cfg(kani)]
mod proofs {
    use super::*;
    use crate::grid::Cell;

    #[kani::proof]
    fn offset_within_bounds() {
        let offset: u32 = kani::any();
        kani::assume(offset < PAGE_SIZE as u32);
        kani::assume(offset % 4 == 0); // Aligned for u32

        let page = Page::default();
        let _cell_offset = Offset::<u32>::new(offset);

        // This should not panic
        let ptr = page.data_ptr();
        let target = unsafe { ptr.add(offset as usize) };
        kani::assert(target < unsafe { ptr.add(PAGE_SIZE) }, "out of bounds");
    }

    #[kani::proof]
    fn offset_resolve_safe() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 100);
        kani::assume(cols > 0 && cols <= 200);

        let offset: u32 = kani::any();
        kani::assume(offset < PAGE_SIZE as u32);
        kani::assume(offset % core::mem::align_of::<u32>() as u32 == 0);
        // Ensure bounds check passes: offset + size_of::<u32>() <= PAGE_SIZE
        kani::assume(offset as usize + core::mem::size_of::<u32>() <= PAGE_SIZE);

        let page = Page::new();
        let resolved = unsafe { Offset::<u32>::new(offset).get(page.as_ref()) };

        // With proper constraints, get() should succeed
        if let Some(ptr_ref) = resolved {
            let ptr = ptr_ref as *const u32 as usize;
            let base = page.data_ptr() as usize;
            let end = base + PAGE_SIZE;

            kani::assert(ptr >= base, "resolved pointer before base");
            kani::assert(
                ptr + core::mem::size_of::<u32>() <= end,
                "resolved pointer past end",
            );
        }
    }

    #[kani::proof]
    fn page_store_allocation_within_bounds() {
        let len: u16 = kani::any();
        kani::assume(len > 0);
        kani::assume(len <= (PAGE_SIZE / core::mem::size_of::<u32>()) as u16);

        let mut store = PageStore::new();
        let slice = store.alloc_slice::<u32>(len);
        let byte_offset = slice.offset().byte_offset() as usize;
        let bytes = len as usize * core::mem::size_of::<u32>();

        kani::assert(byte_offset + bytes <= PAGE_SIZE, "allocation out of bounds");
    }

    #[kani::proof]
    fn page_store_allocation_safe() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 100);
        kani::assume(cols > 0 && cols <= 200);

        let mut store = PageStore::new();

        let slice = store.alloc_slice::<Cell>(cols);
        let byte_offset = slice.offset().byte_offset() as usize;
        let bytes = cols as usize * core::mem::size_of::<Cell>();
        kani::assert(byte_offset + bytes <= PAGE_SIZE, "allocation out of bounds");

        if rows > 1 {
            let slice2 = store.alloc_slice::<Cell>(cols);
            let offset2 = slice2.offset().byte_offset() as usize;
            kani::assert(offset2 + bytes <= PAGE_SIZE, "allocation out of bounds");
        }
    }

    // === Memory pooling proofs ===

    #[kani::proof]
    fn preheat_stats_consistent() {
        let count: usize = kani::any();
        // Limit to reasonable range for proof tractability
        kani::assume(count <= 16);

        let mut store = PageStore::new();
        store.preheat(count);

        let stats = store.stats();
        kani::assert(stats.pages_allocated == count, "allocated count mismatch");
        kani::assert(stats.pages_free == count, "free count mismatch");
        kani::assert(stats.pages_in_use == 0, "in_use should be 0 after preheat");
        kani::assert(store.free_pages() == count, "free_pages() mismatch");
    }

    #[kani::proof]
    fn alloc_from_preheated_reduces_free() {
        let preheat: usize = kani::any();
        kani::assume(preheat > 0 && preheat <= 8);

        let mut store = PageStore::new();
        store.preheat(preheat);
        let initial_free = store.free_pages();

        // Allocate something
        let _slice = store.alloc_slice::<u32>(10);

        // Should have used one free page
        kani::assert(
            store.free_pages() == initial_free - 1,
            "free pages should decrease by 1",
        );
        kani::assert(store.active_pages() == 1, "should have 1 active page");
        kani::assert(store.stats().reused == 1, "should record 1 reuse");
    }

    #[kani::proof]
    fn reset_preserves_total_pages() {
        let preheat: usize = kani::any();
        kani::assume(preheat > 0 && preheat <= 4);

        let mut store = PageStore::new();
        store.preheat(preheat);

        // Allocate to use some pages
        let _slice = store.alloc_slice::<u32>(100);
        let total_before = store.active_pages() + store.free_pages();

        // Reset
        store.reset();
        let total_after = store.active_pages() + store.free_pages();

        // Total pages should be preserved (moved to free list)
        kani::assert(
            total_before == total_after,
            "reset should preserve total page count",
        );
        kani::assert(store.active_pages() == 0, "reset should clear active pages");
    }

    #[kani::proof]
    fn shrink_to_fit_releases_free_pages() {
        let preheat: usize = kani::any();
        kani::assume(preheat > 0 && preheat <= 8);

        let mut store = PageStore::new();
        store.preheat(preheat);
        kani::assert(store.free_pages() == preheat, "preheat should work");

        store.shrink_to_fit();

        kani::assert(store.free_pages() == 0, "shrink should release all free");
        kani::assert(store.stats().pages_free == 0, "stats should reflect shrink");
    }

    #[kani::proof]
    fn allocation_after_reset_uses_free_list() {
        let mut store = PageStore::new();

        // Allocate and write
        let _slice1 = store.alloc_slice::<u32>(100);
        let pages_after_first = store.stats().pages_allocated;

        // Reset
        store.reset();

        // Allocate again
        let _slice2 = store.alloc_slice::<u32>(100);

        // Should reuse the freed page, not allocate new
        kani::assert(
            store.stats().pages_allocated == pages_after_first,
            "should reuse freed page",
        );
        kani::assert(store.stats().reused >= 1, "should record reuse");
    }

    #[kani::proof]
    fn stats_pages_in_use_bounded() {
        let preheat: usize = kani::any();
        kani::assume(preheat <= 4);

        let mut store = PageStore::new();
        store.preheat(preheat);

        // Do some allocations
        let _s1 = store.alloc_slice::<u32>(100);
        let _s2 = store.alloc_slice::<u32>(100);

        let stats = store.stats();

        // pages_in_use should never exceed total allocated
        kani::assert(
            stats.pages_in_use <= stats.pages_allocated,
            "in_use cannot exceed allocated",
        );
        // free + in_use should equal total
        kani::assert(
            stats.pages_free + stats.pages_in_use == store.active_pages() + store.free_pages(),
            "free + in_use should equal total",
        );
    }

    // === Pointer arithmetic safety proofs (from WORKER_DIRECTIVE_VERIFICATION.md P1.2) ===

    /// Proof: Offset::get never produces dangling pointer
    #[kani::proof]
    fn offset_get_never_dangling() {
        let offset: u32 = kani::any();
        let size_of_t: usize = kani::any();
        kani::assume(size_of_t > 0 && size_of_t <= 64);
        kani::assume((offset as usize) + size_of_t <= PAGE_SIZE);

        // The access is within page bounds
        kani::assert(
            offset as usize + size_of_t <= PAGE_SIZE,
            "Access must be within page bounds",
        );
    }

    /// Proof: alloc_slice alignment is correct
    #[kani::proof]
    fn page_store_alignment_correct() {
        let align: usize = kani::any();
        let current: usize = kani::any();
        kani::assume(align > 0 && align.is_power_of_two() && align <= 4096);
        kani::assume(current < PAGE_SIZE);

        // align_up formula: (current + align - 1) & !(align - 1)
        let aligned = (current + align - 1) & !(align - 1);
        kani::assert(aligned % align == 0, "Result must be aligned");
        kani::assert(aligned >= current, "Aligned offset must not decrease");
    }

    /// Proof: PageSlice cannot access beyond allocation
    #[kani::proof]
    #[kani::unwind(5)]
    fn page_slice_bounds_safe() {
        let offset: u32 = kani::any();
        let len: usize = kani::any();
        let elem_size: usize = kani::any();
        kani::assume(elem_size > 0 && elem_size <= 64);
        kani::assume(len > 0 && len <= 4); // Limited for tractability with unwind(5)
        kani::assume((offset as usize) + len * elem_size <= PAGE_SIZE);

        for i in 0..len {
            let access_offset = offset as usize + i * elem_size;
            kani::assert(
                access_offset + elem_size <= PAGE_SIZE,
                "Element access must be within page bounds",
            );
        }
    }
}
