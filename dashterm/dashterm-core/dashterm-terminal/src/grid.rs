//! Terminal grid - the 2D array of cells
//!
//! Manages the terminal buffer with scrollback support.

use crate::cell::Cell;
use serde::{Deserialize, Serialize};

/// Terminal grid with scrollback buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grid {
    /// Current visible lines
    lines: Vec<Vec<Cell>>,
    /// Scrollback buffer
    scrollback: Vec<Vec<Cell>>,
    /// Number of columns
    cols: usize,
    /// Number of visible rows
    rows: usize,
    /// Maximum scrollback lines
    max_scrollback: usize,
    /// Current scroll offset (0 = at bottom)
    scroll_offset: usize,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let lines = (0..rows)
            .map(|_| vec![Cell::default(); cols])
            .collect();

        Self {
            lines,
            scrollback: Vec::new(),
            cols,
            rows,
            max_scrollback: 10_000,
            scroll_offset: 0,
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn get(&self, row: usize, col: usize) -> Option<&Cell> {
        self.lines.get(row).and_then(|line| line.get(col))
    }

    pub fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        self.lines.get_mut(row).and_then(|line| line.get_mut(col))
    }

    pub fn set(&mut self, row: usize, col: usize, cell: Cell) {
        if let Some(line) = self.lines.get_mut(row) {
            if col < line.len() {
                line[col] = cell;
            }
        }
    }

    /// Scroll the grid up by n lines, moving old lines to scrollback
    pub fn scroll_up(&mut self, n: usize) {
        for _ in 0..n {
            if !self.lines.is_empty() {
                let line = self.lines.remove(0);
                if self.scrollback.len() >= self.max_scrollback {
                    self.scrollback.remove(0);
                }
                self.scrollback.push(line);
                self.lines.push(vec![Cell::default(); self.cols]);
            }
        }
    }

    /// Resize the grid
    pub fn resize(&mut self, cols: usize, rows: usize) {
        // Resize existing lines
        for line in &mut self.lines {
            line.resize(cols, Cell::default());
        }

        // Add or remove rows
        while self.lines.len() < rows {
            self.lines.push(vec![Cell::default(); cols]);
        }
        while self.lines.len() > rows {
            let line = self.lines.remove(0);
            if self.scrollback.len() < self.max_scrollback {
                self.scrollback.push(line);
            }
        }

        self.cols = cols;
        self.rows = rows;
    }

    /// Clear the entire grid
    pub fn clear(&mut self) {
        for line in &mut self.lines {
            for cell in line {
                *cell = Cell::default();
            }
        }
    }

    /// Clear from cursor to end of line
    pub fn clear_line_from(&mut self, row: usize, col: usize) {
        if let Some(line) = self.lines.get_mut(row) {
            for cell in line.iter_mut().skip(col) {
                *cell = Cell::default();
            }
        }
    }

    /// Get visible lines for rendering
    pub fn visible_lines(&self) -> &[Vec<Cell>] {
        &self.lines
    }

    /// Get scrollback length
    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_new() {
        let grid = Grid::new(80, 24);
        assert_eq!(grid.cols(), 80);
        assert_eq!(grid.rows(), 24);
        assert_eq!(grid.scrollback_len(), 0);
    }

    #[test]
    fn test_grid_dimensions() {
        let grid = Grid::new(120, 40);
        assert_eq!(grid.cols(), 120);
        assert_eq!(grid.rows(), 40);
    }

    #[test]
    fn test_grid_get_valid_cell() {
        let grid = Grid::new(80, 24);
        let cell = grid.get(0, 0);
        assert!(cell.is_some());
        assert!(cell.unwrap().is_empty());
    }

    #[test]
    fn test_grid_get_out_of_bounds() {
        let grid = Grid::new(80, 24);
        assert!(grid.get(100, 0).is_none());
        assert!(grid.get(0, 100).is_none());
        assert!(grid.get(24, 0).is_none()); // Row 24 is out of bounds (0-23)
    }

    #[test]
    fn test_grid_set_cell() {
        let mut grid = Grid::new(80, 24);
        let cell = Cell::new('A');
        grid.set(5, 10, cell.clone());

        let retrieved = grid.get(5, 10).unwrap();
        assert_eq!(retrieved.content, "A");
    }

    #[test]
    fn test_grid_get_mut() {
        let mut grid = Grid::new(80, 24);
        if let Some(cell) = grid.get_mut(0, 0) {
            cell.content = "X".to_string();
        }
        assert_eq!(grid.get(0, 0).unwrap().content, "X");
    }

    #[test]
    fn test_grid_scroll_up() {
        let mut grid = Grid::new(80, 24);

        // Set a cell in the first row
        grid.set(0, 0, Cell::new('A'));
        assert_eq!(grid.scrollback_len(), 0);

        // Scroll up
        grid.scroll_up(1);

        // First row should now be empty (new row)
        assert!(grid.get(23, 0).unwrap().is_empty());

        // Old first row should be in scrollback
        assert_eq!(grid.scrollback_len(), 1);
    }

    #[test]
    fn test_grid_scroll_up_multiple() {
        let mut grid = Grid::new(80, 24);

        // Scroll up 5 lines
        grid.scroll_up(5);

        // Scrollback should have 5 lines
        assert_eq!(grid.scrollback_len(), 5);
    }

    #[test]
    fn test_grid_resize_larger() {
        let mut grid = Grid::new(80, 24);
        grid.resize(120, 40);

        assert_eq!(grid.cols(), 120);
        assert_eq!(grid.rows(), 40);
    }

    #[test]
    fn test_grid_resize_smaller() {
        let mut grid = Grid::new(80, 24);
        grid.resize(40, 12);

        assert_eq!(grid.cols(), 40);
        assert_eq!(grid.rows(), 12);
    }

    #[test]
    fn test_grid_clear() {
        let mut grid = Grid::new(80, 24);

        // Set some cells
        grid.set(0, 0, Cell::new('A'));
        grid.set(10, 10, Cell::new('B'));

        // Clear
        grid.clear();

        // All cells should be empty
        assert!(grid.get(0, 0).unwrap().is_empty());
        assert!(grid.get(10, 10).unwrap().is_empty());
    }

    #[test]
    fn test_grid_clear_line_from() {
        let mut grid = Grid::new(80, 24);

        // Fill first row with data
        for col in 0..80 {
            grid.set(0, col, Cell::new('X'));
        }

        // Clear from column 40 to end
        grid.clear_line_from(0, 40);

        // Columns 0-39 should still have data
        assert_eq!(grid.get(0, 0).unwrap().content, "X");
        assert_eq!(grid.get(0, 39).unwrap().content, "X");

        // Columns 40+ should be empty
        assert!(grid.get(0, 40).unwrap().is_empty());
        assert!(grid.get(0, 79).unwrap().is_empty());
    }

    #[test]
    fn test_grid_visible_lines() {
        let grid = Grid::new(80, 24);
        let lines = grid.visible_lines();
        assert_eq!(lines.len(), 24);
        assert_eq!(lines[0].len(), 80);
    }

    #[test]
    fn test_grid_scrollback_limit() {
        let mut grid = Grid::new(80, 24);

        // Scroll more than max_scrollback (default 10000)
        // For testing, let's scroll 100 times
        for _ in 0..100 {
            grid.scroll_up(1);
        }

        assert_eq!(grid.scrollback_len(), 100);
    }
}
