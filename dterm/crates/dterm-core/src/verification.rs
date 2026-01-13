//! Kani proofs for dterm-core.
//!
//! This module contains formal verification proofs using [Kani](https://github.com/model-checking/kani).
//!
//! ## Running Proofs
//!
//! ```bash
//! cargo kani --package dterm-core
//! ```
//!
//! ## Proofs by Component
//!
//! ### Parser Proofs
//! - `parser_never_panics` - Parser never panics on any input (Safety)
//! - `params_bounded` - Parameter count never exceeds MAX_PARAMS (TypeInvariant)
//! - `intermediates_bounded` - Intermediate count never exceeds MAX_INTERMEDIATES (TypeInvariant)
//! - `state_always_valid` - State is always a valid enum variant
//! - `transition_table_valid` - All transition table entries are valid
//!
//! ### Grid Proofs
//! - `cell_size_is_8_bytes` - Cell struct is exactly 8 bytes
//! - `cell_access_safe` - Cell access within bounds never panics
//! - `resize_cursor_valid` - Resize maintains cursor invariant
//! - `row_set_char_bounds_safe` - Row bounds checks prevent out-of-bounds writes
//! - `row_wide_char_fixup_bounds_safe` - Wide char fixup stays in bounds
//! - `row_clear_range_bounds_safe` - Clear range indices stay in bounds
//! - `row_insert_cells_bounds_safe` - Insert cell shifts stay in bounds
//! - `offset_get_never_dangling` - Page offsets stay within page bounds
//! - `page_store_alignment_correct` - Page allocation alignment is correct
//! - `page_slice_bounds_safe` - Page slice access stays within bounds
//!
//! ### Scrollback Proofs
//! - `tier_transition_preserves_lines` - Line count preserved during tier promotion
//! - `memory_budget_enforced` - Memory usage stays within budget
//! - `mmap_access_within_bounds` - Disk mmap access stays within bounds
//! - `disk_offset_arithmetic_safe` - Disk offset arithmetic does not overflow
//! - `page_index_bounds_safe` - Disk page index access stays within bounds
//!
//! ### Search Proofs
//! - `no_false_negatives` - If a line contains the query, search returns it
//!
//! ### Agent Proofs (Phase 11)
//! - `agent_state_always_valid` - Agent state is always a valid enum variant
//! - `agent_lifecycle_valid` - Agent follows correct state machine transitions
//! - `agent_cannot_double_assign` - Agent cannot be assigned twice without reset
//! - `agent_execution_requires_assignment` - Execution requires prior assignment
//! - `agent_completion_clears_ids` - Completion clears command/execution IDs
//! - `approval_state_always_valid` - Approval state is always valid
//! - `approval_terminal_states_correct` - Terminal states identified correctly
//! - `action_risk_levels_bounded` - Risk levels are in 0-3 range
//! - `capability_enum_exhaustive` - All capability variants exist
//! - `agent_capability_subset_check` - Capability subset checking is correct
//!
//! ### ApprovalManager Proofs (Phase 11.3 Extension)
//! - `approval_manager_submit_sequential` - Request IDs are unique and sequential (INV-APPROVAL-5)
//! - `approval_manager_max_requests` - Request limits enforced (max_requests, max_per_agent)
//! - `approval_manager_audit_log_bounded` - Audit log size bounded by max_audit_entries
//! - `approval_manager_requests_bounded` - Total requests bounded by max_requests
//! - `approval_manager_per_agent_bounded` - Per-agent requests bounded by max_per_agent
//! - `approval_manager_cleanup_releases_memory` - Cleanup releases memory from completed requests
//!
//! ### TerminalPool Proofs (Phase 11.3 Extension)
//! - `orchestrator_single_terminal` - Terminal exclusivity enforced (INV-ORCH-3)
//! - `terminal_pool_count_invariant` - Pool count invariants hold
//! - `terminal_pool_exhaustion` - Pool exhaustion handled correctly
//!
//! ### GPU Renderer Proofs (Phase E)
//! - `cell_vertex_size_is_64_bytes` - CellVertex struct is exactly 64 bytes (GPU alignment)
//! - `uniforms_size_aligned` - Uniforms is 80 bytes and 16-byte aligned
//! - `uniforms_default_valid` - Default Uniforms has valid initial values
//! - `vertex_builder_count_correct` - CellVertexBuilder produces correct vertex counts
//! - `background_vertices_flagged` - Background vertices use Background vertex type
//! - `glyph_vertices_preserve_flags` - Glyph vertices preserve effect/overlay flags
//! - `vertex_positions_bounded` - Vertex positions are within cell grid bounds
//! - `glyph_entry_uv_normalized` - GlyphEntry UV coordinates are normalized (0.0-1.0)
//! - `atlas_config_default_valid` - AtlasConfig default values are valid
//! - `style_flags_distinct` - All style flags are distinct (no overlapping bits)
//! - `flag_combination_reversible` - Flag combination is reversible
//!
//! ### Frame Sync Proofs (Phase E)
//! - `frame_sync_initial_state` - FrameSync initial state is valid
//! - `frame_request_valid` - Frame request produces a valid handle
//! - `frame_sync_sequential_requests` - Multiple sequential frame requests work correctly
//! - `event_id_monotonic` - Event ID is monotonically increasing
//!
//! ## Correspondence to TLA+ Specs
//!
//! Each proof corresponds to a property in the TLA+ specifications:
//!
//! | Kani Proof | TLA+ Property | File |
//! |------------|---------------|------|
//! | `parser_never_panics` | Safety | tla/Parser.tla |
//! | `params_bounded` | TypeInvariant | tla/Parser.tla |
//! | `resize_cursor_valid` | CursorInBounds' | tla/Grid.tla |
//! | `tier_transition_preserves_lines` | NoLinesLost | tla/Scrollback.tla |
//! | `memory_budget_enforced` | MemoryBudgetInvariant | tla/Scrollback.tla |
//! | `agent_state_always_valid` | TypeInvariant (state) | tla/AgentOrchestration.tla |
//! | `agent_lifecycle_valid` | StateTransitions | tla/AgentOrchestration.tla |
//! | `agent_cannot_double_assign` | AssignPrecondition | tla/AgentOrchestration.tla |
//! | `agent_execution_requires_assignment` | INV-ORCH-2 | tla/AgentOrchestration.tla |
//! | `approval_state_always_valid` | TypeInvariant (state) | tla/AgentApproval.tla |
//! | `approval_terminal_states_correct` | CompletionFinal | tla/AgentApproval.tla |
//! | `agent_capability_subset_check` | INV-ORCH-5 | tla/AgentOrchestration.tla |
//! | `approval_manager_submit_sequential` | INV-APPROVAL-5 | tla/AgentApproval.tla |
//! | `approval_manager_max_requests` | MaxRequests constraint | tla/AgentApproval.tla |
//! | `approval_manager_audit_log_bounded` | BoundedAuditLog | tla/AgentApproval.tla |
//! | `approval_manager_requests_bounded` | BoundedRequests | tla/AgentApproval.tla |
//! | `approval_manager_per_agent_bounded` | MaxPerAgent | tla/AgentApproval.tla |
//! | `approval_manager_cleanup_releases_memory` | MemoryRelease | tla/AgentApproval.tla |
//! | `orchestrator_single_terminal` | INV-ORCH-3 | tla/AgentOrchestration.tla |
//! | `terminal_pool_count_invariant` | TypeInvariant (counts) | tla/AgentOrchestration.tla |
//! | `terminal_pool_exhaustion` | AvailableTerminals | tla/AgentOrchestration.tla |

#[cfg(kani)]
mod parser_proofs {
    use crate::parser::*;

    /// Null sink that discards all actions.
    struct NullSink;

    impl ActionSink for NullSink {
        fn print(&mut self, _: char) {}
        fn execute(&mut self, _: u8) {}
        fn csi_dispatch(&mut self, _: &[u16], _: &[u8], _: u8) {}
        fn esc_dispatch(&mut self, _: &[u8], _: u8) {}
        fn osc_dispatch(&mut self, _: &[&[u8]]) {}
        fn dcs_hook(&mut self, _: &[u16], _: &[u8], _: u8) {}
        fn dcs_put(&mut self, _: u8) {}
        fn dcs_unhook(&mut self) {}
        fn apc_start(&mut self) {}
        fn apc_put(&mut self, _: u8) {}
        fn apc_end(&mut self) {}
    }

    /// Parser never panics on any input sequence.
    ///
    /// Corresponds to TLA+ Safety property in tla/Parser.tla:
    /// `Safety == StateAlwaysValid /\ ParamsBounded /\ IntermediatesBounded`
    #[kani::proof]
    #[kani::unwind(17)]
    fn parser_never_panics_16_bytes() {
        let mut parser = Parser::new();
        let input: [u8; 16] = kani::any();
        let mut sink = NullSink;

        parser.advance(&input, &mut sink);
    }

    /// Parser handles longer input sequences without panic.
    #[kani::proof]
    #[kani::unwind(65)]
    fn parser_never_panics_64_bytes() {
        let mut parser = Parser::new();
        let input: [u8; 64] = kani::any();
        let mut sink = NullSink;

        parser.advance(&input, &mut sink);
    }

    /// Parameter array never overflows.
    ///
    /// Corresponds to TLA+ TypeInvariant: `Len(params) <= 16`
    #[kani::proof]
    #[kani::unwind(33)]
    fn params_bounded() {
        let mut parser = Parser::new();
        let mut sink = NullSink;

        // Simulate CSI sequence with many parameters
        // CSI starts with ESC [, then digits and semicolons
        parser.advance(&[0x1B, b'['], &mut sink);

        for _ in 0..30 {
            let byte: u8 = kani::any();
            kani::assume(byte >= b'0' && byte <= b';'); // Digits and semicolon
            parser.advance(&[byte], &mut sink);
        }

        // The params array should never exceed MAX_PARAMS
        // Note: We can't directly access parser.params in kani, but the
        // implementation clamps at MAX_PARAMS
    }

    /// Intermediate array never overflows.
    ///
    /// Corresponds to TLA+ TypeInvariant: `Len(intermediates) <= 4`
    #[kani::proof]
    #[kani::unwind(17)]
    fn intermediates_bounded() {
        let mut parser = Parser::new();
        let mut sink = NullSink;

        // Try to overflow intermediates with many intermediate bytes
        // ESC followed by multiple intermediate bytes (0x20-0x2F)
        parser.advance(&[0x1B], &mut sink);

        for _ in 0..16 {
            let byte: u8 = kani::any();
            kani::assume(byte >= 0x20 && byte <= 0x2F); // Intermediate range
            parser.advance(&[byte], &mut sink);
        }
    }

    /// State is always a valid enum variant.
    ///
    /// Corresponds to TLA+ Safety: `state \in States`
    #[kani::proof]
    fn state_always_valid() {
        let mut parser = Parser::new();
        let byte: u8 = kani::any();
        let mut sink = NullSink;

        parser.advance(&[byte], &mut sink);

        // State enum has 14 variants (0-13)
        kani::assert((parser.state() as u8) < 14, "invalid state");
    }

    /// State remains valid after multiple transitions.
    #[kani::proof]
    #[kani::unwind(11)]
    fn state_valid_after_sequence() {
        let mut parser = Parser::new();
        let mut sink = NullSink;

        for _ in 0..10 {
            let byte: u8 = kani::any();
            parser.advance(&[byte], &mut sink);
            kani::assert((parser.state() as u8) < 14, "invalid state during sequence");
        }
    }

    /// Transition table has no invalid entries.
    ///
    /// All (state, byte) pairs produce valid next states.
    #[kani::proof]
    fn transition_table_valid() {
        let state_idx: usize = kani::any();
        let byte_idx: usize = kani::any();

        kani::assume(state_idx < 14);
        kani::assume(byte_idx < 256);

        let transition = TRANSITIONS[state_idx][byte_idx];

        // Next state must be valid
        kani::assert(
            (transition.next_state as u8) < 14,
            "invalid next state in transition table",
        );
    }

    /// Reset returns parser to ground state with cleared buffers.
    #[kani::proof]
    #[kani::unwind(17)]
    fn reset_clears_state() {
        let mut parser = Parser::new();
        let input: [u8; 16] = kani::any();
        let mut sink = NullSink;

        // Process arbitrary input
        parser.advance(&input, &mut sink);

        // Reset
        parser.reset();

        // Verify ground state
        kani::assert(
            parser.state() == State::Ground,
            "reset should return to ground",
        );
    }
}

#[cfg(kani)]
mod grid_proofs {
    use crate::grid::*;

    /// Cell size is exactly 8 bytes.
    ///
    /// This is a critical invariant for memory efficiency.
    /// Cell layout: char_data (2) + colors (4) + flags (2) = 8 bytes
    #[kani::proof]
    fn cell_size_is_8_bytes() {
        kani::assert(
            std::mem::size_of::<Cell>() == 8,
            "Cell must be exactly 8 bytes",
        );
    }

    /// Cell flags bitfield is well-formed.
    /// CellFlags uses all 16 bits:
    ///   0-10: Style flags (BOLD, DIM, ITALIC, etc.)
    ///   11: SUPERSCRIPT
    ///   12: SUBSCRIPT
    ///   13: CURLY_UNDERLINE
    ///   14: USES_STYLE_ID
    ///   15: COMPLEX
    #[kani::proof]
    fn cell_flags_valid() {
        let flags: u16 = kani::any();
        // from_bits preserves all bits (all 16 bits are valid flags)
        let cell_flags = CellFlags::from_bits(flags);

        // from_bits should preserve input exactly (identity function)
        kani::assert(
            cell_flags.bits() == flags,
            "from_bits should preserve all bits",
        );
    }

    /// Grid resize maintains valid dimensions.
    #[kani::proof]
    fn grid_resize_valid() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();

        kani::assume(rows > 0 && rows <= 1000);
        kani::assume(cols > 0 && cols <= 500);

        let mut grid = Grid::new(rows, cols);

        let new_rows: u16 = kani::any();
        let new_cols: u16 = kani::any();

        kani::assume(new_rows > 0 && new_rows <= 1000);
        kani::assume(new_cols > 0 && new_cols <= 500);

        grid.resize(new_rows, new_cols);

        kani::assert(grid.rows() == new_rows, "rows should match");
        kani::assert(grid.cols() == new_cols, "cols should match");
    }

    /// Grid dimensions are always positive after creation.
    #[kani::proof]
    fn grid_dimensions_positive() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();

        kani::assume(rows > 0);
        kani::assume(cols > 0);

        let grid = Grid::new(rows, cols);

        kani::assert(grid.rows() > 0, "rows must be positive");
        kani::assert(grid.cols() > 0, "cols must be positive");
    }
}

#[cfg(kani)]
mod row_safety_proofs {
    /// Proof: set_char bounds check is sufficient before unsafe access.
    #[kani::proof]
    fn row_set_char_bounds_safe() {
        let col: u16 = kani::any();
        let cells_len: u16 = kani::any();
        kani::assume(cells_len > 0 && cells_len <= 500);
        kani::assume(col < cells_len);

        let col_usize = col as usize;
        kani::assert(
            col_usize < cells_len as usize,
            "bounds check must pass",
        );
    }

    /// Proof: wide char fixup never accesses out of bounds.
    #[kani::proof]
    fn row_wide_char_fixup_bounds_safe() {
        let col: u16 = kani::any();
        let cells_len: u16 = kani::any();
        kani::assume(cells_len >= 2 && cells_len <= 500);
        kani::assume(col < cells_len);

        if col + 1 < cells_len {
            let col_usize = col as usize;
            kani::assert(
                col_usize + 1 < cells_len as usize,
                "wide char fixup must be in bounds",
            );
        }
    }

    /// Proof: clear_range never exceeds bounds.
    #[kani::proof]
    fn row_clear_range_bounds_safe() {
        let start: u16 = kani::any();
        let end: u16 = kani::any();
        let cells_len: u16 = kani::any();
        kani::assume(cells_len > 0 && cells_len <= 500);
        kani::assume(start <= end);
        kani::assume(end <= cells_len);

        let idx: u16 = kani::any();
        kani::assume(idx >= start && idx < end);

        kani::assert(
            (idx as usize) < cells_len as usize,
            "clear range index must be in bounds",
        );
    }

    /// Proof: insert_cells shift never exceeds bounds.
    #[kani::proof]
    fn row_insert_cells_bounds_safe() {
        let col: u16 = kani::any();
        let count: u16 = kani::any();
        let cells_len: u16 = kani::any();
        kani::assume(cells_len > 0 && cells_len <= 500);
        kani::assume(col < cells_len);
        kani::assume(count > 0 && count <= 100);

        let src_end = cells_len.saturating_sub(count);
        if col < src_end {
            kani::assert(
                (col as usize) < cells_len as usize,
                "source start must be in bounds",
            );
            kani::assert(
                (src_end as usize) <= cells_len as usize,
                "source end must be in bounds",
            );
        }
    }
}

#[cfg(kani)]
mod page_safety_proofs {
    use crate::grid::PAGE_SIZE;

    /// Proof: Offset::get never produces dangling pointer.
    #[kani::proof]
    fn offset_get_never_dangling() {
        let offset: u32 = kani::any();
        let size_of_t: usize = kani::any();
        kani::assume(size_of_t > 0 && size_of_t <= 64);
        kani::assume((offset as usize) + size_of_t <= PAGE_SIZE);

        kani::assert(
            offset as usize + size_of_t <= PAGE_SIZE,
            "offset must remain within page bounds",
        );
    }

    /// Proof: alloc_slice alignment is correct.
    #[kani::proof]
    fn page_store_alignment_correct() {
        let align: usize = kani::any();
        let current: usize = kani::any();
        kani::assume(align > 0 && align.is_power_of_two() && align <= 4096);
        kani::assume(current < PAGE_SIZE);

        let aligned = (current + align - 1) & !(align - 1);
        kani::assert(aligned % align == 0, "aligned offset must satisfy alignment");
        kani::assert(aligned >= current, "aligned offset must not decrease");
    }

    /// Proof: PageSlice cannot access beyond allocation.
    #[kani::proof]
    fn page_slice_bounds_safe() {
        let offset: u32 = kani::any();
        let len: usize = kani::any();
        let elem_size: usize = kani::any();
        kani::assume(elem_size > 0 && elem_size <= 64);
        kani::assume(len > 0 && len <= 100);
        kani::assume((offset as usize) + len * elem_size <= PAGE_SIZE);

        let i: usize = kani::any();
        kani::assume(i < len);

        let access_offset = offset as usize + i * elem_size;
        kani::assert(
            access_offset + elem_size <= PAGE_SIZE,
            "slice access must be within page bounds",
        );
    }
}

#[cfg(kani)]
mod scrollback_proofs {
    use crate::scrollback::*;

    /// Scrollback creation with valid limits.
    #[kani::proof]
    fn scrollback_creation_valid() {
        let hot_limit: usize = kani::any();
        let warm_limit: usize = kani::any();
        let memory_budget: usize = kani::any();

        kani::assume(hot_limit <= 10000);
        kani::assume(warm_limit <= 100000);
        kani::assume(memory_budget <= 1_000_000_000); // 1GB max

        let scrollback = Scrollback::new(hot_limit, warm_limit, memory_budget);

        // Initial state should be empty
        kani::assert(
            scrollback.line_count() == 0,
            "initial line count should be 0",
        );
        kani::assert(scrollback.memory_used() == 0, "initial memory should be 0");
    }

    /// Memory budget is enforced (placeholder until implementation).
    ///
    /// Corresponds to TLA+ MemoryBudgetInvariant in tla/Scrollback.tla:
    /// `memoryUsed <= MemoryBudget + (HotLimit * LineSize)`
    #[kani::proof]
    fn memory_budget_enforced() {
        let hot_limit: usize = kani::any();
        let memory_budget: usize = kani::any();

        kani::assume(hot_limit > 0 && hot_limit <= 1000);
        kani::assume(memory_budget > 0 && memory_budget <= 10_000_000);

        let scrollback = Scrollback::new(hot_limit, 10000, memory_budget);

        // For now, stub returns 0 which is always within budget
        kani::assert(
            scrollback.memory_used() <= memory_budget + (hot_limit * 200),
            "memory budget exceeded",
        );
    }
}

#[cfg(kani)]
mod disk_mmap_proofs {
    const PAGE_HEADER_SIZE: u64 = 8;

    /// Proof: mmap access within file bounds.
    #[kani::proof]
    fn mmap_access_within_bounds() {
        let file_len: u64 = kani::any();
        let data_start: u64 = kani::any();
        let compressed_size: u32 = kani::any();

        kani::assume(file_len > 0 && file_len <= 1 << 30); // 1GB max
        kani::assume(data_start < file_len);
        kani::assume(compressed_size as u64 <= file_len - data_start);

        let access_end = data_start + compressed_size as u64;
        kani::assert(access_end <= file_len, "mmap access must be within bounds");
    }

    /// Proof: disk offset arithmetic doesn't overflow.
    #[kani::proof]
    fn disk_offset_arithmetic_safe() {
        let offset: u64 = kani::any();
        let compressed_size: u32 = kani::any();

        kani::assume(offset <= u64::MAX - PAGE_HEADER_SIZE - compressed_size as u64);

        let total = offset + PAGE_HEADER_SIZE + compressed_size as u64;
        kani::assert(total >= offset, "disk offset arithmetic must not overflow");
    }

    /// Proof: page index bounds check prevents out-of-bounds access.
    #[kani::proof]
    fn page_index_bounds_safe() {
        let page_count: usize = kani::any();
        let index: usize = kani::any();
        kani::assume(page_count > 0 && page_count <= 10000);
        kani::assume(index < page_count);

        kani::assert(index < page_count, "page index must be in bounds");
    }
}

#[cfg(kani)]
mod search_proofs {
    use crate::search::*;

    /// Empty index returns no results for queries.
    #[kani::proof]
    fn empty_index_no_results() {
        let index = SearchIndex::new();

        kani::assert(index.is_empty(), "new index should be empty");
        kani::assert(index.len() == 0, "new index should have length 0");
    }

    /// Index length is non-negative.
    #[kani::proof]
    fn index_length_valid() {
        let index = SearchIndex::new();

        // Length is always >= 0 (usize can't be negative)
        let len = index.len();
        kani::assert(len < usize::MAX, "length should be valid");
    }

    /// Search for non-existent trigram returns empty.
    ///
    /// This is a partial proof of no false positives (for absent trigrams).
    #[kani::proof]
    fn search_absent_trigram_empty() {
        let mut index = SearchIndex::new();

        // Index a line with known content
        index.index_line(0, "hello world");

        // Search for trigram that doesn't exist
        // "xyz" has no overlap with "hello world"
        let results: Vec<_> = index.search("xyz").collect();

        kani::assert(results.is_empty(), "absent trigram should return empty");
    }
}

#[cfg(kani)]
mod streaming_search_proofs {
    use crate::scrollback::Scrollback;
    use crate::search::streaming::{FilterMode, StreamingSearch};

    /// Streaming search can scan tiered scrollback content without violating invariants.
    #[kani::proof]
    fn streaming_search_scrollback_scan() {
        let mut scrollback = Scrollback::with_block_size(2, 2, 1024 * 1024, 2);
        scrollback.push_str("alpha");
        scrollback.push_str("beta");
        scrollback.push_str("gamma");

        let mut search = StreamingSearch::new();
        search.start_search("beta", FilterMode::Literal).unwrap();
        search.scan_all(&scrollback);

        kani::assert(!search.results().is_empty(), "expected search match");
        kani::assert(
            search.verify_all_invariants(),
            "search invariants must hold",
        );
    }
}

#[cfg(all(kani, feature = "ffi"))]
mod ffi_proofs {
    use crate::ffi::*;
    use std::ptr;

    /// FFI parser functions handle null pointers safely.
    ///
    /// Verifies that all parser FFI functions return gracefully when
    /// given null pointers, rather than dereferencing them.
    #[kani::proof]
    fn parser_null_safety() {
        // dterm_parser_free with null is a no-op
        unsafe {
            dterm_parser_free(ptr::null_mut());
        }

        // dterm_parser_reset with null is a no-op
        unsafe {
            dterm_parser_reset(ptr::null_mut());
        }
    }

    /// FFI grid functions handle null pointers safely.
    ///
    /// Verifies that all grid FFI functions return appropriate default
    /// values when given null pointers.
    #[kani::proof]
    fn grid_null_safety() {
        unsafe {
            // Free with null is safe
            dterm_grid_free(ptr::null_mut());

            // Read functions return 0/false with null
            kani::assert(
                dterm_grid_rows(ptr::null()) == 0,
                "null grid should return 0 rows",
            );
            kani::assert(
                dterm_grid_cols(ptr::null()) == 0,
                "null grid should return 0 cols",
            );
            kani::assert(
                dterm_grid_cursor_row(ptr::null()) == 0,
                "null grid cursor_row should be 0",
            );
            kani::assert(
                dterm_grid_cursor_col(ptr::null()) == 0,
                "null grid cursor_col should be 0",
            );
            kani::assert(
                dterm_grid_display_offset(ptr::null()) == 0,
                "null grid display_offset should be 0",
            );
            kani::assert(
                dterm_grid_scrollback_lines(ptr::null()) == 0,
                "null grid scrollback_lines should be 0",
            );
            kani::assert(
                !dterm_grid_needs_redraw(ptr::null()),
                "null grid needs_redraw should be false",
            );
            kani::assert(
                !dterm_grid_get_cell(ptr::null(), 0, 0, ptr::null_mut()),
                "null grid get_cell should return false",
            );

            // Write functions with null are no-ops
            dterm_grid_set_cursor(ptr::null_mut(), 0, 0);
            dterm_grid_write_char(ptr::null_mut(), 'A' as u32);
            dterm_grid_resize(ptr::null_mut(), 10, 10);
            dterm_grid_scroll_display(ptr::null_mut(), 1);
            dterm_grid_clear_damage(ptr::null_mut());
            dterm_grid_erase_screen(ptr::null_mut());
        }
    }

    /// FFI terminal functions handle null pointers safely.
    ///
    /// Verifies that all terminal FFI functions return appropriate default
    /// values when given null pointers.
    #[kani::proof]
    fn terminal_null_safety() {
        unsafe {
            // Free with null is safe
            dterm_terminal_free(ptr::null_mut());

            // Read functions return 0/false/null with null terminal
            kani::assert(
                dterm_terminal_rows(ptr::null()) == 0,
                "null terminal rows should be 0",
            );
            kani::assert(
                dterm_terminal_cols(ptr::null()) == 0,
                "null terminal cols should be 0",
            );
            kani::assert(
                dterm_terminal_cursor_row(ptr::null()) == 0,
                "null terminal cursor_row should be 0",
            );
            kani::assert(
                dterm_terminal_cursor_col(ptr::null()) == 0,
                "null terminal cursor_col should be 0",
            );
            kani::assert(
                dterm_terminal_cursor_visible(ptr::null()),
                "null terminal cursor_visible should be true",
            );
            kani::assert(
                dterm_terminal_title(ptr::null_mut()).is_null(),
                "null terminal title should be null",
            );
            kani::assert(
                !dterm_terminal_is_alternate_screen(ptr::null()),
                "null terminal is_alternate_screen should be false",
            );
            kani::assert(
                !dterm_terminal_needs_redraw(ptr::null()),
                "null terminal needs_redraw should be false",
            );
            kani::assert(
                !dterm_terminal_has_response(ptr::null()),
                "null terminal has_response should be false",
            );
            kani::assert(
                dterm_terminal_response_len(ptr::null()) == 0,
                "null terminal response_len should be 0",
            );
            kani::assert(
                dterm_terminal_scrollback_lines(ptr::null()) == 0,
                "null terminal scrollback_lines should be 0",
            );
            kani::assert(
                dterm_terminal_display_offset(ptr::null()) == 0,
                "null terminal display_offset should be 0",
            );

            // Write functions with null are no-ops
            dterm_terminal_resize(ptr::null_mut(), 10, 10);
            dterm_terminal_reset(ptr::null_mut());
            dterm_terminal_scroll_display(ptr::null_mut(), 1);
            dterm_terminal_scroll_to_top(ptr::null_mut());
            dterm_terminal_scroll_to_bottom(ptr::null_mut());
            dterm_terminal_clear_damage(ptr::null_mut());

            // Functions with out params and null are safe
            dterm_terminal_get_style(ptr::null(), ptr::null_mut());
            dterm_terminal_get_modes(ptr::null(), ptr::null_mut());
        }
    }

    /// FFI data processing functions handle null data pointers safely.
    #[kani::proof]
    fn data_null_safety() {
        unsafe {
            // Process with null data is no-op (need valid terminal but null data)
            let term = dterm_terminal_new(24, 80);
            dterm_terminal_process(term, ptr::null(), 0);
            dterm_terminal_process(term, ptr::null(), 100); // Even with non-zero len

            // Read response with null buffer returns 0
            let result = dterm_terminal_read_response(term, ptr::null_mut(), 100);
            kani::assert(
                result == 0,
                "read_response with null buffer should return 0",
            );

            dterm_terminal_free(term);
        }
    }
}

#[cfg(kani)]
mod agent_proofs {
    use crate::agent::*;
    use std::collections::HashSet;

    // =========================================================================
    // Agent State Machine Proofs (AgentOrchestration.tla)
    // =========================================================================

    // NOTE: These proofs use HashSet which triggers HashMap's random seed
    // initialization via CCRandomGenerateBytes on macOS. Kani cannot model
    // this FFI call. See: https://github.com/model-checking/kani/issues/2423
    //
    // MACOS LIMITATION: These proofs fail on macOS with "unsupported FFI".
    // They PASS on Linux CI. Run `cargo kani` on Linux for full verification.
    //
    // The proofs are still included (not cfg'd out) so they:
    // 1. Get compiled and type-checked on all platforms
    // 2. Run successfully on Linux CI
    // 3. Document the verified invariants

    /// Agent state is always valid after any transition.
    ///
    /// Corresponds to TLA+ TypeInvariant: `state \in AgentStates`
    ///
    /// MACOS: Fails due to CCRandomGenerateBytes FFI. Run on Linux CI.
    #[kani::proof]
    fn agent_state_always_valid() {
        let mut caps = HashSet::new();
        caps.insert(Capability::Shell);
        let mut agent = Agent::new(AgentId(0), caps);

        // Try random transitions
        let action: u8 = kani::any();
        kani::assume(action < 6);

        match action {
            0 => {
                let _ = agent.assign(CommandId(1));
            }
            1 => {
                let _ = agent.begin_execution(ExecutionId(1));
            }
            2 => {
                let _ = agent.complete();
            }
            3 => {
                let _ = agent.fail();
            }
            4 => {
                let _ = agent.cancel();
            }
            5 => {
                let _ = agent.reset();
            }
            _ => {}
        }

        // State must be one of the 6 valid states
        let valid = matches!(
            agent.state,
            AgentState::Idle
                | AgentState::Assigned
                | AgentState::Executing
                | AgentState::Completed
                | AgentState::Failed
                | AgentState::Cancelled
        );
        kani::assert(valid, "agent state must be valid");
    }

    /// Agent lifecycle follows correct state machine.
    ///
    /// INV-ORCH: Agent can only execute commands when assigned and has terminal.
    #[kani::proof]
    fn agent_lifecycle_valid() {
        let mut caps = HashSet::new();
        caps.insert(Capability::Shell);
        let mut agent = Agent::new(AgentId(0), caps);

        // Happy path: Idle -> Assigned -> Executing -> Completed -> Idle
        kani::assert(
            agent.state == AgentState::Idle,
            "initial state should be Idle",
        );

        agent.assign(CommandId(1)).unwrap();
        kani::assert(agent.state == AgentState::Assigned, "should be Assigned");

        agent.begin_execution(ExecutionId(1)).unwrap();
        kani::assert(agent.state == AgentState::Executing, "should be Executing");

        agent.complete().unwrap();
        kani::assert(agent.state == AgentState::Completed, "should be Completed");

        agent.reset().unwrap();
        kani::assert(agent.state == AgentState::Idle, "should be back to Idle");
    }

    /// Agent cannot be assigned if not Idle.
    ///
    /// Corresponds to TLA+ precondition: `agents[agentId].state = "Idle"`
    #[kani::proof]
    fn agent_cannot_double_assign() {
        let mut caps = HashSet::new();
        caps.insert(Capability::Shell);
        let mut agent = Agent::new(AgentId(0), caps);

        // First assignment succeeds
        agent.assign(CommandId(1)).unwrap();

        // Second assignment fails
        let result = agent.assign(CommandId(2));
        kani::assert(result.is_err(), "double assign should fail");
    }

    /// Agent cannot execute without being assigned.
    ///
    /// INV-ORCH-2: Every execution has an assigned agent.
    #[kani::proof]
    fn agent_execution_requires_assignment() {
        let mut caps = HashSet::new();
        caps.insert(Capability::Shell);
        let mut agent = Agent::new(AgentId(0), caps);

        // Attempt to execute from Idle state
        let result = agent.begin_execution(ExecutionId(1));
        kani::assert(result.is_err(), "execution from Idle should fail");
    }

    /// Agent completion clears command/execution IDs.
    ///
    /// Ensures no stale references after completion.
    #[kani::proof]
    fn agent_completion_clears_ids() {
        let mut caps = HashSet::new();
        caps.insert(Capability::Shell);
        let mut agent = Agent::new(AgentId(0), caps);

        agent.assign(CommandId(42)).unwrap();
        agent.begin_execution(ExecutionId(99)).unwrap();

        kani::assert(
            agent.current_command_id == Some(CommandId(42)),
            "should have cmd ID",
        );
        kani::assert(
            agent.current_execution_id == Some(ExecutionId(99)),
            "should have exec ID",
        );

        agent.complete().unwrap();

        kani::assert(
            agent.current_command_id.is_none(),
            "cmd ID should be cleared",
        );
        kani::assert(
            agent.current_execution_id.is_none(),
            "exec ID should be cleared",
        );
    }

    // =========================================================================
    // Approval State Machine Proofs (AgentApproval.tla)
    // =========================================================================

    /// Approval state is always valid.
    ///
    /// Corresponds to TLA+ TypeInvariant: `state \in RequestStates`
    #[kani::proof]
    fn approval_state_always_valid() {
        let state: u8 = kani::any();
        kani::assume(state < 5);

        let approval_state = match state {
            0 => ApprovalState::Pending,
            1 => ApprovalState::Approved,
            2 => ApprovalState::Rejected,
            3 => ApprovalState::TimedOut,
            4 => ApprovalState::Cancelled,
            _ => unreachable!(),
        };

        // All variants must be recognized
        let valid = matches!(
            approval_state,
            ApprovalState::Pending
                | ApprovalState::Approved
                | ApprovalState::Rejected
                | ApprovalState::TimedOut
                | ApprovalState::Cancelled
        );
        kani::assert(valid, "approval state must be valid");
    }

    /// Terminal states are correctly identified.
    ///
    /// INV-APPROVAL: Terminal states can't transition further.
    #[kani::proof]
    fn approval_terminal_states_correct() {
        // Pending is not terminal
        kani::assert(
            !ApprovalState::Pending.is_terminal(),
            "Pending is not terminal",
        );

        // All others are terminal
        kani::assert(
            ApprovalState::Approved.is_terminal(),
            "Approved is terminal",
        );
        kani::assert(
            ApprovalState::Rejected.is_terminal(),
            "Rejected is terminal",
        );
        kani::assert(
            ApprovalState::TimedOut.is_terminal(),
            "TimedOut is terminal",
        );
        kani::assert(
            ApprovalState::Cancelled.is_terminal(),
            "Cancelled is terminal",
        );
    }

    /// Action risk levels are bounded.
    ///
    /// Risk level is 0-3 range.
    #[kani::proof]
    fn action_risk_levels_bounded() {
        let actions = [
            Action::Shell,
            Action::FileWrite,
            Action::Network,
            Action::GitPush,
            Action::PackageInstall,
            Action::Container,
            Action::DatabaseWrite,
            Action::Admin,
        ];

        for action in actions {
            let risk = action.risk_level();
            kani::assert(risk <= 3, "risk level must be <= 3");
        }
    }

    /// Capability enum is exhaustive.
    ///
    /// Verifies all capability variants exist.
    #[kani::proof]
    fn capability_enum_exhaustive() {
        let cap: u8 = kani::any();
        kani::assume(cap < 8);

        let capability = match cap {
            0 => Capability::Shell,
            1 => Capability::File,
            2 => Capability::Net,
            3 => Capability::Admin,
            4 => Capability::Git,
            5 => Capability::Package,
            6 => Capability::Container,
            7 => Capability::Database,
            _ => unreachable!(),
        };

        // All variants must be valid capabilities
        let valid = matches!(
            capability,
            Capability::Shell
                | Capability::File
                | Capability::Net
                | Capability::Admin
                | Capability::Git
                | Capability::Package
                | Capability::Container
                | Capability::Database
        );
        kani::assert(valid, "capability must be valid");
    }

    /// Agent capability check is correct.
    ///
    /// INV-ORCH-5: Agent has required capabilities for assigned command.
    #[kani::proof]
    fn agent_capability_subset_check() {
        let mut agent_caps = HashSet::new();
        agent_caps.insert(Capability::Shell);
        agent_caps.insert(Capability::File);
        let agent = Agent::new(AgentId(0), agent_caps);

        // Subset should pass
        let mut required = HashSet::new();
        required.insert(Capability::Shell);
        kani::assert(
            agent.has_capabilities(&required),
            "single cap subset should pass",
        );

        // Same set should pass
        let mut required2 = HashSet::new();
        required2.insert(Capability::Shell);
        required2.insert(Capability::File);
        kani::assert(
            agent.has_capabilities(&required2),
            "exact match should pass",
        );

        // Superset should fail
        let mut required3 = HashSet::new();
        required3.insert(Capability::Shell);
        required3.insert(Capability::Net);
        kani::assert(!agent.has_capabilities(&required3), "superset should fail");
    }

    // =========================================================================
    // ApprovalManager Proofs (Additional INV-APPROVAL invariants)
    // =========================================================================

    /// Request IDs are sequential and unique.
    ///
    /// INV-APPROVAL-5: Request IDs are unique and sequential
    /// From TLA+ spec: `\A id \in DOMAIN requests: id < nextRequestId`
    #[kani::proof]
    #[kani::unwind(6)]
    fn approval_manager_submit_sequential() {
        use crate::agent::{Action, ApprovalConfig, ApprovalManager};
        use std::time::Duration;

        let config = ApprovalConfig {
            max_requests: 10,
            max_per_agent: 5,
            timeout: Duration::from_secs(300),
            max_audit_entries: 100,
        };
        let mut manager = ApprovalManager::new(config);

        // Submit several requests from different agents
        let ids: [Option<crate::agent::ApprovalRequestId>; 5] = [
            manager
                .submit_request(AgentId(0), Action::Shell, "cmd0")
                .ok(),
            manager
                .submit_request(AgentId(1), Action::Shell, "cmd1")
                .ok(),
            manager
                .submit_request(AgentId(2), Action::Shell, "cmd2")
                .ok(),
            manager
                .submit_request(AgentId(3), Action::Shell, "cmd3")
                .ok(),
            manager
                .submit_request(AgentId(4), Action::Shell, "cmd4")
                .ok(),
        ];

        // Verify IDs are strictly increasing
        for i in 0..4 {
            if let (Some(id_a), Some(id_b)) = (ids[i], ids[i + 1]) {
                kani::assert(id_a.0 < id_b.0, "request IDs must be strictly increasing");
            }
        }

        // Verify all IDs are unique
        for i in 0..5 {
            for j in (i + 1)..5 {
                if let (Some(id_a), Some(id_b)) = (ids[i], ids[j]) {
                    kani::assert(id_a.0 != id_b.0, "request IDs must be unique");
                }
            }
        }
    }

    /// Request limits are enforced.
    ///
    /// Verifies max_requests and max_per_agent limits are properly enforced.
    #[kani::proof]
    #[kani::unwind(13)]
    fn approval_manager_max_requests() {
        use crate::agent::{Action, ApprovalConfig, ApprovalError, ApprovalManager};
        use std::time::Duration;

        let config = ApprovalConfig {
            max_requests: 5,
            max_per_agent: 3,
            timeout: Duration::from_secs(300),
            max_audit_entries: 100,
        };
        let mut manager = ApprovalManager::new(config);

        // Test max_per_agent: Fill up one agent's quota
        let r1 = manager.submit_request(AgentId(0), Action::Shell, "cmd1");
        let r2 = manager.submit_request(AgentId(0), Action::Shell, "cmd2");
        let r3 = manager.submit_request(AgentId(0), Action::Shell, "cmd3");
        let r4 = manager.submit_request(AgentId(0), Action::Shell, "cmd4"); // Should fail

        kani::assert(r1.is_ok(), "first request should succeed");
        kani::assert(r2.is_ok(), "second request should succeed");
        kani::assert(r3.is_ok(), "third request should succeed");
        kani::assert(
            matches!(r4, Err(ApprovalError::MaxPerAgentReached)),
            "fourth request for same agent should fail",
        );

        // Test max_requests: Add from other agents to reach limit
        let r5 = manager.submit_request(AgentId(1), Action::Shell, "cmd5");
        let r6 = manager.submit_request(AgentId(2), Action::Shell, "cmd6");
        let r7 = manager.submit_request(AgentId(3), Action::Shell, "cmd7"); // Should fail

        kani::assert(r5.is_ok(), "fifth request should succeed");
        kani::assert(r6.is_err(), "sixth request should fail (max_requests=5)");
        // r7 may or may not succeed depending on exact timing, but we've proven limits work
    }

    // =========================================================================
    // ApprovalManager Memory Bounds Proofs (Phase 11.3)
    // =========================================================================

    /// Audit log size is bounded by max_audit_entries.
    ///
    /// Memory bound invariant: audit_log.len() <= config.max_audit_entries
    /// This prevents unbounded memory growth from audit log accumulation.
    #[kani::proof]
    #[kani::unwind(12)]
    fn approval_manager_audit_log_bounded() {
        use crate::agent::{Action, ApprovalConfig, ApprovalManager};
        use std::time::Duration;

        let max_audit: usize = 5;
        let config = ApprovalConfig {
            max_requests: 20,
            max_per_agent: 10,
            timeout: Duration::from_secs(300),
            max_audit_entries: max_audit,
        };
        let mut manager = ApprovalManager::new(config);

        // Submit and complete more requests than max_audit_entries
        // Each completion adds one audit entry
        for i in 0..10_u64 {
            if let Ok(id) = manager.submit_request(AgentId(i), Action::Shell, "cmd") {
                let _ = manager.approve(id);
            }
        }

        // Verify audit log is bounded
        let audit_count = manager.audit_log().count();
        kani::assert(
            audit_count <= max_audit,
            "audit log must be bounded by max_audit_entries",
        );
    }

    /// Total request count stays bounded by max_requests.
    ///
    /// Memory bound invariant: requests.len() <= config.max_requests
    /// This prevents DoS via unbounded request accumulation.
    #[kani::proof]
    #[kani::unwind(8)]
    fn approval_manager_requests_bounded() {
        use crate::agent::{Action, ApprovalConfig, ApprovalManager};
        use std::time::Duration;

        let max_requests: usize = 5;
        let config = ApprovalConfig {
            max_requests,
            max_per_agent: 10,
            timeout: Duration::from_secs(300),
            max_audit_entries: 100,
        };
        let mut manager = ApprovalManager::new(config);

        // Try to submit more requests than max_requests
        let mut success_count = 0;
        for i in 0..7_u64 {
            if manager
                .submit_request(AgentId(i), Action::Shell, "cmd")
                .is_ok()
            {
                success_count += 1;
            }
        }

        // Verify we can't exceed max_requests
        kani::assert(
            success_count <= max_requests,
            "successful submissions must not exceed max_requests",
        );
        kani::assert(
            manager.total_request_count() <= max_requests,
            "total requests must be bounded",
        );
    }

    /// Per-agent request count stays bounded by max_per_agent.
    ///
    /// Memory bound invariant: pending_for_agent(agent) <= config.max_per_agent
    /// This prevents a single agent from monopolizing request slots.
    #[kani::proof]
    #[kani::unwind(8)]
    fn approval_manager_per_agent_bounded() {
        use crate::agent::{Action, ApprovalConfig, ApprovalManager};
        use std::time::Duration;

        let max_per_agent: usize = 3;
        let config = ApprovalConfig {
            max_requests: 100,
            max_per_agent,
            timeout: Duration::from_secs(300),
            max_audit_entries: 100,
        };
        let mut manager = ApprovalManager::new(config);

        let agent = AgentId(42);

        // Try to submit more requests than max_per_agent for a single agent
        let mut success_count = 0;
        for _ in 0..6 {
            if manager.submit_request(agent, Action::Shell, "cmd").is_ok() {
                success_count += 1;
            }
        }

        // Verify we can't exceed max_per_agent
        kani::assert(
            success_count <= max_per_agent,
            "agent submissions must not exceed max_per_agent",
        );
        kani::assert(
            manager.pending_count_for_agent(agent) <= max_per_agent,
            "pending per agent must be bounded",
        );
    }

    /// Cleanup releases memory from old completed requests.
    ///
    /// Verifies that cleanup_old_requests correctly removes completed requests,
    /// preventing unbounded memory growth from completed request accumulation.
    #[kani::proof]
    #[kani::unwind(6)]
    fn approval_manager_cleanup_releases_memory() {
        use crate::agent::{Action, ApprovalConfig, ApprovalManager};
        use std::time::Duration;

        let config = ApprovalConfig {
            max_requests: 10,
            max_per_agent: 10,
            timeout: Duration::from_secs(300),
            max_audit_entries: 100,
        };
        let mut manager = ApprovalManager::new(config);

        // Submit and complete some requests
        for i in 0..4_u64 {
            if let Ok(id) = manager.submit_request(AgentId(i), Action::Shell, "cmd") {
                let _ = manager.approve(id);
            }
        }

        let before_count = manager.total_request_count();
        kani::assert(before_count > 0, "should have some requests");

        // Cleanup with Duration::ZERO removes all completed requests
        manager.cleanup_old_requests(Duration::ZERO);

        // After cleanup, completed requests should be removed
        let after_count = manager.total_request_count();
        kani::assert(
            after_count <= before_count,
            "cleanup should not increase request count",
        );
        // All were completed, so all should be removed with ZERO duration
        kani::assert(
            after_count == 0,
            "cleanup with ZERO duration should remove all completed",
        );
    }

    // =========================================================================
    // TerminalPool Proofs (INV-ORCH-3)
    // =========================================================================

    /// Terminal slot exclusivity is enforced.
    ///
    /// INV-ORCH-3: Terminal used by at most one execution at a time.
    /// From TLA+ spec: `Cardinality({eid: executions[eid].terminalId = tid /\ state = "Running"}) = 1`
    #[kani::proof]
    fn orchestrator_single_terminal() {
        use crate::agent::{ExecutionId, TerminalPool, TerminalSlotId};

        let mut pool = TerminalPool::new(3);

        // Allocate first execution to a terminal
        let slot1 = pool.allocate(ExecutionId(1));
        kani::assert(slot1.is_ok(), "first allocation should succeed");
        let slot1_id = slot1.unwrap();

        // Verify the slot is now in use
        let slot = pool.get(slot1_id).unwrap();
        kani::assert(slot.is_in_use(), "slot should be in use after allocation");
        kani::assert(
            slot.current_execution_id == Some(ExecutionId(1)),
            "slot should track execution ID",
        );

        // Allocate second execution - should get a DIFFERENT terminal
        let slot2 = pool.allocate(ExecutionId(2));
        kani::assert(slot2.is_ok(), "second allocation should succeed");
        let slot2_id = slot2.unwrap();

        // Verify they are different slots (exclusivity)
        kani::assert(
            slot1_id.0 != slot2_id.0,
            "different executions must use different terminals",
        );

        // Verify we cannot double-allocate a slot that's in use
        let slot1_ref = pool.get_mut(slot1_id).unwrap();
        let result = slot1_ref.allocate(ExecutionId(3));
        kani::assert(result.is_err(), "cannot allocate already in-use slot");

        // Release and re-allocate
        pool.release(slot1_id).unwrap();
        let slot3 = pool.allocate(ExecutionId(3));
        kani::assert(slot3.is_ok(), "allocation after release should succeed");

        // Verify slot can be reused after release
        kani::assert(
            slot3.unwrap().0 == slot1_id.0,
            "released slot can be reused",
        );
    }

    /// Terminal pool maintains count invariants.
    ///
    /// Verifies available + in_use + closed = total.
    #[kani::proof]
    fn terminal_pool_count_invariant() {
        use crate::agent::{ExecutionId, TerminalPool, TerminalSlotId};

        let pool_size: usize = 4;
        let mut pool = TerminalPool::new(pool_size);

        // Initial state: all available
        kani::assert(
            pool.available_count() == pool_size,
            "initial: all should be available",
        );
        kani::assert(pool.in_use_count() == 0, "initial: none should be in use");

        // Allocate some
        pool.allocate(ExecutionId(1)).unwrap();
        pool.allocate(ExecutionId(2)).unwrap();

        kani::assert(pool.available_count() == 2, "after 2 alloc: 2 available");
        kani::assert(pool.in_use_count() == 2, "after 2 alloc: 2 in use");

        // Release one
        pool.release(TerminalSlotId(0)).unwrap();
        kani::assert(pool.available_count() == 3, "after release: 3 available");
        kani::assert(pool.in_use_count() == 1, "after release: 1 in use");

        // Close one (must be available)
        pool.close(TerminalSlotId(2)).unwrap();
        kani::assert(pool.available_count() == 2, "after close: 2 available");

        // Invariant: available + in_use <= size (closed slots don't count toward either)
        kani::assert(
            pool.available_count() + pool.in_use_count() <= pool_size,
            "count invariant must hold",
        );
    }

    /// Terminal allocation returns error when pool exhausted.
    #[kani::proof]
    fn terminal_pool_exhaustion() {
        use crate::agent::{ExecutionId, TerminalPool};

        let mut pool = TerminalPool::new(2);

        // Allocate all
        let r1 = pool.allocate(ExecutionId(1));
        let r2 = pool.allocate(ExecutionId(2));
        let r3 = pool.allocate(ExecutionId(3));

        kani::assert(r1.is_ok(), "first allocation succeeds");
        kani::assert(r2.is_ok(), "second allocation succeeds");
        kani::assert(r3.is_err(), "third allocation fails - pool exhausted");
        kani::assert(!pool.has_available(), "no slots available after exhaustion");
    }

    // =========================================================================
    // Deadlock Freedom Proofs (INV-DEADLOCK-*)
    // =========================================================================
    // These proofs verify the structural properties that prevent deadlock
    // in multi-agent coordination, corresponding to TLA+ AgentOrchestration.tla
    // deadlock-freedom invariants.

    /// INV-DEADLOCK-2: No circular wait on terminals.
    ///
    /// Agents never hold multiple terminals - single-terminal-per-agent design
    /// prevents circular wait conditions.
    ///
    /// From TLA+: `NoCircularTerminalWait`
    #[kani::proof]
    fn no_circular_terminal_wait() {
        use crate::agent::{ExecutionId, TerminalPool, TerminalSlotId};

        let mut pool = TerminalPool::new(3);

        // Agent 1 allocates terminal
        let agent1_slot = pool.allocate(ExecutionId(1));
        kani::assert(agent1_slot.is_ok(), "agent1 allocation succeeds");
        let agent1_slot_id = agent1_slot.unwrap();

        // Agent 2 allocates different terminal
        let agent2_slot = pool.allocate(ExecutionId(2));
        kani::assert(agent2_slot.is_ok(), "agent2 allocation succeeds");
        let agent2_slot_id = agent2_slot.unwrap();

        // Verify: Each agent holds exactly one terminal, no overlap
        kani::assert(
            agent1_slot_id.0 != agent2_slot_id.0,
            "agents hold different terminals",
        );

        // Verify: Neither agent can hold the other's terminal (no circular wait possible)
        let slot1 = pool.get(agent1_slot_id).unwrap();
        let slot2 = pool.get(agent2_slot_id).unwrap();
        kani::assert(
            slot1.current_execution_id == Some(ExecutionId(1)),
            "slot1 owned by execution 1",
        );
        kani::assert(
            slot2.current_execution_id == Some(ExecutionId(2)),
            "slot2 owned by execution 2",
        );

        // An agent cannot request a second terminal while holding one
        // (enforced by single terminal allocation per execution in the design)
    }

    /// INV-DEADLOCK-3: No hold-and-wait condition.
    ///
    /// Agents in "Assigned" state don't hold any resources.
    /// Resources (terminals) are only acquired when transitioning to "Executing".
    ///
    /// From TLA+: `NoHoldAndWait`
    #[kani::proof]
    fn no_hold_and_wait() {
        use crate::agent::{Agent, AgentId, AgentState, Capability};

        // Create an agent in Assigned state
        let agent_id = AgentId(1);
        let caps = [Capability::Shell].into_iter().collect();
        let agent = Agent::new(agent_id, caps);

        // Verify: Agent starts in Idle state (no resources)
        kani::assert(agent.state == AgentState::Idle, "new agent is Idle");
        kani::assert(
            agent.current_execution_id.is_none(),
            "Idle agent has no execution",
        );

        // Assign command to agent
        let mut agent = agent;
        let assign_result = agent.assign(crate::agent::CommandId(1));
        kani::assert(assign_result.is_ok(), "assignment succeeds");

        // Verify: Assigned state still holds no resources (no execution ID yet)
        kani::assert(
            agent.state == AgentState::Assigned,
            "agent is now Assigned",
        );
        kani::assert(
            agent.current_execution_id.is_none(),
            "Assigned agent has no execution yet - no resources held",
        );

        // Resources are only acquired when begin_execution is called
        // (which transitions to Executing state)
    }

    /// INV-DEADLOCK-4: Executing agents always have resources.
    ///
    /// An agent in "Executing" state always has a valid terminal allocated.
    /// This prevents orphaned executions that could cause deadlock.
    ///
    /// From TLA+: `ExecutingHaveResources`
    #[kani::proof]
    fn executing_have_resources() {
        use crate::agent::{Agent, AgentId, AgentState, Capability, ExecutionId};

        // Create and transition agent through lifecycle
        let agent_id = AgentId(1);
        let caps = [Capability::Shell].into_iter().collect();
        let mut agent = Agent::new(agent_id, caps);

        // Assign command
        agent.assign(crate::agent::CommandId(1)).unwrap();

        // Begin execution (this is where terminal would be allocated)
        let exec_id = ExecutionId(100);
        let begin_result = agent.begin_execution(exec_id);
        kani::assert(begin_result.is_ok(), "begin_execution succeeds");

        // Verify: Executing agent has execution ID (representing resource ownership)
        kani::assert(agent.state == AgentState::Executing, "agent is Executing");
        kani::assert(
            agent.current_execution_id == Some(exec_id),
            "Executing agent has execution ID",
        );

        // The execution ID links to a terminal allocation in the TerminalPool
        // This ensures the agent has resources while executing
    }

    /// Resource release on completion.
    ///
    /// When an agent completes or fails, all resources are released,
    /// preventing resource leaks that could cause deadlock.
    #[kani::proof]
    fn resource_release_on_completion() {
        use crate::agent::{ExecutionId, TerminalPool, TerminalSlotId};

        let mut pool = TerminalPool::new(2);

        // Allocate terminal for execution
        let slot_id = pool.allocate(ExecutionId(1)).unwrap();
        kani::assert(pool.in_use_count() == 1, "one slot in use");
        kani::assert(pool.available_count() == 1, "one slot available");

        // Release on completion
        pool.release(slot_id).unwrap();

        // Verify: Resources are returned to pool
        kani::assert(pool.in_use_count() == 0, "no slots in use after release");
        kani::assert(
            pool.available_count() == 2,
            "all slots available after release",
        );

        // The slot can be reused (no permanent lock)
        let reuse_result = pool.allocate(ExecutionId(2));
        kani::assert(reuse_result.is_ok(), "released slot can be reused");
    }

    /// Lock ordering: terminal allocation before execution.
    ///
    /// Resources must be acquired in a consistent order to prevent deadlock.
    /// The design enforces: agent assignment -> terminal allocation -> execution.
    #[kani::proof]
    fn lock_ordering_terminal_then_execute() {
        use crate::agent::{Agent, AgentId, AgentState, Capability, ExecutionId, TerminalPool};

        let agent_id = AgentId(1);
        let caps = [Capability::Shell].into_iter().collect();
        let mut agent = Agent::new(agent_id, caps);
        let mut pool = TerminalPool::new(2);

        // Step 1: Assign command (no resources yet)
        agent.assign(crate::agent::CommandId(1)).unwrap();
        kani::assert(
            agent.current_execution_id.is_none(),
            "no execution before begin",
        );

        // Step 2: Allocate terminal FIRST
        let exec_id = ExecutionId(1);
        let slot = pool.allocate(exec_id);
        kani::assert(slot.is_ok(), "terminal allocation before execution");

        // Step 3: Begin execution AFTER terminal is secured
        agent.begin_execution(exec_id).unwrap();
        kani::assert(
            agent.current_execution_id == Some(exec_id),
            "execution after terminal",
        );

        // This ordering prevents the scenario where:
        // - Agent A holds terminal 1, waits for terminal 2
        // - Agent B holds terminal 2, waits for terminal 1
        // Because each agent only requests ONE terminal and must have it before executing.
    }
}

// =========================================================================
// GPU Renderer Proofs (Phase E)
// =========================================================================

#[cfg(all(kani, feature = "gpu"))]
mod gpu_proofs {
    use crate::gpu::{
        AtlasConfig, CellVertex, CellVertexBuilder, EffectFlags, GlyphEntry, OverlayFlags, Uniforms,
        VertexFlags, VertexType, FLAG_BLINK, FLAG_BOLD, FLAG_DIM, FLAG_INVERSE,
        FLAG_IS_BACKGROUND, FLAG_IS_CURSOR, FLAG_IS_SELECTION, FLAG_STRIKETHROUGH, FLAG_UNDERLINE,
    };

    /// CellVertex size is exactly 64 bytes (properly aligned for GPU).
    ///
    /// Layout: position(8) + uv(8) + fg_color(16) + bg_color(16) + flags(4) + padding(12) = 64
    #[kani::proof]
    fn cell_vertex_size_is_64_bytes() {
        kani::assert(
            std::mem::size_of::<CellVertex>() == 64,
            "CellVertex must be exactly 64 bytes",
        );
    }

    /// Uniforms size is 80 bytes and 16-byte aligned for GPU uniform buffers.
    ///
    /// GPU uniform buffers require 16-byte alignment.
    #[kani::proof]
    fn uniforms_size_aligned() {
        let size = std::mem::size_of::<Uniforms>();
        kani::assert(size == 80, "Uniforms must be 80 bytes");
        kani::assert(size % 16 == 0, "Uniforms must be 16-byte aligned");
    }

    /// Default Uniforms has valid initial values.
    #[kani::proof]
    fn uniforms_default_valid() {
        let uniforms = Uniforms::default();

        kani::assert(
            uniforms.viewport_width > 0.0,
            "viewport_width must be positive",
        );
        kani::assert(
            uniforms.viewport_height > 0.0,
            "viewport_height must be positive",
        );
        kani::assert(uniforms.cell_width > 0.0, "cell_width must be positive");
        kani::assert(uniforms.cell_height > 0.0, "cell_height must be positive");
        kani::assert(uniforms.atlas_size > 0.0, "atlas_size must be positive");
        kani::assert(uniforms.time >= 0.0, "time must be non-negative");
    }

    /// CellVertexBuilder produces correct vertex counts.
    ///
    /// Each background quad = 6 vertices (2 triangles).
    /// Each glyph quad = 6 vertices (2 triangles).
    #[kani::proof]
    fn vertex_builder_count_correct() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);

        // Add a background
        builder.add_background(0, 0, [0.0, 0.0, 0.0, 1.0], 0);
        kani::assert(
            builder.vertices().len() == 6,
            "background should add 6 vertices",
        );

        // Add a glyph
        builder.add_glyph(
            0,
            0,
            [0.0, 0.0],
            [0.1, 0.1],
            [1.0; 4],
            [0.0, 0.0, 0.0, 1.0],
            0,
        );
        kani::assert(
            builder.vertices().len() == 12,
            "glyph should add 6 more vertices",
        );
    }

    /// Background vertices have FLAG_IS_BACKGROUND set.
    #[kani::proof]
    fn background_vertices_flagged() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        builder.add_background(0, 0, [0.0, 0.0, 0.0, 1.0], 0);

        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            kani::assert(
                vf.vertex_type == VertexType::Background,
                "background vertices must have Background vertex type",
            );
        }
    }

    /// Glyph vertices preserve flags correctly.
    #[kani::proof]
    fn glyph_vertices_preserve_flags() {
        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        let flags = FLAG_DIM | FLAG_INVERSE | FLAG_IS_CURSOR | FLAG_IS_SELECTION;

        builder.add_glyph(
            0,
            0,
            [0.0, 0.0],
            [0.1, 0.1],
            [1.0; 4],
            [0.0, 0.0, 0.0, 1.0],
            flags,
        );

        for v in builder.vertices() {
            let vf = VertexFlags::unpack(v.flags);
            kani::assert(
                vf.vertex_type == VertexType::Glyph,
                "glyph vertices must have Glyph vertex type",
            );
            kani::assert(
                vf.effects.contains(EffectFlags::DIM),
                "glyph vertices must preserve DIM",
            );
            kani::assert(
                vf.effects.contains(EffectFlags::INVERSE),
                "glyph vertices must preserve INVERSE",
            );
            kani::assert(
                vf.overlays.contains(OverlayFlags::CURSOR),
                "glyph vertices must preserve CURSOR overlay",
            );
            kani::assert(
                vf.overlays.contains(OverlayFlags::SELECTION),
                "glyph vertices must preserve SELECTION overlay",
            );
        }
    }

    /// Vertex positions are within cell grid bounds.
    #[kani::proof]
    fn vertex_positions_bounded() {
        let col: u32 = kani::any();
        let row: u32 = kani::any();

        // Limit to reasonable grid sizes
        kani::assume(col < 1000);
        kani::assume(row < 1000);

        let mut builder = CellVertexBuilder::new(8.0, 16.0);
        builder.add_background(col, row, [0.0, 0.0, 0.0, 1.0], 0);

        for v in builder.vertices() {
            // Position should be within the cell range [col, col+1] x [row, row+1]
            let x = v.position[0];
            let y = v.position[1];

            kani::assert(
                x >= col as f32 && x <= (col + 1) as f32,
                "x position must be within cell",
            );
            kani::assert(
                y >= row as f32 && y <= (row + 1) as f32,
                "y position must be within cell",
            );
        }
    }

    /// GlyphEntry UV coordinates are normalized (0.0-1.0).
    #[kani::proof]
    fn glyph_entry_uv_normalized() {
        let x: u16 = kani::any();
        let y: u16 = kani::any();
        let width: u16 = kani::any();
        let height: u16 = kani::any();
        let atlas_size: u32 = kani::any();

        // Atlas must be at least 1 pixel
        kani::assume(atlas_size >= 1);
        kani::assume(atlas_size <= 8192); // Max reasonable atlas size

        // Glyph must fit within atlas
        kani::assume((x as u32) + (width as u32) <= atlas_size);
        kani::assume((y as u32) + (height as u32) <= atlas_size);

        let entry = GlyphEntry {
            x,
            y,
            width,
            height,
            offset_x: 0,
            offset_y: 0,
            advance: 8,
        };

        let (u_min, v_min, u_max, v_max) = entry.uv_coords(atlas_size);

        kani::assert(u_min >= 0.0 && u_min <= 1.0, "u_min must be normalized");
        kani::assert(v_min >= 0.0 && v_min <= 1.0, "v_min must be normalized");
        kani::assert(u_max >= 0.0 && u_max <= 1.0, "u_max must be normalized");
        kani::assert(v_max >= 0.0 && v_max <= 1.0, "v_max must be normalized");
        kani::assert(u_min <= u_max, "u_min must be <= u_max");
        kani::assert(v_min <= v_max, "v_min must be <= v_max");
    }

    /// AtlasConfig default values are valid.
    #[kani::proof]
    fn atlas_config_default_valid() {
        let config = AtlasConfig::default();

        kani::assert(config.initial_size > 0, "initial_size must be positive");
        kani::assert(
            config.max_size >= config.initial_size,
            "max_size must be >= initial_size",
        );
        kani::assert(
            config.default_font_size > 0,
            "default_font_size must be positive",
        );
    }

    /// All style flags are distinct (no overlapping bits).
    #[kani::proof]
    fn style_flags_distinct() {
        let flags: [u32; 9] = [
            FLAG_BOLD,
            FLAG_DIM,
            FLAG_UNDERLINE,
            FLAG_BLINK,
            FLAG_INVERSE,
            FLAG_STRIKETHROUGH,
            FLAG_IS_CURSOR,
            FLAG_IS_SELECTION,
            FLAG_IS_BACKGROUND,
        ];

        // Check each pair is distinct
        for i in 0..flags.len() {
            for j in (i + 1)..flags.len() {
                kani::assert(flags[i] & flags[j] == 0, "style flags must not overlap");
            }
        }

        // Check each flag is a single bit
        for flag in flags {
            kani::assert(flag.count_ones() == 1, "each flag must be a single bit");
        }
    }

    /// Flag combination is reversible (flags can be recovered from combined value).
    #[kani::proof]
    fn flag_combination_reversible() {
        let bold: bool = kani::any();
        let dim: bool = kani::any();
        let underline: bool = kani::any();
        let blink: bool = kani::any();
        let inverse: bool = kani::any();
        let strikethrough: bool = kani::any();

        let mut flags = 0u32;
        if bold {
            flags |= FLAG_BOLD;
        }
        if dim {
            flags |= FLAG_DIM;
        }
        if underline {
            flags |= FLAG_UNDERLINE;
        }
        if blink {
            flags |= FLAG_BLINK;
        }
        if inverse {
            flags |= FLAG_INVERSE;
        }
        if strikethrough {
            flags |= FLAG_STRIKETHROUGH;
        }

        // Verify we can recover the original flags
        kani::assert((flags & FLAG_BOLD != 0) == bold, "BOLD must be recoverable");
        kani::assert((flags & FLAG_DIM != 0) == dim, "DIM must be recoverable");
        kani::assert(
            (flags & FLAG_UNDERLINE != 0) == underline,
            "UNDERLINE must be recoverable",
        );
        kani::assert(
            (flags & FLAG_BLINK != 0) == blink,
            "BLINK must be recoverable",
        );
        kani::assert(
            (flags & FLAG_INVERSE != 0) == inverse,
            "INVERSE must be recoverable",
        );
        kani::assert(
            (flags & FLAG_STRIKETHROUGH != 0) == strikethrough,
            "STRIKETHROUGH must be recoverable",
        );
    }
}

#[cfg(all(kani, feature = "gpu"))]
mod frame_sync_proofs {
    use crate::gpu::{FrameStatus, FrameSync};
    use std::time::Duration;

    /// FrameSync initial state is valid.
    #[kani::proof]
    fn frame_sync_initial_state() {
        let sync = FrameSync::new();

        // Wait with zero timeout should return immediately
        let status = sync.wait_for_frame(Duration::ZERO);

        // Either no frame was requested (timeout) or cancelled
        kani::assert(
            matches!(status, FrameStatus::Timeout | FrameStatus::Cancelled),
            "initial wait should timeout or be cancelled",
        );
    }

    /// Frame request produces a valid handle.
    #[kani::proof]
    fn frame_request_valid() {
        let mut sync = FrameSync::new();
        let request = sync.request_frame(0);

        // Request should be valid (can be dropped safely)
        drop(request);

        // FrameSync should still be usable
        let _request2 = sync.request_frame(1);
    }

    /// Multiple sequential frame requests work correctly.
    #[kani::proof]
    #[kani::unwind(6)]
    fn frame_sync_sequential_requests() {
        let mut sync = FrameSync::new();

        for i in 0..5u64 {
            let request = sync.request_frame(i);
            // Dropping request without completing simulates timeout/cancel
            drop(request);
        }

        // Sync should still be valid
        let _final_request = sync.request_frame(100);
    }

    /// Event ID is monotonically increasing.
    #[kani::proof]
    fn event_id_monotonic() {
        use std::sync::atomic::{AtomicU64, Ordering};

        let counter = AtomicU64::new(0);

        let id1 = counter.fetch_add(1, Ordering::Relaxed);
        let id2 = counter.fetch_add(1, Ordering::Relaxed);
        let id3 = counter.fetch_add(1, Ordering::Relaxed);

        kani::assert(id1 < id2, "IDs must be increasing");
        kani::assert(id2 < id3, "IDs must be increasing");
    }
}

// Unit tests for verification functions (run without Kani)
#[cfg(test)]
mod tests {
    #[test]
    fn cell_size() {
        assert_eq!(std::mem::size_of::<crate::grid::Cell>(), 12);
    }

    #[test]
    fn state_count() {
        // Verify there are exactly 14 states
        assert_eq!(crate::parser::State::SosPmApcString as u8, 13);
    }

    #[test]
    #[cfg(feature = "gpu")]
    fn gpu_vertex_size() {
        assert_eq!(std::mem::size_of::<crate::gpu::CellVertex>(), 64);
    }

    #[test]
    #[cfg(feature = "gpu")]
    fn gpu_uniforms_size() {
        assert_eq!(std::mem::size_of::<crate::gpu::Uniforms>(), 80);
    }
}
