//! Cell-related types for Alacritty compatibility.
//!
//! This module re-exports cell types from dterm-core's grid module
//! to match Alacritty's `term::cell` module structure.

// Re-export the Cell type
pub use dterm_core::grid::Cell;

// Re-export CellFlags as Flags (Alacritty naming)
pub use dterm_core::grid::CellFlags as Flags;

// Re-export hyperlink types
pub use dterm_core::grid::CellExtra;

/// Hyperlink data stored in a cell.
///
/// This is a compatibility type that wraps dterm-core's hyperlink data.
/// In dterm-core, hyperlinks are stored in CellExtras with an ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hyperlink {
    /// Unique identifier for the hyperlink.
    pub id: String,
    /// URI of the hyperlink.
    pub uri: String,
}

impl Hyperlink {
    /// Create a new hyperlink.
    pub fn new<T: ToString>(id: Option<T>, uri: String) -> Self {
        let id = id.map(|i| i.to_string()).unwrap_or_else(|| {
            // Generate a unique ID if none provided
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            format!("{}_dterm", COUNTER.fetch_add(1, Ordering::Relaxed))
        });
        Self { id, uri }
    }

    /// Get the hyperlink ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the hyperlink URI.
    pub fn uri(&self) -> &str {
        &self.uri
    }
}

/// Trait for calculating the occupied length of a line.
///
/// This matches Alacritty's `LineLength` trait.
pub trait LineLength {
    /// Calculate the occupied line length.
    ///
    /// Returns the column index of the last non-empty cell plus one,
    /// or the full width if the line is wrapped.
    fn line_length(&self) -> crate::index::Column;
}

impl LineLength for dterm_core::grid::Row {
    fn line_length(&self) -> crate::index::Column {
        use dterm_core::grid::RowFlags;

        let cols = self.cols() as usize;

        // If line is wrapped, return full width
        if self.flags().contains(RowFlags::WRAPPED) {
            return crate::index::Column(cols);
        }

        // Return the row's stored length (last non-empty cell + 1)
        // Row already tracks this internally
        crate::index::Column(self.len() as usize)
    }
}
