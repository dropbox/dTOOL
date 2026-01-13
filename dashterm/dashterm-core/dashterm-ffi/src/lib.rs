//! C FFI bindings for Swift integration
//!
//! Provides a C-compatible API for the DashTerm core functionality.

use dashterm_graph::{ComputationGraph, Node, Edge, EdgeType, NodeType, NodeGroup};
use dashterm_terminal::Terminal;
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use std::sync::{Arc, Mutex};

// =============================================================================
// Terminal FFI
// =============================================================================

/// Opaque terminal handle
pub struct DashTermTerminal {
    inner: Arc<Mutex<Terminal>>,
}

/// Create a new terminal instance
#[no_mangle]
pub extern "C" fn dashterm_terminal_new(cols: u32, rows: u32) -> *mut DashTermTerminal {
    let terminal = Terminal::new(cols as usize, rows as usize);
    let handle = Box::new(DashTermTerminal {
        inner: Arc::new(Mutex::new(terminal)),
    });
    Box::into_raw(handle)
}

/// Free a terminal instance
#[no_mangle]
pub extern "C" fn dashterm_terminal_free(ptr: *mut DashTermTerminal) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

/// Process input bytes from PTY
#[no_mangle]
pub extern "C" fn dashterm_terminal_process(
    ptr: *mut DashTermTerminal,
    data: *const u8,
    len: usize,
) {
    if ptr.is_null() || data.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        let bytes = std::slice::from_raw_parts(data, len);
        if let Ok(mut terminal) = handle.inner.lock() {
            terminal.process(bytes);
        }
    }
}

/// Resize the terminal
#[no_mangle]
pub extern "C" fn dashterm_terminal_resize(ptr: *mut DashTermTerminal, cols: u32, rows: u32) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            terminal.resize(cols as usize, rows as usize);
        }
    }
}

/// Get terminal size
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_size(ptr: *const DashTermTerminal) -> DashTermSize {
    if ptr.is_null() {
        return DashTermSize { cols: 80, rows: 24 };
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(terminal) = handle.inner.lock() {
            let size = terminal.size();
            DashTermSize {
                cols: size.cols as u32,
                rows: size.rows as u32,
            }
        } else {
            DashTermSize { cols: 80, rows: 24 }
        }
    }
}

/// Get cursor position
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_cursor(ptr: *const DashTermTerminal) -> DashTermCursor {
    if ptr.is_null() {
        return DashTermCursor { row: 0, col: 0, visible: true };
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(terminal) = handle.inner.lock() {
            let (row, col) = terminal.cursor();
            DashTermCursor {
                row: row as u32,
                col: col as u32,
                visible: terminal.cursor_visible(),
            }
        } else {
            DashTermCursor { row: 0, col: 0, visible: true }
        }
    }
}

/// Get the terminal grid as JSON
/// Caller must free the returned string with dashterm_string_free
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_grid_json(ptr: *const DashTermTerminal) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(terminal) = handle.inner.lock() {
            let cells = terminal.get_cells();
            if let Ok(json) = serde_json::to_string(&cells) {
                if let Ok(cstring) = CString::new(json) {
                    return cstring.into_raw();
                }
            }
        }
        ptr::null_mut()
    }
}

/// Get the window title
/// Caller must free the returned string with dashterm_string_free
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_title(ptr: *const DashTermTerminal) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(terminal) = handle.inner.lock() {
            if let Ok(cstring) = CString::new(terminal.title()) {
                return cstring.into_raw();
            }
        }
        ptr::null_mut()
    }
}

/// Get pending terminal events as JSON
/// Returns events like Bell, TitleChanged, Exit, Redraw
/// Caller must free the returned string with dashterm_string_free
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_events_json(ptr: *mut DashTermTerminal) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            let events = terminal.take_events();
            if let Ok(json) = serde_json::to_string(&events) {
                if let Ok(cstring) = CString::new(json) {
                    return cstring.into_raw();
                }
            }
        }
        ptr::null_mut()
    }
}

/// Get damaged lines for efficient partial updates as JSON
/// Returns array of [line, left_col, right_col] tuples
/// Caller must free the returned string with dashterm_string_free
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_damage_json(ptr: *mut DashTermTerminal) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            let damage = terminal.damage();
            if let Ok(json) = serde_json::to_string(&damage) {
                if let Ok(cstring) = CString::new(json) {
                    return cstring.into_raw();
                }
            }
        }
        ptr::null_mut()
    }
}

/// Reset damage tracking after rendering
#[no_mangle]
pub extern "C" fn dashterm_terminal_reset_damage(ptr: *mut DashTermTerminal) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            terminal.reset_damage();
        }
    }
}

// =============================================================================
// Agent Parsing FFI
// =============================================================================

/// Enable agent output parsing for the terminal
/// Agent events will be collected and can be retrieved with dashterm_terminal_get_agent_events_json
#[no_mangle]
pub extern "C" fn dashterm_terminal_enable_agent_parsing(ptr: *mut DashTermTerminal) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            terminal.enable_agent_parsing();
        }
    }
}

/// Disable agent output parsing
#[no_mangle]
pub extern "C" fn dashterm_terminal_disable_agent_parsing(ptr: *mut DashTermTerminal) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            terminal.disable_agent_parsing();
        }
    }
}

/// Check if agent parsing is enabled
#[no_mangle]
pub extern "C" fn dashterm_terminal_is_agent_parsing_enabled(ptr: *const DashTermTerminal) -> bool {
    if ptr.is_null() {
        return false;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(terminal) = handle.inner.lock() {
            terminal.is_agent_parsing_enabled()
        } else {
            false
        }
    }
}

/// Get pending agent events as JSON
/// Returns JSON array of AgentEvent objects
/// Caller must free the returned string with dashterm_string_free
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_agent_events_json(ptr: *mut DashTermTerminal) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            let events = terminal.take_agent_events();
            if let Ok(json) = serde_json::to_string(&events) {
                if let Ok(cstring) = CString::new(json) {
                    return cstring.into_raw();
                }
            }
        }
        ptr::null_mut()
    }
}

/// Get the currently active agent node ID (if any)
/// Caller must free the returned string with dashterm_string_free
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_active_agent_node(ptr: *const DashTermTerminal) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(terminal) = handle.inner.lock() {
            if let Some(node) = terminal.active_agent_node() {
                if let Ok(cstring) = CString::new(node) {
                    return cstring.into_raw();
                }
            }
        }
        ptr::null_mut()
    }
}

/// Get the currently active agent tool name (if any)
/// Caller must free the returned string with dashterm_string_free
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_active_agent_tool(ptr: *const DashTermTerminal) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(terminal) = handle.inner.lock() {
            if let Some(tool) = terminal.active_agent_tool() {
                if let Ok(cstring) = CString::new(tool) {
                    return cstring.into_raw();
                }
            }
        }
        ptr::null_mut()
    }
}

/// Clear agent parser state
#[no_mangle]
pub extern "C" fn dashterm_terminal_clear_agent_state(ptr: *mut DashTermTerminal) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            terminal.clear_agent_state();
        }
    }
}

/// Get the current scroll display offset (0 = bottom, positive = scrolled up)
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_display_offset(ptr: *const DashTermTerminal) -> u32 {
    if ptr.is_null() {
        return 0;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(terminal) = handle.inner.lock() {
            terminal.display_offset() as u32
        } else {
            0
        }
    }
}

/// Get the total number of history lines available for scrolling
#[no_mangle]
pub extern "C" fn dashterm_terminal_get_history_size(ptr: *const DashTermTerminal) -> u32 {
    if ptr.is_null() {
        return 0;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(terminal) = handle.inner.lock() {
            terminal.history_size() as u32
        } else {
            0
        }
    }
}

/// Scroll the display up by the given number of lines
#[no_mangle]
pub extern "C" fn dashterm_terminal_scroll_up(ptr: *mut DashTermTerminal, lines: u32) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            terminal.scroll_up(lines as usize);
        }
    }
}

/// Scroll the display down by the given number of lines
#[no_mangle]
pub extern "C" fn dashterm_terminal_scroll_down(ptr: *mut DashTermTerminal, lines: u32) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            terminal.scroll_down(lines as usize);
        }
    }
}

/// Scroll to the top of history
#[no_mangle]
pub extern "C" fn dashterm_terminal_scroll_to_top(ptr: *mut DashTermTerminal) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            terminal.scroll_to_top();
        }
    }
}

/// Scroll to the bottom (most recent output)
#[no_mangle]
pub extern "C" fn dashterm_terminal_scroll_to_bottom(ptr: *mut DashTermTerminal) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut terminal) = handle.inner.lock() {
            terminal.scroll_to_bottom();
        }
    }
}

// =============================================================================
// Graph FFI
// =============================================================================

/// Opaque graph handle
pub struct DashTermGraph {
    inner: Arc<Mutex<ComputationGraph>>,
}

/// Create a new computation graph
#[no_mangle]
pub extern "C" fn dashterm_graph_new(name: *const c_char) -> *mut DashTermGraph {
    let name_str = if name.is_null() {
        "Unnamed Graph".to_string()
    } else {
        unsafe { CStr::from_ptr(name).to_string_lossy().into_owned() }
    };

    let graph = ComputationGraph::new(name_str);
    let handle = Box::new(DashTermGraph {
        inner: Arc::new(Mutex::new(graph)),
    });
    Box::into_raw(handle)
}

/// Free a graph instance
#[no_mangle]
pub extern "C" fn dashterm_graph_free(ptr: *mut DashTermGraph) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

/// Add a node to the graph
/// Returns the node ID or null on failure
#[no_mangle]
pub extern "C" fn dashterm_graph_add_node(
    ptr: *mut DashTermGraph,
    id: *const c_char,
    label: *const c_char,
    node_type: u32,
) -> *mut c_char {
    if ptr.is_null() || id.is_null() || label.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        let handle = &*ptr;
        let id_str = CStr::from_ptr(id).to_string_lossy().into_owned();
        let label_str = CStr::from_ptr(label).to_string_lossy().into_owned();
        let node_type = match node_type {
            0 => NodeType::Start,
            1 => NodeType::End,
            2 => NodeType::Model,
            3 => NodeType::Tool,
            4 => NodeType::Condition,
            5 => NodeType::Parallel,
            6 => NodeType::Join,
            7 => NodeType::Human,
            _ => NodeType::Custom,
        };

        if let Ok(mut graph) = handle.inner.lock() {
            let node = Node::new(id_str.clone(), label_str, node_type);
            graph.add_node(node);
            if let Ok(cstring) = CString::new(id_str) {
                return cstring.into_raw();
            }
        }
        ptr::null_mut()
    }
}

/// Add an edge to the graph
/// Returns true on success
#[no_mangle]
pub extern "C" fn dashterm_graph_add_edge(
    ptr: *mut DashTermGraph,
    from_id: *const c_char,
    to_id: *const c_char,
    edge_type: u32,
) -> bool {
    if ptr.is_null() || from_id.is_null() || to_id.is_null() {
        return false;
    }

    unsafe {
        let handle = &*ptr;
        let from_str = CStr::from_ptr(from_id).to_string_lossy().into_owned();
        let to_str = CStr::from_ptr(to_id).to_string_lossy().into_owned();
        let edge_type = match edge_type {
            0 => EdgeType::Normal,
            1 => EdgeType::ConditionalTrue,
            2 => EdgeType::ConditionalFalse,
            3 => EdgeType::Error,
            4 => EdgeType::Fork,
            5 => EdgeType::Join,
            6 => EdgeType::Loop,
            _ => EdgeType::Normal,
        };

        if let Ok(mut graph) = handle.inner.lock() {
            let edge = Edge::new(edge_type);
            return graph.add_edge(&from_str, &to_str, edge).is_ok();
        }
        false
    }
}

/// Get the graph layout data as JSON
/// Caller must free the returned string with dashterm_string_free
#[no_mangle]
pub extern "C" fn dashterm_graph_get_layout_json(ptr: *const DashTermGraph) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(graph) = handle.inner.lock() {
            let layout = graph.get_layout_data();
            if let Ok(json) = serde_json::to_string(&layout) {
                if let Ok(cstring) = CString::new(json) {
                    return cstring.into_raw();
                }
            }
        }
        ptr::null_mut()
    }
}

/// Compute auto-layout for all nodes in the graph
/// Uses a hierarchical/layered algorithm based on topological depth
#[no_mangle]
pub extern "C" fn dashterm_graph_compute_layout(ptr: *mut DashTermGraph) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(mut graph) = handle.inner.lock() {
            graph.compute_layout();
        }
    }
}

/// Compute zoom and pan values to fit all nodes in a viewport of the given size.
/// Returns a DashTermZoomFit struct with zoom, pan_x, and pan_y values.
#[no_mangle]
pub extern "C" fn dashterm_graph_compute_zoom_to_fit(
    ptr: *const DashTermGraph,
    viewport_width: f32,
    viewport_height: f32,
) -> DashTermZoomFit {
    if ptr.is_null() {
        return DashTermZoomFit { zoom: 1.0, pan_x: 0.0, pan_y: 0.0 };
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(graph) = handle.inner.lock() {
            let (zoom, pan_x, pan_y) = graph.compute_zoom_to_fit(viewport_width, viewport_height);
            DashTermZoomFit { zoom, pan_x, pan_y }
        } else {
            DashTermZoomFit { zoom: 1.0, pan_x: 0.0, pan_y: 0.0 }
        }
    }
}

/// Get the bounding box of all nodes in the graph as JSON
/// Returns JSON with min_x, min_y, max_x, max_y
/// Caller must free the returned string with dashterm_string_free
#[no_mangle]
pub extern "C" fn dashterm_graph_get_bounding_box_json(ptr: *const DashTermGraph) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let handle = &*ptr;
        if let Ok(graph) = handle.inner.lock() {
            let bbox = graph.compute_bounding_box();
            if let Ok(json) = serde_json::to_string(&bbox) {
                if let Ok(cstring) = CString::new(json) {
                    return cstring.into_raw();
                }
            }
        }
        ptr::null_mut()
    }
}

// =============================================================================
// Graph Group FFI
// =============================================================================

/// Create a node group in the graph
/// Returns the group ID or null on failure
#[no_mangle]
pub extern "C" fn dashterm_graph_create_group(
    ptr: *mut DashTermGraph,
    group_id: *const c_char,
    label: *const c_char,
) -> *mut c_char {
    if ptr.is_null() || group_id.is_null() || label.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        let handle = &*ptr;
        let group_id_str = CStr::from_ptr(group_id).to_string_lossy().into_owned();
        let label_str = CStr::from_ptr(label).to_string_lossy().into_owned();

        if let Ok(mut graph) = handle.inner.lock() {
            let group = NodeGroup::new(group_id_str.clone(), label_str);
            graph.create_group(group);
            if let Ok(cstring) = CString::new(group_id_str) {
                return cstring.into_raw();
            }
        }
        ptr::null_mut()
    }
}

/// Add a node to a group
/// Returns true on success
#[no_mangle]
pub extern "C" fn dashterm_graph_add_node_to_group(
    ptr: *mut DashTermGraph,
    node_id: *const c_char,
    group_id: *const c_char,
) -> bool {
    if ptr.is_null() || node_id.is_null() || group_id.is_null() {
        return false;
    }

    unsafe {
        let handle = &*ptr;
        let node_id_str = CStr::from_ptr(node_id).to_string_lossy().into_owned();
        let group_id_str = CStr::from_ptr(group_id).to_string_lossy().into_owned();

        if let Ok(mut graph) = handle.inner.lock() {
            return graph.add_node_to_group(&node_id_str, &group_id_str).is_ok();
        }
        false
    }
}

/// Toggle a group's collapsed state
/// Returns the new collapsed state (true = collapsed, false = expanded)
#[no_mangle]
pub extern "C" fn dashterm_graph_toggle_group(
    ptr: *mut DashTermGraph,
    group_id: *const c_char,
) -> bool {
    if ptr.is_null() || group_id.is_null() {
        return false;
    }

    unsafe {
        let handle = &*ptr;
        let group_id_str = CStr::from_ptr(group_id).to_string_lossy().into_owned();

        if let Ok(mut graph) = handle.inner.lock() {
            return graph.toggle_group(&group_id_str);
        }
        false
    }
}

/// Automatically group consecutive tool nodes under model nodes
/// Returns the number of groups created
#[no_mangle]
pub extern "C" fn dashterm_graph_auto_group_tools(
    ptr: *mut DashTermGraph,
    min_tools: u32,
) -> u32 {
    if ptr.is_null() {
        return 0;
    }

    unsafe {
        let handle = &*ptr;
        if let Ok(mut graph) = handle.inner.lock() {
            return graph.auto_group_tool_sequences(min_tools as usize) as u32;
        }
        0
    }
}

/// Update a group's status based on its child nodes
#[no_mangle]
pub extern "C" fn dashterm_graph_update_group_status(
    ptr: *mut DashTermGraph,
    group_id: *const c_char,
) {
    if ptr.is_null() || group_id.is_null() {
        return;
    }

    unsafe {
        let handle = &*ptr;
        let group_id_str = CStr::from_ptr(group_id).to_string_lossy().into_owned();

        if let Ok(mut graph) = handle.inner.lock() {
            graph.update_group_status(&group_id_str);
        }
    }
}

// =============================================================================
// Common Types and Utilities
// =============================================================================

/// Terminal size
#[repr(C)]
pub struct DashTermSize {
    pub cols: u32,
    pub rows: u32,
}

/// Cursor position and visibility
#[repr(C)]
pub struct DashTermCursor {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
}

/// Zoom-to-fit result with zoom level and pan offsets
#[repr(C)]
pub struct DashTermZoomFit {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
}

/// Free a string allocated by the library
#[no_mangle]
pub extern "C" fn dashterm_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { drop(CString::from_raw(ptr)) };
    }
}

/// Get the library version
#[no_mangle]
pub extern "C" fn dashterm_version() -> *const c_char {
    static VERSION: &[u8] = b"0.1.0\0";
    VERSION.as_ptr() as *const c_char
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn test_terminal_create_and_free() {
        let terminal = dashterm_terminal_new(80, 24);
        assert!(!terminal.is_null());
        dashterm_terminal_free(terminal);
    }

    #[test]
    fn test_terminal_get_size() {
        let terminal = dashterm_terminal_new(120, 40);
        assert!(!terminal.is_null());

        let size = dashterm_terminal_get_size(terminal);
        assert_eq!(size.cols, 120);
        assert_eq!(size.rows, 40);

        dashterm_terminal_free(terminal);
    }

    #[test]
    fn test_terminal_resize() {
        let terminal = dashterm_terminal_new(80, 24);
        assert!(!terminal.is_null());

        dashterm_terminal_resize(terminal, 100, 50);
        let size = dashterm_terminal_get_size(terminal);
        assert_eq!(size.cols, 100);
        assert_eq!(size.rows, 50);

        dashterm_terminal_free(terminal);
    }

    #[test]
    fn test_terminal_process_and_get_grid() {
        let terminal = dashterm_terminal_new(80, 24);
        assert!(!terminal.is_null());

        // Process some text
        let text = b"Hello, World!";
        dashterm_terminal_process(terminal, text.as_ptr(), text.len());

        // Get grid JSON
        let json_ptr = dashterm_terminal_get_grid_json(terminal);
        assert!(!json_ptr.is_null());

        let json_str = unsafe { CStr::from_ptr(json_ptr) }.to_str().unwrap();
        // Each cell contains a single character, so check for 'H' (the first char)
        assert!(json_str.contains("\"content\":\"H\""), "Grid should contain 'H' character");

        dashterm_string_free(json_ptr);
        dashterm_terminal_free(terminal);
    }

    #[test]
    fn test_terminal_cursor() {
        let terminal = dashterm_terminal_new(80, 24);
        assert!(!terminal.is_null());

        let cursor = dashterm_terminal_get_cursor(terminal);
        // Initial cursor should be at origin
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);

        // Process some text to move cursor
        let text = b"ABC";
        dashterm_terminal_process(terminal, text.as_ptr(), text.len());

        let cursor = dashterm_terminal_get_cursor(terminal);
        assert_eq!(cursor.col, 3); // Cursor moved right by 3

        dashterm_terminal_free(terminal);
    }

    #[test]
    fn test_terminal_events() {
        let terminal = dashterm_terminal_new(80, 24);
        assert!(!terminal.is_null());

        // Process some text to generate events
        let text = b"test";
        dashterm_terminal_process(terminal, text.as_ptr(), text.len());

        // Get events JSON
        let events_ptr = dashterm_terminal_get_events_json(terminal);
        assert!(!events_ptr.is_null());

        let events_str = unsafe { CStr::from_ptr(events_ptr) }.to_str().unwrap();
        // Should be valid JSON array
        assert!(events_str.starts_with('['));
        assert!(events_str.ends_with(']'));

        dashterm_string_free(events_ptr);
        dashterm_terminal_free(terminal);
    }

    #[test]
    fn test_terminal_damage() {
        let terminal = dashterm_terminal_new(80, 24);
        assert!(!terminal.is_null());

        // Process text to create damage
        let text = b"damaged text";
        dashterm_terminal_process(terminal, text.as_ptr(), text.len());

        // Get damage JSON
        let damage_ptr = dashterm_terminal_get_damage_json(terminal);
        assert!(!damage_ptr.is_null());

        let damage_str = unsafe { CStr::from_ptr(damage_ptr) }.to_str().unwrap();
        // Should be valid JSON array
        assert!(damage_str.starts_with('['));

        dashterm_string_free(damage_ptr);

        // Reset damage
        dashterm_terminal_reset_damage(terminal);

        dashterm_terminal_free(terminal);
    }

    #[test]
    fn test_version() {
        let version = dashterm_version();
        assert!(!version.is_null());

        let version_str = unsafe { CStr::from_ptr(version) }.to_str().unwrap();
        assert_eq!(version_str, "0.1.0");
    }

    #[test]
    fn test_graph_create_and_free() {
        let name = CString::new("Test Graph").unwrap();
        let graph = dashterm_graph_new(name.as_ptr());
        assert!(!graph.is_null());
        dashterm_graph_free(graph);
    }

    #[test]
    fn test_graph_add_node_and_edge() {
        let name = CString::new("Test Graph").unwrap();
        let graph = dashterm_graph_new(name.as_ptr());
        assert!(!graph.is_null());

        let node1_id = CString::new("node1").unwrap();
        let node1_label = CString::new("Start").unwrap();
        let result = dashterm_graph_add_node(graph, node1_id.as_ptr(), node1_label.as_ptr(), 0);
        assert!(!result.is_null());
        dashterm_string_free(result);

        let node2_id = CString::new("node2").unwrap();
        let node2_label = CString::new("End").unwrap();
        let result = dashterm_graph_add_node(graph, node2_id.as_ptr(), node2_label.as_ptr(), 1);
        assert!(!result.is_null());
        dashterm_string_free(result);

        // Add edge
        let success = dashterm_graph_add_edge(graph, node1_id.as_ptr(), node2_id.as_ptr(), 0);
        assert!(success);

        dashterm_graph_free(graph);
    }

    #[test]
    fn test_graph_zoom_to_fit() {
        let name = CString::new("Test Graph").unwrap();
        let graph = dashterm_graph_new(name.as_ptr());
        assert!(!graph.is_null());

        // Add some nodes to create a graph
        let node1_id = CString::new("start").unwrap();
        let node1_label = CString::new("Start").unwrap();
        let _ = dashterm_graph_add_node(graph, node1_id.as_ptr(), node1_label.as_ptr(), 0);

        let node2_id = CString::new("end").unwrap();
        let node2_label = CString::new("End").unwrap();
        let _ = dashterm_graph_add_node(graph, node2_id.as_ptr(), node2_label.as_ptr(), 1);

        // Add edge
        dashterm_graph_add_edge(graph, node1_id.as_ptr(), node2_id.as_ptr(), 0);

        // Compute layout
        dashterm_graph_compute_layout(graph);

        // Test zoom to fit
        let result = dashterm_graph_compute_zoom_to_fit(graph, 800.0, 600.0);

        // Should have valid zoom values
        assert!(result.zoom >= 0.1);
        assert!(result.zoom <= 2.0);

        dashterm_graph_free(graph);
    }

    #[test]
    fn test_graph_bounding_box() {
        let name = CString::new("Test Graph").unwrap();
        let graph = dashterm_graph_new(name.as_ptr());
        assert!(!graph.is_null());

        // Add a node
        let node_id = CString::new("start").unwrap();
        let node_label = CString::new("Start").unwrap();
        let _ = dashterm_graph_add_node(graph, node_id.as_ptr(), node_label.as_ptr(), 0);

        // Compute layout
        dashterm_graph_compute_layout(graph);

        // Get bounding box JSON
        let json_ptr = dashterm_graph_get_bounding_box_json(graph);
        assert!(!json_ptr.is_null());

        let json_str = unsafe { CStr::from_ptr(json_ptr) }.to_str().unwrap();
        assert!(json_str.contains("min_x"));
        assert!(json_str.contains("max_y"));

        dashterm_string_free(json_ptr);
        dashterm_graph_free(graph);
    }

    #[test]
    fn test_terminal_scrollback() {
        let terminal = dashterm_terminal_new(80, 24);
        assert!(!terminal.is_null());

        // Initially, history should be empty and display offset should be 0
        let history_size = dashterm_terminal_get_history_size(terminal);
        assert_eq!(history_size, 0, "Initial history should be empty");

        let display_offset = dashterm_terminal_get_display_offset(terminal);
        assert_eq!(display_offset, 0, "Initial display offset should be 0");

        // Generate some output to create history (enough lines to overflow the 24-row screen)
        for i in 0..50 {
            let line = format!("Line {}\r\n", i);
            dashterm_terminal_process(terminal, line.as_ptr(), line.len());
        }

        // Now we should have some history
        let history_size = dashterm_terminal_get_history_size(terminal);
        assert!(history_size > 0, "Should have scrollback history after output");

        // Scroll up
        dashterm_terminal_scroll_up(terminal, 5);
        let offset = dashterm_terminal_get_display_offset(terminal);
        assert_eq!(offset, 5, "Display offset should be 5 after scrolling up");

        // Scroll down
        dashterm_terminal_scroll_down(terminal, 3);
        let offset = dashterm_terminal_get_display_offset(terminal);
        assert_eq!(offset, 2, "Display offset should be 2 after scrolling down");

        // Scroll to bottom
        dashterm_terminal_scroll_to_bottom(terminal);
        let offset = dashterm_terminal_get_display_offset(terminal);
        assert_eq!(offset, 0, "Display offset should be 0 after scroll to bottom");

        // Scroll to top
        dashterm_terminal_scroll_to_top(terminal);
        let offset = dashterm_terminal_get_display_offset(terminal);
        assert_eq!(offset, history_size, "Display offset should equal history size after scroll to top");

        dashterm_terminal_free(terminal);
    }
}
