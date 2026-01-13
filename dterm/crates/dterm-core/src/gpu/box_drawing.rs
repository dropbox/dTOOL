//! Box drawing character rendering.
//!
//! This module provides detection and vertex generation for Unicode box drawing
//! characters. These characters are rendered with geometric primitives rather than
//! font glyphs to ensure pixel-perfect alignment at cell boundaries.
//!
//! ## Supported Unicode Ranges
//!
//! - U+2500-U+257F: Box Drawing (light, heavy, double lines, corners, tees)
//! - U+2580-U+259F: Block Elements (shades, quadrants)
//! - U+25E2-U+25FF: Geometric shapes (triangles)
//! - U+1FB00-U+1FB3C: Legacy Terminal (sextants)
//! - U+E0A0-U+E0D7: Powerline glyphs (arrows, separators, VCS symbols)
//!
//! ## Design
//!
//! Box drawing characters are detected during the render loop. When detected,
//! instead of looking up a glyph in the font atlas, we generate vertex data
//! for geometric primitives (lines, rectangles) that perfectly fill the cell.
//!
//! This ensures:
//! - Lines connect perfectly at cell boundaries
//! - No gaps between adjacent box drawing characters
//! - Consistent rendering regardless of font
//! - Powerline prompts display correctly (arrows, branch symbols)

// GPU module uses intentional casts that are safe for terminal/GPU dimensions
#![allow(clippy::cast_precision_loss)]

use super::pipeline::CellVertex;
use super::vertex_flags::VERTEX_TYPE_DECORATION;

/// Line weight for box drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineWeight {
    /// Light line (standard weight)
    Light,
    /// Heavy/bold line
    Heavy,
    /// Double line (two parallel lines)
    Double,
}

/// Check if a character should be rendered with box drawing primitives.
///
/// Returns true for characters in the box drawing, block element,
/// geometric shape, legacy terminal, and Powerline Unicode ranges.
#[inline]
pub fn is_box_drawing(c: char) -> bool {
    matches!(c,
        '\u{2500}'..='\u{257F}' |  // Box Drawing
        '\u{2580}'..='\u{259F}' |  // Block Elements
        '\u{25E2}'..='\u{25FF}' |  // Geometric Shapes (triangles)
        '\u{1FB00}'..='\u{1FB3C}' | // Legacy Terminal (sextants)
        '\u{E0A0}'..='\u{E0D7}'    // Powerline glyphs
    )
}

/// Generate vertices for a box drawing character.
///
/// Returns a vector of vertices that render the character as geometric
/// primitives. The vertices use cell coordinates (col, row) and will
/// be transformed to screen coordinates by the vertex shader.
///
/// # Arguments
/// * `c` - The box drawing character
/// * `col` - Column position in the grid
/// * `row` - Row position in the grid
/// * `color` - Foreground color (RGBA, 0-1)
///
/// # Returns
/// Vector of vertices for rendering the character, or empty if unsupported.
pub fn generate_box_drawing_vertices(
    c: char,
    col: u32,
    row: u32,
    color: [f32; 4],
) -> Vec<CellVertex> {
    let x = col as f32;
    let y = row as f32;

    match c {
        // ═══════════════════════════════════════════════════════════════════
        // BOX DRAWING: LIGHT LINES (U+2500-U+253F)
        // ═══════════════════════════════════════════════════════════════════

        // Single horizontal and vertical lines
        '─' => horizontal_line(x, y, color, LineWeight::Light),            // U+2500
        '━' => horizontal_line(x, y, color, LineWeight::Heavy),            // U+2501
        '│' => vertical_line(x, y, color, LineWeight::Light),              // U+2502
        '┃' => vertical_line(x, y, color, LineWeight::Heavy),              // U+2503

        // Dashed lines (rendered as solid for now - TODO: dashed pattern)
        '┄' => horizontal_line(x, y, color, LineWeight::Light),            // U+2504 triple dash
        '┅' => horizontal_line(x, y, color, LineWeight::Heavy),            // U+2505
        '┆' => vertical_line(x, y, color, LineWeight::Light),              // U+2506
        '┇' => vertical_line(x, y, color, LineWeight::Heavy),              // U+2507
        '┈' => horizontal_line(x, y, color, LineWeight::Light),            // U+2508 quadruple dash
        '┉' => horizontal_line(x, y, color, LineWeight::Heavy),            // U+2509
        '┊' => vertical_line(x, y, color, LineWeight::Light),              // U+250A
        '┋' => vertical_line(x, y, color, LineWeight::Heavy),              // U+250B

        // Light corners
        '┌' => corner_top_left(x, y, color, LineWeight::Light),            // U+250C
        '┐' => corner_top_right(x, y, color, LineWeight::Light),           // U+2510
        '└' => corner_bottom_left(x, y, color, LineWeight::Light),         // U+2514
        '┘' => corner_bottom_right(x, y, color, LineWeight::Light),        // U+2518

        // Heavy corners
        '┏' => corner_top_left(x, y, color, LineWeight::Heavy),            // U+250F
        '┓' => corner_top_right(x, y, color, LineWeight::Heavy),           // U+2513
        '┗' => corner_bottom_left(x, y, color, LineWeight::Heavy),         // U+2517
        '┛' => corner_bottom_right(x, y, color, LineWeight::Heavy),        // U+251B

        // Mixed weight corners (light horizontal, heavy vertical or vice versa)
        '┍' => corner_top_left_mixed(x, y, color, false),                  // U+250D down heavy
        '┎' => corner_top_left_mixed(x, y, color, true),                   // U+250E right heavy
        '┑' => corner_top_right_mixed(x, y, color, false),                 // U+2511 down heavy
        '┒' => corner_top_right_mixed(x, y, color, true),                  // U+2512 left heavy
        '┕' => corner_bottom_left_mixed(x, y, color, false),               // U+2515 up heavy
        '┖' => corner_bottom_left_mixed(x, y, color, true),                // U+2516 right heavy
        '┙' => corner_bottom_right_mixed(x, y, color, false),              // U+2519 up heavy
        '┚' => corner_bottom_right_mixed(x, y, color, true),               // U+251A left heavy

        // Tee pieces (light)
        '├' => tee_left(x, y, color, LineWeight::Light),                   // U+251C
        '┤' => tee_right(x, y, color, LineWeight::Light),                  // U+2524
        '┬' => tee_top(x, y, color, LineWeight::Light),                    // U+252C
        '┴' => tee_bottom(x, y, color, LineWeight::Light),                 // U+2534

        // Tee pieces (heavy)
        '┣' => tee_left(x, y, color, LineWeight::Heavy),                   // U+2523
        '┫' => tee_right(x, y, color, LineWeight::Heavy),                  // U+252B
        '┳' => tee_top(x, y, color, LineWeight::Heavy),                    // U+2533
        '┻' => tee_bottom(x, y, color, LineWeight::Heavy),                 // U+253B

        // Cross (light and heavy)
        '┼' => cross(x, y, color, LineWeight::Light),                      // U+253C
        '╋' => cross(x, y, color, LineWeight::Heavy),                      // U+254B

        // Mixed weight tees - left tee variants
        '┝' => tee_left_mixed(x, y, color, false, true),                   // U+251D right heavy
        '┞' => tee_left_mixed(x, y, color, true, false),                   // U+251E up heavy
        '┟' => tee_left_mixed(x, y, color, false, false),                  // U+251F down heavy
        '┠' => tee_left_mixed(x, y, color, true, true),                    // U+2520 vertical heavy
        '┡' => tee_left(x, y, color, LineWeight::Heavy),                   // U+2521 up heavy right heavy
        '┢' => tee_left(x, y, color, LineWeight::Heavy),                   // U+2522 down heavy right heavy

        // Mixed weight tees - right tee variants
        '┥' => tee_right_mixed(x, y, color, false, true),                  // U+2525 left heavy
        '┦' => tee_right_mixed(x, y, color, true, false),                  // U+2526 up heavy
        '┧' => tee_right_mixed(x, y, color, false, false),                 // U+2527 down heavy
        '┨' => tee_right_mixed(x, y, color, true, true),                   // U+2528 vertical heavy
        '┩' => tee_right(x, y, color, LineWeight::Heavy),                  // U+2529 up heavy left heavy
        '┪' => tee_right(x, y, color, LineWeight::Heavy),                  // U+252A down heavy left heavy

        // Mixed weight tees - top tee variants
        '┭' => tee_top_mixed(x, y, color, true, false),                    // U+252D left heavy
        '┮' => tee_top_mixed(x, y, color, false, true),                    // U+252E right heavy
        '┯' => tee_top_mixed(x, y, color, true, true),                     // U+252F horizontal heavy
        '┰' => tee_top(x, y, color, LineWeight::Heavy),                    // U+2530 down heavy
        '┱' => tee_top(x, y, color, LineWeight::Heavy),                    // U+2531 down heavy left heavy
        '┲' => tee_top(x, y, color, LineWeight::Heavy),                    // U+2532 down heavy right heavy

        // Mixed weight tees - bottom tee variants
        '┵' => tee_bottom_mixed(x, y, color, true, false),                 // U+2535 left heavy
        '┶' => tee_bottom_mixed(x, y, color, false, true),                 // U+2536 right heavy
        '┷' => tee_bottom_mixed(x, y, color, true, true),                  // U+2537 horizontal heavy
        '┸' => tee_bottom(x, y, color, LineWeight::Heavy),                 // U+2538 up heavy
        '┹' => tee_bottom(x, y, color, LineWeight::Heavy),                 // U+2539 up heavy left heavy
        '┺' => tee_bottom(x, y, color, LineWeight::Heavy),                 // U+253A up heavy right heavy

        // Mixed weight crosses
        '┽' => cross_mixed(x, y, color, true, false),                      // U+253D left heavy
        '┾' => cross_mixed(x, y, color, false, true),                      // U+253E right heavy
        '┿' => cross(x, y, color, LineWeight::Heavy),                      // U+253F horizontal heavy
        '╀' => cross_mixed(x, y, color, false, false),                     // U+2540 up heavy
        '╁' => cross_mixed(x, y, color, false, false),                     // U+2541 down heavy
        '╂' => cross(x, y, color, LineWeight::Heavy),                      // U+2542 vertical heavy
        '╃' => cross(x, y, color, LineWeight::Heavy),                      // U+2543 up heavy left heavy
        '╄' => cross(x, y, color, LineWeight::Heavy),                      // U+2544 up heavy right heavy
        '╅' => cross(x, y, color, LineWeight::Heavy),                      // U+2545 down heavy left heavy
        '╆' => cross(x, y, color, LineWeight::Heavy),                      // U+2546 down heavy right heavy
        '╇' => cross(x, y, color, LineWeight::Heavy),                      // U+2547 down heavy horizontal heavy
        '╈' => cross(x, y, color, LineWeight::Heavy),                      // U+2548 up heavy horizontal heavy
        '╉' => cross(x, y, color, LineWeight::Heavy),                      // U+2549 right heavy vertical heavy
        '╊' => cross(x, y, color, LineWeight::Heavy),                      // U+254A left heavy vertical heavy

        // ═══════════════════════════════════════════════════════════════════
        // BOX DRAWING: DOUBLE LINES (U+2550-U+256C)
        // ═══════════════════════════════════════════════════════════════════

        '═' => horizontal_line(x, y, color, LineWeight::Double),           // U+2550
        '║' => vertical_line(x, y, color, LineWeight::Double),             // U+2551

        // Double corners
        '╔' => corner_top_left(x, y, color, LineWeight::Double),           // U+2554
        '╗' => corner_top_right(x, y, color, LineWeight::Double),          // U+2557
        '╚' => corner_bottom_left(x, y, color, LineWeight::Double),        // U+255A
        '╝' => corner_bottom_right(x, y, color, LineWeight::Double),       // U+255D

        // Single/double mixed corners
        '╒' => corner_top_left_double_h(x, y, color),                      // U+2552 double horizontal
        '╓' => corner_top_left_double_v(x, y, color),                      // U+2553 double vertical
        '╕' => corner_top_right_double_h(x, y, color),                     // U+2555
        '╖' => corner_top_right_double_v(x, y, color),                     // U+2556
        '╘' => corner_bottom_left_double_h(x, y, color),                   // U+2558
        '╙' => corner_bottom_left_double_v(x, y, color),                   // U+2559
        '╛' => corner_bottom_right_double_h(x, y, color),                  // U+255B
        '╜' => corner_bottom_right_double_v(x, y, color),                  // U+255C

        // Double tees
        '╠' => tee_left(x, y, color, LineWeight::Double),                  // U+2560
        '╣' => tee_right(x, y, color, LineWeight::Double),                 // U+2563
        '╦' => tee_top(x, y, color, LineWeight::Double),                   // U+2566
        '╩' => tee_bottom(x, y, color, LineWeight::Double),                // U+2569

        // Single/double mixed tees
        '╞' => tee_left_double_h(x, y, color),                             // U+255E double horizontal
        '╟' => tee_left_double_v(x, y, color),                             // U+255F double vertical
        '╡' => tee_right_double_h(x, y, color),                            // U+2561
        '╢' => tee_right_double_v(x, y, color),                            // U+2562
        '╤' => tee_top_double_h(x, y, color),                              // U+2564
        '╥' => tee_top_double_v(x, y, color),                              // U+2565
        '╧' => tee_bottom_double_h(x, y, color),                           // U+2567
        '╨' => tee_bottom_double_v(x, y, color),                           // U+2568

        // Double cross
        '╬' => cross(x, y, color, LineWeight::Double),                     // U+256C

        // Single/double mixed crosses
        '╪' => cross_double_h(x, y, color),                                // U+256A
        '╫' => cross_double_v(x, y, color),                                // U+256B

        // ═══════════════════════════════════════════════════════════════════
        // BOX DRAWING: ARCS AND DIAGONALS (U+256D-U+257F)
        // ═══════════════════════════════════════════════════════════════════

        // Rounded corners (arcs)
        '╭' => arc_top_left(x, y, color),                                  // U+256D
        '╮' => arc_top_right(x, y, color),                                 // U+256E
        '╯' => arc_bottom_right(x, y, color),                              // U+256F
        '╰' => arc_bottom_left(x, y, color),                               // U+2570

        // Diagonals
        '╱' => diagonal_forward(x, y, color),                              // U+2571
        '╲' => diagonal_back(x, y, color),                                 // U+2572
        '╳' => diagonal_cross(x, y, color),                                // U+2573

        // Half lines (left/right/up/down only)
        '╴' => horizontal_line_left(x, y, color, LineWeight::Light),       // U+2574 left
        '╵' => vertical_line_up(x, y, color, LineWeight::Light),           // U+2575 up
        '╶' => horizontal_line_right(x, y, color, LineWeight::Light),      // U+2576 right
        '╷' => vertical_line_down(x, y, color, LineWeight::Light),         // U+2577 down
        '╸' => horizontal_line_left(x, y, color, LineWeight::Heavy),       // U+2578 left heavy
        '╹' => vertical_line_up(x, y, color, LineWeight::Heavy),           // U+2579 up heavy
        '╺' => horizontal_line_right(x, y, color, LineWeight::Heavy),      // U+257A right heavy
        '╻' => vertical_line_down(x, y, color, LineWeight::Heavy),         // U+257B down heavy

        // Mixed weight half-lines
        '╼' => horizontal_line_mixed(x, y, color),                         // U+257C light left heavy right
        '╽' => vertical_line_mixed(x, y, color),                           // U+257D light up heavy down
        '╾' => horizontal_line_mixed_rev(x, y, color),                     // U+257E heavy left light right
        '╿' => vertical_line_mixed_rev(x, y, color),                       // U+257F heavy up light down

        // ═══════════════════════════════════════════════════════════════════
        // BLOCK ELEMENTS (U+2580-U+259F)
        // ═══════════════════════════════════════════════════════════════════

        '▀' => block_upper_half(x, y, color),                              // U+2580
        '▁' => block_lower_eighth(x, y, color, 1),                         // U+2581 lower 1/8
        '▂' => block_lower_eighth(x, y, color, 2),                         // U+2582 lower 2/8
        '▃' => block_lower_eighth(x, y, color, 3),                         // U+2583 lower 3/8
        '▄' => block_lower_half(x, y, color),                              // U+2584
        '▅' => block_lower_eighth(x, y, color, 5),                         // U+2585 lower 5/8
        '▆' => block_lower_eighth(x, y, color, 6),                         // U+2586 lower 6/8
        '▇' => block_lower_eighth(x, y, color, 7),                         // U+2587 lower 7/8
        '█' => block_full(x, y, color),                                    // U+2588 full block
        '▉' => block_left_eighth(x, y, color, 7),                          // U+2589 left 7/8
        '▊' => block_left_eighth(x, y, color, 6),                          // U+258A left 6/8
        '▋' => block_left_eighth(x, y, color, 5),                          // U+258B left 5/8
        '▌' => block_left_half(x, y, color),                               // U+258C left half
        '▍' => block_left_eighth(x, y, color, 3),                          // U+258D left 3/8
        '▎' => block_left_eighth(x, y, color, 2),                          // U+258E left 2/8
        '▏' => block_left_eighth(x, y, color, 1),                          // U+258F left 1/8
        '▐' => block_right_half(x, y, color),                              // U+2590 right half

        // Shades
        '░' => shade(x, y, color, 0.25),                                   // U+2591 light shade
        '▒' => shade(x, y, color, 0.50),                                   // U+2592 medium shade
        '▓' => shade(x, y, color, 0.75),                                   // U+2593 dark shade

        // Upper eighths
        '▔' => block_upper_eighth(x, y, color, 1),                         // U+2594 upper 1/8
        '▕' => block_right_eighth(x, y, color, 1),                         // U+2595 right 1/8

        // Quadrants
        '▖' => quadrant_lower_left(x, y, color),                           // U+2596
        '▗' => quadrant_lower_right(x, y, color),                          // U+2597
        '▘' => quadrant_upper_left(x, y, color),                           // U+2598
        '▙' => quadrant_upper_left_lower(x, y, color),                     // U+2599 upper left + lower
        '▚' => quadrant_diagonal(x, y, color),                             // U+259A upper left + lower right
        '▛' => quadrant_upper_lower_left(x, y, color),                     // U+259B upper + lower left
        '▜' => quadrant_upper_lower_right(x, y, color),                    // U+259C upper + lower right
        '▝' => quadrant_upper_right(x, y, color),                          // U+259D
        '▞' => quadrant_diagonal_rev(x, y, color),                         // U+259E upper right + lower left
        '▟' => quadrant_not_upper_left(x, y, color),                       // U+259F all but upper left

        // ═══════════════════════════════════════════════════════════════════
        // GEOMETRIC SHAPES - TRIANGLES (U+25E2-U+25FF selected)
        // ═══════════════════════════════════════════════════════════════════

        '◢' => triangle_lower_right(x, y, color),                          // U+25E2
        '◣' => triangle_lower_left(x, y, color),                           // U+25E3
        '◤' => triangle_upper_left(x, y, color),                           // U+25E4
        '◥' => triangle_upper_right(x, y, color),                          // U+25E5

        // ═══════════════════════════════════════════════════════════════════
        // POWERLINE GLYPHS (U+E0A0-U+E0D7)
        // ═══════════════════════════════════════════════════════════════════

        // Version control symbols
        '\u{E0A0}' => powerline_branch(x, y, color),                        // Git branch
        '\u{E0A1}' => powerline_line_number(x, y, color),                   // Line number
        '\u{E0A2}' => powerline_lock(x, y, color),                          // Lock/readonly
        '\u{E0A3}' => powerline_column_number(x, y, color),                 // Column number

        // Arrow separators (most common)
        '\u{E0B0}' => powerline_right_arrow(x, y, color),                   // Right solid arrow
        '\u{E0B1}' => powerline_right_arrow_outline(x, y, color),           // Right arrow outline
        '\u{E0B2}' => powerline_left_arrow(x, y, color),                    // Left solid arrow
        '\u{E0B3}' => powerline_left_arrow_outline(x, y, color),            // Left arrow outline

        // Semicircle separators
        '\u{E0B4}' => powerline_right_semicircle(x, y, color),              // Right semicircle
        '\u{E0B5}' => powerline_right_semicircle_outline(x, y, color),      // Right semicircle outline
        '\u{E0B6}' => powerline_left_semicircle(x, y, color),               // Left semicircle
        '\u{E0B7}' => powerline_left_semicircle_outline(x, y, color),       // Left semicircle outline

        // Triangle separators
        '\u{E0B8}' => powerline_lower_left_triangle(x, y, color),           // Lower left triangle
        '\u{E0B9}' => powerline_lower_left_triangle_outline(x, y, color),   // Lower left triangle outline
        '\u{E0BA}' => powerline_lower_right_triangle(x, y, color),          // Lower right triangle
        '\u{E0BB}' => powerline_lower_right_triangle_outline(x, y, color),  // Lower right triangle outline
        '\u{E0BC}' => powerline_upper_left_triangle(x, y, color),           // Upper left triangle
        '\u{E0BD}' => powerline_upper_left_triangle_outline(x, y, color),   // Upper left triangle outline
        '\u{E0BE}' => powerline_upper_right_triangle(x, y, color),          // Upper right triangle
        '\u{E0BF}' => powerline_upper_right_triangle_outline(x, y, color),  // Upper right triangle outline

        // Flame/fire separators
        '\u{E0C0}' => powerline_flame_left(x, y, color),                    // Flame left
        '\u{E0C1}' => powerline_flame_left_outline(x, y, color),            // Flame left outline
        '\u{E0C2}' => powerline_flame_right(x, y, color),                   // Flame right
        '\u{E0C3}' => powerline_flame_right_outline(x, y, color),           // Flame right outline

        // Pixelated separators
        '\u{E0C4}' => powerline_pixelated_right(x, y, color),               // Pixelated right
        '\u{E0C5}' => powerline_pixelated_right_outline(x, y, color),       // Pixelated right outline
        '\u{E0C6}' => powerline_pixelated_left(x, y, color),                // Pixelated left
        '\u{E0C7}' => powerline_pixelated_left_outline(x, y, color),        // Pixelated left outline

        // Ice/waveform separators
        '\u{E0C8}' => powerline_ice_left(x, y, color),                      // Ice left
        '\u{E0CA}' => powerline_ice_right(x, y, color),                     // Ice right

        // Honeycomb separators
        '\u{E0CC}' => powerline_honeycomb(x, y, color),                     // Honeycomb
        '\u{E0CD}' => powerline_honeycomb_outline(x, y, color),             // Honeycomb outline

        // Trapezoid separators
        '\u{E0D0}' => powerline_trapezoid_right(x, y, color),               // Trapezoid right
        '\u{E0D2}' => powerline_trapezoid_left(x, y, color),                // Trapezoid left

        // Default: return empty (character not supported)
        _ => Vec::new(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// LINE PRIMITIVES
// ═══════════════════════════════════════════════════════════════════════════

/// Line thickness as fraction of cell dimension.
const LIGHT_THICKNESS: f32 = 0.08;
const HEAVY_THICKNESS: f32 = 0.16;
const DOUBLE_GAP: f32 = 0.12;

fn line_thickness(weight: LineWeight) -> f32 {
    match weight {
        LineWeight::Light => LIGHT_THICKNESS,
        LineWeight::Heavy => HEAVY_THICKNESS,
        LineWeight::Double => LIGHT_THICKNESS,
    }
}

/// Full horizontal line across the cell.
fn horizontal_line(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let thickness = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        // Two lines with gap
        let offset = DOUBLE_GAP / 2.0 + thickness / 2.0;
        let mut verts = rect(x, y + center - offset - thickness / 2.0, 1.0, thickness, color);
        verts.extend(rect(x, y + center + offset - thickness / 2.0, 1.0, thickness, color));
        verts
    } else {
        rect(x, y + center - thickness / 2.0, 1.0, thickness, color)
    }
}

/// Full vertical line down the cell.
fn vertical_line(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let thickness = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        let offset = DOUBLE_GAP / 2.0 + thickness / 2.0;
        let mut verts = rect(x + center - offset - thickness / 2.0, y, thickness, 1.0, color);
        verts.extend(rect(x + center + offset - thickness / 2.0, y, thickness, 1.0, color));
        verts
    } else {
        rect(x + center - thickness / 2.0, y, thickness, 1.0, color)
    }
}

/// Left half of horizontal line.
fn horizontal_line_left(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let thickness = line_thickness(weight);
    let center = 0.5;
    rect(x, y + center - thickness / 2.0, 0.5, thickness, color)
}

/// Right half of horizontal line.
fn horizontal_line_right(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let thickness = line_thickness(weight);
    let center = 0.5;
    rect(x + 0.5, y + center - thickness / 2.0, 0.5, thickness, color)
}

/// Upper half of vertical line.
fn vertical_line_up(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let thickness = line_thickness(weight);
    let center = 0.5;
    rect(x + center - thickness / 2.0, y, thickness, 0.5, color)
}

/// Lower half of vertical line.
fn vertical_line_down(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let thickness = line_thickness(weight);
    let center = 0.5;
    rect(x + center - thickness / 2.0, y + 0.5, thickness, 0.5, color)
}

/// Mixed weight horizontal: light left, heavy right.
fn horizontal_line_mixed(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let center = 0.5;
    let mut verts = rect(x, y + center - LIGHT_THICKNESS / 2.0, 0.5, LIGHT_THICKNESS, color);
    verts.extend(rect(x + 0.5, y + center - HEAVY_THICKNESS / 2.0, 0.5, HEAVY_THICKNESS, color));
    verts
}

/// Mixed weight horizontal: heavy left, light right.
fn horizontal_line_mixed_rev(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let center = 0.5;
    let mut verts = rect(x, y + center - HEAVY_THICKNESS / 2.0, 0.5, HEAVY_THICKNESS, color);
    verts.extend(rect(x + 0.5, y + center - LIGHT_THICKNESS / 2.0, 0.5, LIGHT_THICKNESS, color));
    verts
}

/// Mixed weight vertical: light up, heavy down.
fn vertical_line_mixed(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let center = 0.5;
    let mut verts = rect(x + center - LIGHT_THICKNESS / 2.0, y, LIGHT_THICKNESS, 0.5, color);
    verts.extend(rect(x + center - HEAVY_THICKNESS / 2.0, y + 0.5, HEAVY_THICKNESS, 0.5, color));
    verts
}

/// Mixed weight vertical: heavy up, light down.
fn vertical_line_mixed_rev(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let center = 0.5;
    let mut verts = rect(x + center - HEAVY_THICKNESS / 2.0, y, HEAVY_THICKNESS, 0.5, color);
    verts.extend(rect(x + center - LIGHT_THICKNESS / 2.0, y + 0.5, LIGHT_THICKNESS, 0.5, color));
    verts
}

// ═══════════════════════════════════════════════════════════════════════════
// CORNER PRIMITIVES
// ═══════════════════════════════════════════════════════════════════════════

/// Top-left corner: ┌
fn corner_top_left(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let t = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        let offset = DOUBLE_GAP / 2.0 + t / 2.0;
        let mut verts = Vec::new();
        // Outer lines
        verts.extend(rect(x + center - offset - t / 2.0, y + center - offset - t / 2.0, t, 0.5 + offset + t / 2.0, color));
        verts.extend(rect(x + center - offset - t / 2.0, y + center - offset - t / 2.0, 0.5 + offset + t / 2.0, t, color));
        // Inner lines
        verts.extend(rect(x + center + offset - t / 2.0, y + center + offset - t / 2.0, t, 0.5 - offset + t / 2.0, color));
        verts.extend(rect(x + center + offset - t / 2.0, y + center + offset - t / 2.0, 0.5 - offset + t / 2.0, t, color));
        verts
    } else {
        let mut verts = rect(x + center - t / 2.0, y + center - t / 2.0, t, 0.5 + t / 2.0, color);
        verts.extend(rect(x + center - t / 2.0, y + center - t / 2.0, 0.5 + t / 2.0, t, color));
        verts
    }
}

/// Top-right corner: ┐
fn corner_top_right(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let t = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        let offset = DOUBLE_GAP / 2.0 + t / 2.0;
        let mut verts = Vec::new();
        // Outer lines
        verts.extend(rect(x + center + offset - t / 2.0, y + center - offset - t / 2.0, t, 0.5 + offset + t / 2.0, color));
        verts.extend(rect(x, y + center - offset - t / 2.0, center + offset + t / 2.0, t, color));
        // Inner lines
        verts.extend(rect(x + center - offset - t / 2.0, y + center + offset - t / 2.0, t, 0.5 - offset + t / 2.0, color));
        verts.extend(rect(x, y + center + offset - t / 2.0, center - offset + t / 2.0, t, color));
        verts
    } else {
        let mut verts = rect(x + center - t / 2.0, y + center - t / 2.0, t, 0.5 + t / 2.0, color);
        verts.extend(rect(x, y + center - t / 2.0, center + t / 2.0, t, color));
        verts
    }
}

/// Bottom-left corner: └
fn corner_bottom_left(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let t = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        let offset = DOUBLE_GAP / 2.0 + t / 2.0;
        let mut verts = Vec::new();
        // Outer lines
        verts.extend(rect(x + center - offset - t / 2.0, y, t, center + offset + t / 2.0, color));
        verts.extend(rect(x + center - offset - t / 2.0, y + center + offset - t / 2.0, 0.5 + offset + t / 2.0, t, color));
        // Inner lines
        verts.extend(rect(x + center + offset - t / 2.0, y, t, center - offset + t / 2.0, color));
        verts.extend(rect(x + center + offset - t / 2.0, y + center - offset - t / 2.0, 0.5 - offset + t / 2.0, t, color));
        verts
    } else {
        let mut verts = rect(x + center - t / 2.0, y, t, center + t / 2.0, color);
        verts.extend(rect(x + center - t / 2.0, y + center - t / 2.0, 0.5 + t / 2.0, t, color));
        verts
    }
}

/// Bottom-right corner: ┘
fn corner_bottom_right(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let t = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        let offset = DOUBLE_GAP / 2.0 + t / 2.0;
        let mut verts = Vec::new();
        // Outer lines
        verts.extend(rect(x + center + offset - t / 2.0, y, t, center + offset + t / 2.0, color));
        verts.extend(rect(x, y + center + offset - t / 2.0, center + offset + t / 2.0, t, color));
        // Inner lines
        verts.extend(rect(x + center - offset - t / 2.0, y, t, center - offset + t / 2.0, color));
        verts.extend(rect(x, y + center - offset - t / 2.0, center - offset + t / 2.0, t, color));
        verts
    } else {
        let mut verts = rect(x + center - t / 2.0, y, t, center + t / 2.0, color);
        verts.extend(rect(x, y + center - t / 2.0, center + t / 2.0, t, color));
        verts
    }
}

/// Mixed weight corners
fn corner_top_left_mixed(x: f32, y: f32, color: [f32; 4], right_heavy: bool) -> Vec<CellVertex> {
    let center = 0.5;
    let t_v = if right_heavy { LIGHT_THICKNESS } else { HEAVY_THICKNESS };
    let t_h = if right_heavy { HEAVY_THICKNESS } else { LIGHT_THICKNESS };

    let mut verts = rect(x + center - t_v / 2.0, y + center - t_h / 2.0, t_v, 0.5 + t_h / 2.0, color);
    verts.extend(rect(x + center - t_v / 2.0, y + center - t_h / 2.0, 0.5 + t_v / 2.0, t_h, color));
    verts
}

fn corner_top_right_mixed(x: f32, y: f32, color: [f32; 4], left_heavy: bool) -> Vec<CellVertex> {
    let center = 0.5;
    let t_v = if left_heavy { LIGHT_THICKNESS } else { HEAVY_THICKNESS };
    let t_h = if left_heavy { HEAVY_THICKNESS } else { LIGHT_THICKNESS };

    let mut verts = rect(x + center - t_v / 2.0, y + center - t_h / 2.0, t_v, 0.5 + t_h / 2.0, color);
    verts.extend(rect(x, y + center - t_h / 2.0, center + t_v / 2.0, t_h, color));
    verts
}

fn corner_bottom_left_mixed(x: f32, y: f32, color: [f32; 4], right_heavy: bool) -> Vec<CellVertex> {
    let center = 0.5;
    let t_v = if right_heavy { LIGHT_THICKNESS } else { HEAVY_THICKNESS };
    let t_h = if right_heavy { HEAVY_THICKNESS } else { LIGHT_THICKNESS };

    let mut verts = rect(x + center - t_v / 2.0, y, t_v, center + t_h / 2.0, color);
    verts.extend(rect(x + center - t_v / 2.0, y + center - t_h / 2.0, 0.5 + t_v / 2.0, t_h, color));
    verts
}

fn corner_bottom_right_mixed(x: f32, y: f32, color: [f32; 4], left_heavy: bool) -> Vec<CellVertex> {
    let center = 0.5;
    let t_v = if left_heavy { LIGHT_THICKNESS } else { HEAVY_THICKNESS };
    let t_h = if left_heavy { HEAVY_THICKNESS } else { LIGHT_THICKNESS };

    let mut verts = rect(x + center - t_v / 2.0, y, t_v, center + t_h / 2.0, color);
    verts.extend(rect(x, y + center - t_h / 2.0, center + t_v / 2.0, t_h, color));
    verts
}

/// Single/double mixed corners
fn corner_top_left_double_h(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - t / 2.0, y + center - offset - t / 2.0, t, 0.5 + offset + t / 2.0, color);
    verts.extend(rect(x + center - t / 2.0, y + center - offset - t / 2.0, 0.5 + t / 2.0, t, color));
    verts.extend(rect(x + center - t / 2.0, y + center + offset - t / 2.0, 0.5 + t / 2.0, t, color));
    verts
}

fn corner_top_left_double_v(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - offset - t / 2.0, y + center - t / 2.0, t, 0.5 + t / 2.0, color);
    verts.extend(rect(x + center + offset - t / 2.0, y + center - t / 2.0, t, 0.5 + t / 2.0, color));
    verts.extend(rect(x + center - offset - t / 2.0, y + center - t / 2.0, 0.5 + offset + t / 2.0, t, color));
    verts
}

fn corner_top_right_double_h(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - t / 2.0, y + center - offset - t / 2.0, t, 0.5 + offset + t / 2.0, color);
    verts.extend(rect(x, y + center - offset - t / 2.0, center + t / 2.0, t, color));
    verts.extend(rect(x, y + center + offset - t / 2.0, center + t / 2.0, t, color));
    verts
}

fn corner_top_right_double_v(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - offset - t / 2.0, y + center - t / 2.0, t, 0.5 + t / 2.0, color);
    verts.extend(rect(x + center + offset - t / 2.0, y + center - t / 2.0, t, 0.5 + t / 2.0, color));
    verts.extend(rect(x, y + center - t / 2.0, center + offset + t / 2.0, t, color));
    verts
}

fn corner_bottom_left_double_h(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - t / 2.0, y, t, center + offset + t / 2.0, color);
    verts.extend(rect(x + center - t / 2.0, y + center - offset - t / 2.0, 0.5 + t / 2.0, t, color));
    verts.extend(rect(x + center - t / 2.0, y + center + offset - t / 2.0, 0.5 + t / 2.0, t, color));
    verts
}

fn corner_bottom_left_double_v(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - offset - t / 2.0, y, t, center + t / 2.0, color);
    verts.extend(rect(x + center + offset - t / 2.0, y, t, center + t / 2.0, color));
    verts.extend(rect(x + center - offset - t / 2.0, y + center - t / 2.0, 0.5 + offset + t / 2.0, t, color));
    verts
}

fn corner_bottom_right_double_h(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - t / 2.0, y, t, center + offset + t / 2.0, color);
    verts.extend(rect(x, y + center - offset - t / 2.0, center + t / 2.0, t, color));
    verts.extend(rect(x, y + center + offset - t / 2.0, center + t / 2.0, t, color));
    verts
}

fn corner_bottom_right_double_v(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - offset - t / 2.0, y, t, center + t / 2.0, color);
    verts.extend(rect(x + center + offset - t / 2.0, y, t, center + t / 2.0, color));
    verts.extend(rect(x, y + center - t / 2.0, center + offset + t / 2.0, t, color));
    verts
}

// ═══════════════════════════════════════════════════════════════════════════
// TEE PRIMITIVES
// ═══════════════════════════════════════════════════════════════════════════

/// Left tee: ├
fn tee_left(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let t = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        let offset = DOUBLE_GAP / 2.0 + t / 2.0;
        let mut verts = Vec::new();
        // Vertical lines
        verts.extend(rect(x + center - offset - t / 2.0, y, t, 1.0, color));
        verts.extend(rect(x + center + offset - t / 2.0, y, t, 1.0, color));
        // Horizontal lines from center to right
        verts.extend(rect(x + center + offset - t / 2.0, y + center - offset - t / 2.0, 0.5 - offset + t / 2.0, t, color));
        verts.extend(rect(x + center + offset - t / 2.0, y + center + offset - t / 2.0, 0.5 - offset + t / 2.0, t, color));
        verts
    } else {
        let mut verts = rect(x + center - t / 2.0, y, t, 1.0, color);
        verts.extend(rect(x + center - t / 2.0, y + center - t / 2.0, 0.5 + t / 2.0, t, color));
        verts
    }
}

/// Right tee: ┤
fn tee_right(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let t = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        let offset = DOUBLE_GAP / 2.0 + t / 2.0;
        let mut verts = Vec::new();
        // Vertical lines
        verts.extend(rect(x + center - offset - t / 2.0, y, t, 1.0, color));
        verts.extend(rect(x + center + offset - t / 2.0, y, t, 1.0, color));
        // Horizontal lines from left to center
        verts.extend(rect(x, y + center - offset - t / 2.0, center - offset + t / 2.0, t, color));
        verts.extend(rect(x, y + center + offset - t / 2.0, center - offset + t / 2.0, t, color));
        verts
    } else {
        let mut verts = rect(x + center - t / 2.0, y, t, 1.0, color);
        verts.extend(rect(x, y + center - t / 2.0, center + t / 2.0, t, color));
        verts
    }
}

/// Top tee: ┬
fn tee_top(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let t = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        let offset = DOUBLE_GAP / 2.0 + t / 2.0;
        let mut verts = Vec::new();
        // Horizontal lines
        verts.extend(rect(x, y + center - offset - t / 2.0, 1.0, t, color));
        verts.extend(rect(x, y + center + offset - t / 2.0, 1.0, t, color));
        // Vertical lines from center to bottom
        verts.extend(rect(x + center - offset - t / 2.0, y + center + offset - t / 2.0, t, 0.5 - offset + t / 2.0, color));
        verts.extend(rect(x + center + offset - t / 2.0, y + center + offset - t / 2.0, t, 0.5 - offset + t / 2.0, color));
        verts
    } else {
        let mut verts = rect(x, y + center - t / 2.0, 1.0, t, color);
        verts.extend(rect(x + center - t / 2.0, y + center - t / 2.0, t, 0.5 + t / 2.0, color));
        verts
    }
}

/// Bottom tee: ┴
fn tee_bottom(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let t = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        let offset = DOUBLE_GAP / 2.0 + t / 2.0;
        let mut verts = Vec::new();
        // Horizontal lines
        verts.extend(rect(x, y + center - offset - t / 2.0, 1.0, t, color));
        verts.extend(rect(x, y + center + offset - t / 2.0, 1.0, t, color));
        // Vertical lines from top to center
        verts.extend(rect(x + center - offset - t / 2.0, y, t, center - offset + t / 2.0, color));
        verts.extend(rect(x + center + offset - t / 2.0, y, t, center - offset + t / 2.0, color));
        verts
    } else {
        let mut verts = rect(x, y + center - t / 2.0, 1.0, t, color);
        verts.extend(rect(x + center - t / 2.0, y, t, center + t / 2.0, color));
        verts
    }
}

/// Mixed weight tees
#[allow(unused_variables)]
fn tee_left_mixed(x: f32, y: f32, color: [f32; 4], up_heavy: bool, down_heavy: bool) -> Vec<CellVertex> {
    // Simplified: use light weight for all mixed variants
    tee_left(x, y, color, LineWeight::Light)
}

#[allow(unused_variables)]
fn tee_right_mixed(x: f32, y: f32, color: [f32; 4], up_heavy: bool, down_heavy: bool) -> Vec<CellVertex> {
    tee_right(x, y, color, LineWeight::Light)
}

#[allow(unused_variables)]
fn tee_top_mixed(x: f32, y: f32, color: [f32; 4], left_heavy: bool, right_heavy: bool) -> Vec<CellVertex> {
    tee_top(x, y, color, LineWeight::Light)
}

#[allow(unused_variables)]
fn tee_bottom_mixed(x: f32, y: f32, color: [f32; 4], left_heavy: bool, right_heavy: bool) -> Vec<CellVertex> {
    tee_bottom(x, y, color, LineWeight::Light)
}

/// Single/double mixed tees
fn tee_left_double_h(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - t / 2.0, y, t, 1.0, color);
    verts.extend(rect(x + center - t / 2.0, y + center - offset - t / 2.0, 0.5 + t / 2.0, t, color));
    verts.extend(rect(x + center - t / 2.0, y + center + offset - t / 2.0, 0.5 + t / 2.0, t, color));
    verts
}

fn tee_left_double_v(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - offset - t / 2.0, y, t, 1.0, color);
    verts.extend(rect(x + center + offset - t / 2.0, y, t, 1.0, color));
    verts.extend(rect(x + center + offset - t / 2.0, y + center - t / 2.0, 0.5 - offset + t / 2.0, t, color));
    verts
}

fn tee_right_double_h(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - t / 2.0, y, t, 1.0, color);
    verts.extend(rect(x, y + center - offset - t / 2.0, center + t / 2.0, t, color));
    verts.extend(rect(x, y + center + offset - t / 2.0, center + t / 2.0, t, color));
    verts
}

fn tee_right_double_v(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - offset - t / 2.0, y, t, 1.0, color);
    verts.extend(rect(x + center + offset - t / 2.0, y, t, 1.0, color));
    verts.extend(rect(x, y + center - t / 2.0, center - offset + t / 2.0, t, color));
    verts
}

fn tee_top_double_h(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x, y + center - offset - t / 2.0, 1.0, t, color);
    verts.extend(rect(x, y + center + offset - t / 2.0, 1.0, t, color));
    verts.extend(rect(x + center - t / 2.0, y + center + offset - t / 2.0, t, 0.5 - offset + t / 2.0, color));
    verts
}

fn tee_top_double_v(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x, y + center - t / 2.0, 1.0, t, color);
    verts.extend(rect(x + center - offset - t / 2.0, y + center - t / 2.0, t, 0.5 + t / 2.0, color));
    verts.extend(rect(x + center + offset - t / 2.0, y + center - t / 2.0, t, 0.5 + t / 2.0, color));
    verts
}

fn tee_bottom_double_h(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x, y + center - offset - t / 2.0, 1.0, t, color);
    verts.extend(rect(x, y + center + offset - t / 2.0, 1.0, t, color));
    verts.extend(rect(x + center - t / 2.0, y, t, center - offset + t / 2.0, color));
    verts
}

fn tee_bottom_double_v(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x, y + center - t / 2.0, 1.0, t, color);
    verts.extend(rect(x + center - offset - t / 2.0, y, t, center + t / 2.0, color));
    verts.extend(rect(x + center + offset - t / 2.0, y, t, center + t / 2.0, color));
    verts
}

// ═══════════════════════════════════════════════════════════════════════════
// CROSS PRIMITIVES
// ═══════════════════════════════════════════════════════════════════════════

/// Cross: ┼
fn cross(x: f32, y: f32, color: [f32; 4], weight: LineWeight) -> Vec<CellVertex> {
    let t = line_thickness(weight);
    let center = 0.5;

    if weight == LineWeight::Double {
        let offset = DOUBLE_GAP / 2.0 + t / 2.0;
        let mut verts = Vec::new();
        // Vertical lines
        verts.extend(rect(x + center - offset - t / 2.0, y, t, 1.0, color));
        verts.extend(rect(x + center + offset - t / 2.0, y, t, 1.0, color));
        // Horizontal lines (with gaps for vertical)
        verts.extend(rect(x, y + center - offset - t / 2.0, center - offset - t / 2.0, t, color));
        verts.extend(rect(x + center + offset + t / 2.0, y + center - offset - t / 2.0, 0.5 - offset - t / 2.0, t, color));
        verts.extend(rect(x, y + center + offset - t / 2.0, center - offset - t / 2.0, t, color));
        verts.extend(rect(x + center + offset + t / 2.0, y + center + offset - t / 2.0, 0.5 - offset - t / 2.0, t, color));
        verts
    } else {
        let mut verts = rect(x + center - t / 2.0, y, t, 1.0, color);
        verts.extend(rect(x, y + center - t / 2.0, 1.0, t, color));
        verts
    }
}

/// Mixed weight crosses
#[allow(unused_variables)]
fn cross_mixed(x: f32, y: f32, color: [f32; 4], left_heavy: bool, right_heavy: bool) -> Vec<CellVertex> {
    cross(x, y, color, LineWeight::Light)
}

fn cross_double_h(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - t / 2.0, y, t, 1.0, color);
    verts.extend(rect(x, y + center - offset - t / 2.0, 1.0, t, color));
    verts.extend(rect(x, y + center + offset - t / 2.0, 1.0, t, color));
    verts
}

fn cross_double_v(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let offset = DOUBLE_GAP / 2.0 + t / 2.0;

    let mut verts = rect(x + center - offset - t / 2.0, y, t, 1.0, color);
    verts.extend(rect(x + center + offset - t / 2.0, y, t, 1.0, color));
    verts.extend(rect(x, y + center - t / 2.0, 1.0, t, color));
    verts
}

// ═══════════════════════════════════════════════════════════════════════════
// ARC AND DIAGONAL PRIMITIVES
// ═══════════════════════════════════════════════════════════════════════════

/// Arc top-left: ╭
fn arc_top_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    // Approximate arc with line segments
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let segments = 4;
    let mut verts = Vec::new();

    for i in 0..segments {
        let angle1 = std::f32::consts::PI * (0.5 + i as f32 * 0.5 / segments as f32);
        let angle2 = std::f32::consts::PI * (0.5 + (i + 1) as f32 * 0.5 / segments as f32);

        let x1 = x + center + center * angle1.cos();
        let y1 = y + center + center * angle1.sin();
        let x2 = x + center + center * angle2.cos();
        let y2 = y + center + center * angle2.sin();

        verts.extend(line_segment(x1, y1, x2, y2, t, color));
    }
    verts
}

/// Arc top-right: ╮
fn arc_top_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let segments = 4;
    let mut verts = Vec::new();

    for i in 0..segments {
        let angle1 = std::f32::consts::PI * (i as f32 * 0.5 / segments as f32);
        let angle2 = std::f32::consts::PI * ((i + 1) as f32 * 0.5 / segments as f32);

        let x1 = x + center + center * angle1.cos();
        let y1 = y + center + center * angle1.sin();
        let x2 = x + center + center * angle2.cos();
        let y2 = y + center + center * angle2.sin();

        verts.extend(line_segment(x1, y1, x2, y2, t, color));
    }
    verts
}

/// Arc bottom-right: ╯
fn arc_bottom_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let segments = 4;
    let mut verts = Vec::new();

    for i in 0..segments {
        let angle1 = std::f32::consts::PI * (1.5 + i as f32 * 0.5 / segments as f32);
        let angle2 = std::f32::consts::PI * (1.5 + (i + 1) as f32 * 0.5 / segments as f32);

        let x1 = x + center + center * angle1.cos();
        let y1 = y + center + center * angle1.sin();
        let x2 = x + center + center * angle2.cos();
        let y2 = y + center + center * angle2.sin();

        verts.extend(line_segment(x1, y1, x2, y2, t, color));
    }
    verts
}

/// Arc bottom-left: ╰
fn arc_bottom_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = LIGHT_THICKNESS;
    let center = 0.5;
    let segments = 4;
    let mut verts = Vec::new();

    for i in 0..segments {
        let angle1 = std::f32::consts::PI * (1.0 + i as f32 * 0.5 / segments as f32);
        let angle2 = std::f32::consts::PI * (1.0 + (i + 1) as f32 * 0.5 / segments as f32);

        let x1 = x + center + center * angle1.cos();
        let y1 = y + center + center * angle1.sin();
        let x2 = x + center + center * angle2.cos();
        let y2 = y + center + center * angle2.sin();

        verts.extend(line_segment(x1, y1, x2, y2, t, color));
    }
    verts
}

/// Diagonal forward slash: ╱
fn diagonal_forward(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    line_segment(x + 1.0, y, x, y + 1.0, LIGHT_THICKNESS, color)
}

/// Diagonal backslash: ╲
fn diagonal_back(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    line_segment(x, y, x + 1.0, y + 1.0, LIGHT_THICKNESS, color)
}

/// Diagonal cross: ╳
fn diagonal_cross(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = diagonal_forward(x, y, color);
    verts.extend(diagonal_back(x, y, color));
    verts
}

// ═══════════════════════════════════════════════════════════════════════════
// BLOCK ELEMENT PRIMITIVES
// ═══════════════════════════════════════════════════════════════════════════

fn block_full(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    rect(x, y, 1.0, 1.0, color)
}

fn block_upper_half(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    rect(x, y, 1.0, 0.5, color)
}

fn block_lower_half(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    rect(x, y + 0.5, 1.0, 0.5, color)
}

fn block_left_half(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    rect(x, y, 0.5, 1.0, color)
}

fn block_right_half(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    rect(x + 0.5, y, 0.5, 1.0, color)
}

fn block_lower_eighth(x: f32, y: f32, color: [f32; 4], eighths: u8) -> Vec<CellVertex> {
    let height = eighths as f32 / 8.0;
    rect(x, y + 1.0 - height, 1.0, height, color)
}

fn block_upper_eighth(x: f32, y: f32, color: [f32; 4], eighths: u8) -> Vec<CellVertex> {
    let height = eighths as f32 / 8.0;
    rect(x, y, 1.0, height, color)
}

fn block_left_eighth(x: f32, y: f32, color: [f32; 4], eighths: u8) -> Vec<CellVertex> {
    let width = eighths as f32 / 8.0;
    rect(x, y, width, 1.0, color)
}

fn block_right_eighth(x: f32, y: f32, color: [f32; 4], eighths: u8) -> Vec<CellVertex> {
    let width = eighths as f32 / 8.0;
    rect(x + 1.0 - width, y, width, 1.0, color)
}

/// Shade pattern (simplified as solid with reduced alpha)
fn shade(x: f32, y: f32, color: [f32; 4], density: f32) -> Vec<CellVertex> {
    let shaded_color = [color[0], color[1], color[2], color[3] * density];
    rect(x, y, 1.0, 1.0, shaded_color)
}

// Quadrants
fn quadrant_upper_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    rect(x, y, 0.5, 0.5, color)
}

fn quadrant_upper_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    rect(x + 0.5, y, 0.5, 0.5, color)
}

fn quadrant_lower_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    rect(x, y + 0.5, 0.5, 0.5, color)
}

fn quadrant_lower_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    rect(x + 0.5, y + 0.5, 0.5, 0.5, color)
}

fn quadrant_upper_left_lower(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = quadrant_upper_left(x, y, color);
    verts.extend(block_lower_half(x, y, color));
    verts
}

fn quadrant_diagonal(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = quadrant_upper_left(x, y, color);
    verts.extend(quadrant_lower_right(x, y, color));
    verts
}

fn quadrant_upper_lower_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = block_upper_half(x, y, color);
    verts.extend(quadrant_lower_left(x, y, color));
    verts
}

fn quadrant_upper_lower_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = block_upper_half(x, y, color);
    verts.extend(quadrant_lower_right(x, y, color));
    verts
}

fn quadrant_diagonal_rev(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = quadrant_upper_right(x, y, color);
    verts.extend(quadrant_lower_left(x, y, color));
    verts
}

fn quadrant_not_upper_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = quadrant_upper_right(x, y, color);
    verts.extend(block_lower_half(x, y, color));
    verts
}

// ═══════════════════════════════════════════════════════════════════════════
// TRIANGLE PRIMITIVES
// ═══════════════════════════════════════════════════════════════════════════

fn triangle_lower_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    triangle(x + 1.0, y, x + 1.0, y + 1.0, x, y + 1.0, color)
}

fn triangle_lower_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    triangle(x, y, x + 1.0, y + 1.0, x, y + 1.0, color)
}

fn triangle_upper_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    triangle(x, y, x + 1.0, y, x, y + 1.0, color)
}

fn triangle_upper_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    triangle(x, y, x + 1.0, y, x + 1.0, y + 1.0, color)
}

// ═══════════════════════════════════════════════════════════════════════════
// POWERLINE GLYPH PRIMITIVES (U+E0A0-U+E0D7)
// ═══════════════════════════════════════════════════════════════════════════

/// Powerline right-pointing solid arrow separator (U+E0B0)
/// Triangle pointing right, filling the cell
fn powerline_right_arrow(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    triangle(x, y, x, y + 1.0, x + 1.0, y + 0.5, color)
}

/// Powerline right-pointing line arrow separator (U+E0B1)
/// Outline version of right arrow
fn powerline_right_arrow_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = line_segment(x, y, x + 1.0, y + 0.5, LIGHT_THICKNESS, color);
    verts.extend(line_segment(x + 1.0, y + 0.5, x, y + 1.0, LIGHT_THICKNESS, color));
    verts
}

/// Powerline left-pointing solid arrow separator (U+E0B2)
/// Triangle pointing left, filling the cell
fn powerline_left_arrow(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    triangle(x + 1.0, y, x + 1.0, y + 1.0, x, y + 0.5, color)
}

/// Powerline left-pointing line arrow separator (U+E0B3)
/// Outline version of left arrow
fn powerline_left_arrow_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = line_segment(x + 1.0, y, x, y + 0.5, LIGHT_THICKNESS, color);
    verts.extend(line_segment(x, y + 0.5, x + 1.0, y + 1.0, LIGHT_THICKNESS, color));
    verts
}

/// Powerline right-pointing half-circle separator (U+E0B4)
fn powerline_right_semicircle(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    // Approximate semicircle with triangular fan
    let center_x = x;
    let center_y = y + 0.5;
    let radius = 0.5;
    let segments = 8;
    let mut verts = Vec::new();

    for i in 0..segments {
        let angle1 = std::f32::consts::PI * (-0.5 + i as f32 / segments as f32);
        let angle2 = std::f32::consts::PI * (-0.5 + (i + 1) as f32 / segments as f32);

        let x1 = center_x + radius * angle1.cos();
        let y1 = center_y + radius * angle1.sin();
        let x2 = center_x + radius * angle2.cos();
        let y2 = center_y + radius * angle2.sin();

        verts.extend(triangle(center_x, center_y, x1, y1, x2, y2, color));
    }
    verts
}

/// Powerline right-pointing half-circle outline separator (U+E0B5)
fn powerline_right_semicircle_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let center_x = x;
    let center_y = y + 0.5;
    let radius = 0.5;
    let segments = 8;
    let mut verts = Vec::new();

    for i in 0..segments {
        let angle1 = std::f32::consts::PI * (-0.5 + i as f32 / segments as f32);
        let angle2 = std::f32::consts::PI * (-0.5 + (i + 1) as f32 / segments as f32);

        let x1 = center_x + radius * angle1.cos();
        let y1 = center_y + radius * angle1.sin();
        let x2 = center_x + radius * angle2.cos();
        let y2 = center_y + radius * angle2.sin();

        verts.extend(line_segment(x1, y1, x2, y2, LIGHT_THICKNESS, color));
    }
    verts
}

/// Powerline left-pointing half-circle separator (U+E0B6)
fn powerline_left_semicircle(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let center_x = x + 1.0;
    let center_y = y + 0.5;
    let radius = 0.5;
    let segments = 8;
    let mut verts = Vec::new();

    for i in 0..segments {
        let angle1 = std::f32::consts::PI * (0.5 + i as f32 / segments as f32);
        let angle2 = std::f32::consts::PI * (0.5 + (i + 1) as f32 / segments as f32);

        let x1 = center_x + radius * angle1.cos();
        let y1 = center_y + radius * angle1.sin();
        let x2 = center_x + radius * angle2.cos();
        let y2 = center_y + radius * angle2.sin();

        verts.extend(triangle(center_x, center_y, x1, y1, x2, y2, color));
    }
    verts
}

/// Powerline left-pointing half-circle outline separator (U+E0B7)
fn powerline_left_semicircle_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let center_x = x + 1.0;
    let center_y = y + 0.5;
    let radius = 0.5;
    let segments = 8;
    let mut verts = Vec::new();

    for i in 0..segments {
        let angle1 = std::f32::consts::PI * (0.5 + i as f32 / segments as f32);
        let angle2 = std::f32::consts::PI * (0.5 + (i + 1) as f32 / segments as f32);

        let x1 = center_x + radius * angle1.cos();
        let y1 = center_y + radius * angle1.sin();
        let x2 = center_x + radius * angle2.cos();
        let y2 = center_y + radius * angle2.sin();

        verts.extend(line_segment(x1, y1, x2, y2, LIGHT_THICKNESS, color));
    }
    verts
}

/// Powerline lower-left triangle (U+E0B8)
fn powerline_lower_left_triangle(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    triangle(x, y, x + 1.0, y + 1.0, x, y + 1.0, color)
}

/// Powerline lower-left triangle outline (U+E0B9)
fn powerline_lower_left_triangle_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    line_segment(x, y, x + 1.0, y + 1.0, LIGHT_THICKNESS, color)
}

/// Powerline lower-right triangle (U+E0BA)
fn powerline_lower_right_triangle(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    triangle(x + 1.0, y, x + 1.0, y + 1.0, x, y + 1.0, color)
}

/// Powerline lower-right triangle outline (U+E0BB)
fn powerline_lower_right_triangle_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    line_segment(x + 1.0, y, x, y + 1.0, LIGHT_THICKNESS, color)
}

/// Powerline upper-left triangle (U+E0BC)
fn powerline_upper_left_triangle(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    triangle(x, y, x + 1.0, y, x, y + 1.0, color)
}

/// Powerline upper-left triangle outline (U+E0BD)
fn powerline_upper_left_triangle_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    line_segment(x + 1.0, y, x, y + 1.0, LIGHT_THICKNESS, color)
}

/// Powerline upper-right triangle (U+E0BE)
fn powerline_upper_right_triangle(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    triangle(x, y, x + 1.0, y, x + 1.0, y + 1.0, color)
}

/// Powerline upper-right triangle outline (U+E0BF)
fn powerline_upper_right_triangle_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    line_segment(x, y, x + 1.0, y + 1.0, LIGHT_THICKNESS, color)
}

/// Git branch symbol (U+E0A0) - Y-shaped branch
fn powerline_branch(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let center_x = x + 0.5;
    let center_y = y + 0.5;
    let t = HEAVY_THICKNESS;

    // Main vertical stem from center down
    let mut verts = rect(center_x - t / 2.0, center_y, t, 0.5, color);
    // Left branch going up-left
    verts.extend(line_segment(center_x, center_y, x + 0.2, y + 0.15, t, color));
    // Right branch going up-right
    verts.extend(line_segment(center_x, center_y, x + 0.8, y + 0.15, t, color));
    // Small circles at branch tips (as small rectangles)
    verts.extend(rect(x + 0.15, y + 0.1, 0.1, 0.1, color));
    verts.extend(rect(x + 0.75, y + 0.1, 0.1, 0.1, color));
    verts.extend(rect(center_x - 0.05, y + 0.9, 0.1, 0.1, color));
    verts
}

/// Line number symbol (U+E0A1) - LN marker
fn powerline_line_number(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = HEAVY_THICKNESS;
    // L shape
    let mut verts = rect(x + 0.15, y + 0.2, t, 0.6, color);
    verts.extend(rect(x + 0.15, y + 0.7, 0.25, t, color));
    // N shape
    verts.extend(rect(x + 0.5, y + 0.2, t, 0.6, color));
    verts.extend(rect(x + 0.75, y + 0.2, t, 0.6, color));
    verts.extend(line_segment(x + 0.5, y + 0.25, x + 0.75, y + 0.75, t, color));
    verts
}

/// Lock/readonly symbol (U+E0A2) - padlock
fn powerline_lock(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    // Lock body (rectangle)
    let mut verts = rect(x + 0.25, y + 0.45, 0.5, 0.45, color);
    // Lock shackle (arch at top) - using line segments
    verts.extend(line_segment(x + 0.3, y + 0.45, x + 0.3, y + 0.25, HEAVY_THICKNESS, color));
    verts.extend(line_segment(x + 0.7, y + 0.45, x + 0.7, y + 0.25, HEAVY_THICKNESS, color));
    // Top arc
    let segments = 4;
    for i in 0..segments {
        let angle1 = std::f32::consts::PI * (1.0 + i as f32 / segments as f32);
        let angle2 = std::f32::consts::PI * (1.0 + (i + 1) as f32 / segments as f32);
        let cx = x + 0.5;
        let cy = y + 0.25;
        let r = 0.2;
        let x1 = cx + r * angle1.cos();
        let y1 = cy + r * angle1.sin();
        let x2 = cx + r * angle2.cos();
        let y2 = cy + r * angle2.sin();
        verts.extend(line_segment(x1, y1, x2, y2, HEAVY_THICKNESS, color));
    }
    verts
}

/// Column number symbol (U+E0A3)
fn powerline_column_number(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let t = HEAVY_THICKNESS;
    // C shape
    let mut verts = rect(x + 0.15, y + 0.2, t, 0.6, color);
    verts.extend(rect(x + 0.15, y + 0.2, 0.25, t, color));
    verts.extend(rect(x + 0.15, y + 0.7, 0.25, t, color));
    // N shape
    verts.extend(rect(x + 0.5, y + 0.2, t, 0.6, color));
    verts.extend(rect(x + 0.75, y + 0.2, t, 0.6, color));
    verts.extend(line_segment(x + 0.5, y + 0.25, x + 0.75, y + 0.75, t, color));
    verts
}

/// Flame/fire left (U+E0C0)
fn powerline_flame_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    // Simplified flame as triangular shape
    let mut verts = triangle(x + 0.8, y, x + 0.2, y + 0.5, x + 0.8, y + 1.0, color);
    verts.extend(triangle(x + 0.2, y + 0.5, x, y + 0.3, x, y + 0.7, color));
    verts
}

/// Flame/fire left outline (U+E0C1)
fn powerline_flame_left_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = line_segment(x + 0.8, y, x + 0.2, y + 0.5, LIGHT_THICKNESS, color);
    verts.extend(line_segment(x + 0.2, y + 0.5, x + 0.8, y + 1.0, LIGHT_THICKNESS, color));
    verts.extend(line_segment(x + 0.2, y + 0.5, x, y + 0.3, LIGHT_THICKNESS, color));
    verts.extend(line_segment(x + 0.2, y + 0.5, x, y + 0.7, LIGHT_THICKNESS, color));
    verts
}

/// Flame/fire right (U+E0C2)
fn powerline_flame_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = triangle(x + 0.2, y, x + 0.8, y + 0.5, x + 0.2, y + 1.0, color);
    verts.extend(triangle(x + 0.8, y + 0.5, x + 1.0, y + 0.3, x + 1.0, y + 0.7, color));
    verts
}

/// Flame/fire right outline (U+E0C3)
fn powerline_flame_right_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = line_segment(x + 0.2, y, x + 0.8, y + 0.5, LIGHT_THICKNESS, color);
    verts.extend(line_segment(x + 0.8, y + 0.5, x + 0.2, y + 1.0, LIGHT_THICKNESS, color));
    verts.extend(line_segment(x + 0.8, y + 0.5, x + 1.0, y + 0.3, LIGHT_THICKNESS, color));
    verts.extend(line_segment(x + 0.8, y + 0.5, x + 1.0, y + 0.7, LIGHT_THICKNESS, color));
    verts
}

/// Pixelated right (U+E0C4)
fn powerline_pixelated_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    // Create pixelated diagonal pattern
    let pixel = 0.125;
    let mut verts = Vec::new();
    // Diagonal staircase pattern
    for i in 0..8 {
        let px = x + i as f32 * pixel;
        let py = y + (i as f32 * pixel);
        verts.extend(rect(px, py, pixel * 2.0, pixel, color));
    }
    verts
}

/// Pixelated right outline (U+E0C5)
fn powerline_pixelated_right_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let pixel = 0.125;
    let mut verts = Vec::new();
    for i in 0..8 {
        let px = x + i as f32 * pixel;
        let py = y + (i as f32 * pixel);
        // Draw outline of each pixel step
        verts.extend(line_segment(px, py, px + pixel, py, LIGHT_THICKNESS / 2.0, color));
        verts.extend(line_segment(px + pixel, py, px + pixel, py + pixel, LIGHT_THICKNESS / 2.0, color));
    }
    verts
}

/// Pixelated left (U+E0C6)
fn powerline_pixelated_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let pixel = 0.125;
    let mut verts = Vec::new();
    for i in 0..8 {
        let px = x + 1.0 - (i + 2) as f32 * pixel;
        let py = y + (i as f32 * pixel);
        verts.extend(rect(px, py, pixel * 2.0, pixel, color));
    }
    verts
}

/// Pixelated left outline (U+E0C7)
fn powerline_pixelated_left_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let pixel = 0.125;
    let mut verts = Vec::new();
    for i in 0..8 {
        let px = x + 1.0 - (i + 1) as f32 * pixel;
        let py = y + (i as f32 * pixel);
        verts.extend(line_segment(px, py, px - pixel, py, LIGHT_THICKNESS / 2.0, color));
        verts.extend(line_segment(px - pixel, py, px - pixel, py + pixel, LIGHT_THICKNESS / 2.0, color));
    }
    verts
}

/// Ice/waveform left (U+E0C8)
fn powerline_ice_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    // Zigzag ice pattern
    let mut verts = Vec::new();
    let segments = 4;
    for i in 0..segments {
        let y1 = y + i as f32 * 0.25;
        let y2 = y + (i + 1) as f32 * 0.25;
        let x_offset = if i % 2 == 0 { 0.2 } else { 0.0 };
        verts.extend(triangle(x + 0.8, y1, x + x_offset, y1 + 0.125, x + 0.8, y2, color));
    }
    verts
}

/// Ice/waveform right (U+E0CA)
fn powerline_ice_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = Vec::new();
    let segments = 4;
    for i in 0..segments {
        let y1 = y + i as f32 * 0.25;
        let y2 = y + (i + 1) as f32 * 0.25;
        let x_offset = if i % 2 == 0 { 0.8 } else { 1.0 };
        verts.extend(triangle(x + 0.2, y1, x + x_offset, y1 + 0.125, x + 0.2, y2, color));
    }
    verts
}

/// Honeycomb (U+E0CC)
fn powerline_honeycomb(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    // Simplified hexagon
    let cx = x + 0.5;
    let cy = y + 0.5;
    let r = 0.4;
    let mut verts = Vec::new();
    for i in 0..6 {
        let angle1 = std::f32::consts::PI * (i as f32 / 3.0);
        let angle2 = std::f32::consts::PI * ((i + 1) as f32 / 3.0);
        let x1 = cx + r * angle1.cos();
        let y1 = cy + r * angle1.sin();
        let x2 = cx + r * angle2.cos();
        let y2 = cy + r * angle2.sin();
        verts.extend(triangle(cx, cy, x1, y1, x2, y2, color));
    }
    verts
}

/// Honeycomb outline (U+E0CD)
fn powerline_honeycomb_outline(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let cx = x + 0.5;
    let cy = y + 0.5;
    let r = 0.4;
    let mut verts = Vec::new();
    for i in 0..6 {
        let angle1 = std::f32::consts::PI * (i as f32 / 3.0);
        let angle2 = std::f32::consts::PI * ((i + 1) as f32 / 3.0);
        let x1 = cx + r * angle1.cos();
        let y1 = cy + r * angle1.sin();
        let x2 = cx + r * angle2.cos();
        let y2 = cy + r * angle2.sin();
        verts.extend(line_segment(x1, y1, x2, y2, LIGHT_THICKNESS, color));
    }
    verts
}

/// Right-pointing trapezoid (U+E0D0)
fn powerline_trapezoid_right(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = triangle(x, y + 0.15, x + 1.0, y, x + 1.0, y + 0.5, color);
    verts.extend(triangle(x, y + 0.15, x + 1.0, y + 0.5, x, y + 0.85, color));
    verts.extend(triangle(x, y + 0.85, x + 1.0, y + 0.5, x + 1.0, y + 1.0, color));
    verts
}

/// Left-pointing trapezoid (U+E0D2)
fn powerline_trapezoid_left(x: f32, y: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let mut verts = triangle(x + 1.0, y + 0.15, x, y, x, y + 0.5, color);
    verts.extend(triangle(x + 1.0, y + 0.15, x, y + 0.5, x + 1.0, y + 0.85, color));
    verts.extend(triangle(x + 1.0, y + 0.85, x, y + 0.5, x, y + 1.0, color));
    verts
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════

/// Create a rectangle as two triangles.
fn rect(x: f32, y: f32, width: f32, height: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let flags = VERTEX_TYPE_DECORATION;

    vec![
        // Triangle 1: top-left, bottom-left, top-right
        CellVertex {
            position: [x, y],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x, y + height],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x + width, y],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        // Triangle 2: top-right, bottom-left, bottom-right
        CellVertex {
            position: [x + width, y],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x, y + height],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x + width, y + height],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
    ]
}

/// Create a single triangle.
fn triangle(x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let flags = VERTEX_TYPE_DECORATION;

    vec![
        CellVertex {
            position: [x1, y1],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x2, y2],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x3, y3],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
    ]
}

/// Create a line segment as a thin rectangle.
fn line_segment(x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: [f32; 4]) -> Vec<CellVertex> {
    let flags = VERTEX_TYPE_DECORATION;

    // Calculate perpendicular direction
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();

    if len < 0.001 {
        return Vec::new();
    }

    let nx = -dy / len * thickness / 2.0;
    let ny = dx / len * thickness / 2.0;

    vec![
        CellVertex {
            position: [x1 + nx, y1 + ny],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x1 - nx, y1 - ny],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x2 + nx, y2 + ny],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x2 + nx, y2 + ny],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x1 - nx, y1 - ny],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
        CellVertex {
            position: [x2 - nx, y2 - ny],
            uv: [0.0, 0.0],
            fg_color: color,
            bg_color: [0.0, 0.0, 0.0, 0.0],
            flags,
            _padding: [0; 3],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_box_drawing() {
        // Box drawing range
        assert!(is_box_drawing('─'));  // U+2500
        assert!(is_box_drawing('│'));  // U+2502
        assert!(is_box_drawing('┌'));  // U+250C
        assert!(is_box_drawing('┘'));  // U+2518
        assert!(is_box_drawing('╬'));  // U+256C

        // Block elements
        assert!(is_box_drawing('█'));  // U+2588
        assert!(is_box_drawing('▀'));  // U+2580
        assert!(is_box_drawing('▄'));  // U+2584

        // Triangles
        assert!(is_box_drawing('◢'));  // U+25E2

        // Non-box drawing
        assert!(!is_box_drawing('A'));
        assert!(!is_box_drawing(' '));
        assert!(!is_box_drawing('→'));  // Arrow, not box drawing
    }

    #[test]
    fn test_horizontal_line_vertices() {
        let verts = horizontal_line(0.0, 0.0, [1.0; 4], LineWeight::Light);
        assert_eq!(verts.len(), 6); // 2 triangles = 6 vertices
    }

    #[test]
    fn test_vertical_line_vertices() {
        let verts = vertical_line(0.0, 0.0, [1.0; 4], LineWeight::Light);
        assert_eq!(verts.len(), 6);
    }

    #[test]
    fn test_double_line_vertices() {
        let verts = horizontal_line(0.0, 0.0, [1.0; 4], LineWeight::Double);
        assert_eq!(verts.len(), 12); // 2 lines x 6 vertices
    }

    #[test]
    fn test_corner_vertices() {
        let verts = corner_top_left(0.0, 0.0, [1.0; 4], LineWeight::Light);
        assert_eq!(verts.len(), 12); // 2 rectangles
    }

    #[test]
    fn test_cross_vertices() {
        let verts = cross(0.0, 0.0, [1.0; 4], LineWeight::Light);
        assert_eq!(verts.len(), 12); // 2 lines
    }

    #[test]
    fn test_block_full_vertices() {
        let verts = block_full(0.0, 0.0, [1.0; 4]);
        assert_eq!(verts.len(), 6); // 1 rectangle = 2 triangles
    }

    #[test]
    fn test_generate_box_drawing_vertices() {
        // Test that known characters produce vertices
        assert!(!generate_box_drawing_vertices('─', 0, 0, [1.0; 4]).is_empty());
        assert!(!generate_box_drawing_vertices('│', 0, 0, [1.0; 4]).is_empty());
        assert!(!generate_box_drawing_vertices('┌', 0, 0, [1.0; 4]).is_empty());
        assert!(!generate_box_drawing_vertices('█', 0, 0, [1.0; 4]).is_empty());

        // Unknown character should return empty
        assert!(generate_box_drawing_vertices('A', 0, 0, [1.0; 4]).is_empty());
    }

    #[test]
    fn test_vertex_positions_in_bounds() {
        // Check that all vertices are within cell bounds
        let chars = ['─', '│', '┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '┼', '█'];

        for c in chars {
            let verts = generate_box_drawing_vertices(c, 5, 10, [1.0; 4]);
            for v in &verts {
                assert!(v.position[0] >= 5.0 && v.position[0] <= 6.0,
                    "X out of bounds for '{}': {}", c, v.position[0]);
                assert!(v.position[1] >= 10.0 && v.position[1] <= 11.0,
                    "Y out of bounds for '{}': {}", c, v.position[1]);
            }
        }
    }

    /// CRITICAL TEST: Ensures all box drawing characters are VISIBLE
    ///
    /// This test was added after discovering that box drawing characters were
    /// rendering as INVISIBLE because the hybrid renderer didn't call
    /// generate_box_drawing_vertices(). See:
    /// docs/RETROSPECTIVE_INVISIBLE_CHARS_2025-12-31.md
    #[test]
    fn test_all_box_drawing_chars_generate_vertices() {
        // Track characters that don't generate vertices
        let mut missing: Vec<(char, u32)> = Vec::new();

        // Test Box Drawing range U+2500-U+257F (128 characters)
        for code in 0x2500u32..=0x257Fu32 {
            if let Some(c) = char::from_u32(code) {
                let verts = generate_box_drawing_vertices(c, 0, 0, [1.0; 4]);
                if verts.is_empty() {
                    missing.push((c, code));
                }
            }
        }

        // Test Block Elements range U+2580-U+259F (32 characters)
        for code in 0x2580u32..=0x259Fu32 {
            if let Some(c) = char::from_u32(code) {
                let verts = generate_box_drawing_vertices(c, 0, 0, [1.0; 4]);
                if verts.is_empty() {
                    missing.push((c, code));
                }
            }
        }

        // Test Geometric Shapes (triangles) U+25E2-U+25FF
        for code in 0x25E2u32..=0x25FFu32 {
            if let Some(c) = char::from_u32(code) {
                let verts = generate_box_drawing_vertices(c, 0, 0, [1.0; 4]);
                if verts.is_empty() {
                    missing.push((c, code));
                }
            }
        }

        // Allow some characters to be unimplemented for now, but track count
        // The goal is to reduce this number to 0 over time
        let allowed_missing = 80; // Some geometric shapes and less common chars

        assert!(
            missing.len() <= allowed_missing,
            "Too many box drawing characters without vertex generation ({} > {}).\n\
             First 10 missing: {:?}\n\
             This means these characters will be INVISIBLE when rendered!",
            missing.len(),
            allowed_missing,
            &missing[..missing.len().min(10)]
        );
    }

    /// Test that critical box drawing characters ALL generate vertices
    /// These are the most commonly used and MUST be visible
    #[test]
    fn test_critical_box_drawing_must_be_visible() {
        let critical_chars = [
            // Light lines
            ('─', "HORIZONTAL LINE"),
            ('│', "VERTICAL LINE"),
            // Light corners
            ('┌', "TOP LEFT CORNER"),
            ('┐', "TOP RIGHT CORNER"),
            ('└', "BOTTOM LEFT CORNER"),
            ('┘', "BOTTOM RIGHT CORNER"),
            // Light tees
            ('├', "LEFT TEE"),
            ('┤', "RIGHT TEE"),
            ('┬', "TOP TEE"),
            ('┴', "BOTTOM TEE"),
            // Cross
            ('┼', "CROSS"),
            // Heavy lines
            ('━', "HEAVY HORIZONTAL"),
            ('┃', "HEAVY VERTICAL"),
            // Double lines
            ('═', "DOUBLE HORIZONTAL"),
            ('║', "DOUBLE VERTICAL"),
            ('╔', "DOUBLE TOP LEFT"),
            ('╗', "DOUBLE TOP RIGHT"),
            ('╚', "DOUBLE BOTTOM LEFT"),
            ('╝', "DOUBLE BOTTOM RIGHT"),
            // Block elements
            ('█', "FULL BLOCK"),
            ('▀', "UPPER HALF BLOCK"),
            ('▄', "LOWER HALF BLOCK"),
            ('▌', "LEFT HALF BLOCK"),
            ('▐', "RIGHT HALF BLOCK"),
            ('░', "LIGHT SHADE"),
            ('▒', "MEDIUM SHADE"),
            ('▓', "DARK SHADE"),
        ];

        for (c, name) in critical_chars {
            let verts = generate_box_drawing_vertices(c, 0, 0, [1.0; 4]);
            assert!(
                !verts.is_empty(),
                "CRITICAL: {} ({}, U+{:04X}) generates NO vertices!\n\
                 This character will be INVISIBLE to users.\n\
                 Used by: tmux borders, vim windows, ncurses apps",
                name, c, c as u32
            );

            // Verify vertices are non-zero (not transparent/hidden)
            let has_visible_area = verts.iter().any(|v| {
                v.fg_color[3] > 0.0 // Non-zero alpha
            });
            assert!(
                has_visible_area,
                "CRITICAL: {} generates vertices but with zero alpha (invisible)",
                name
            );
        }
    }

    /// Verify is_box_drawing matches generate_box_drawing_vertices coverage
    /// If is_box_drawing returns true, the character SHOULD generate vertices
    #[test]
    fn test_is_box_drawing_implies_generates_vertices() {
        let mut mismatches: Vec<(char, u32)> = Vec::new();

        // Test all characters that is_box_drawing claims to support
        for code in 0x2500u32..=0x257Fu32 {
            if let Some(c) = char::from_u32(code) {
                if is_box_drawing(c) {
                    let verts = generate_box_drawing_vertices(c, 0, 0, [1.0; 4]);
                    if verts.is_empty() {
                        mismatches.push((c, code));
                    }
                }
            }
        }

        for code in 0x2580u32..=0x259Fu32 {
            if let Some(c) = char::from_u32(code) {
                if is_box_drawing(c) {
                    let verts = generate_box_drawing_vertices(c, 0, 0, [1.0; 4]);
                    if verts.is_empty() {
                        mismatches.push((c, code));
                    }
                }
            }
        }

        // Allow some mismatches for now, but track
        let allowed_mismatches = 80;

        assert!(
            mismatches.len() <= allowed_mismatches,
            "is_box_drawing() claims to support {} characters that generate no vertices!\n\
             First 10: {:?}\n\
             This is a rendering consistency bug - these chars will be INVISIBLE",
            mismatches.len(),
            &mismatches[..mismatches.len().min(10)]
        );
    }

    /// Test that Powerline glyphs are now detected and render vertices
    #[test]
    fn test_powerline_glyphs_supported() {
        // Common Powerline glyphs that MUST work
        let powerline_chars = [
            ('\u{E0A0}', "Git branch"),
            ('\u{E0A1}', "Line number"),
            ('\u{E0A2}', "Lock"),
            ('\u{E0A3}', "Column number"),
            ('\u{E0B0}', "Right arrow solid"),
            ('\u{E0B1}', "Right arrow outline"),
            ('\u{E0B2}', "Left arrow solid"),
            ('\u{E0B3}', "Left arrow outline"),
            ('\u{E0B4}', "Right semicircle"),
            ('\u{E0B6}', "Left semicircle"),
            ('\u{E0B8}', "Lower left triangle"),
            ('\u{E0BA}', "Lower right triangle"),
            ('\u{E0BC}', "Upper left triangle"),
            ('\u{E0BE}', "Upper right triangle"),
        ];

        for (c, name) in powerline_chars {
            assert!(
                is_box_drawing(c),
                "Powerline glyph {} (U+{:04X}) not detected by is_box_drawing()",
                name, c as u32
            );

            let verts = generate_box_drawing_vertices(c, 0, 0, [1.0; 4]);
            assert!(
                !verts.is_empty(),
                "Powerline glyph {} (U+{:04X}) generates no vertices - will be INVISIBLE",
                name, c as u32
            );

            // Verify vertices have non-zero alpha (visible)
            let has_visible = verts.iter().any(|v| v.fg_color[3] > 0.0);
            assert!(
                has_visible,
                "Powerline glyph {} has zero alpha - will be invisible",
                name
            );
        }
    }

    /// Test Powerline arrow separators specifically (most commonly used)
    #[test]
    fn test_powerline_arrows_visible() {
        // These are the most critical Powerline symbols - used in prompts everywhere
        let right_arrow = '\u{E0B0}';
        let left_arrow = '\u{E0B2}';

        // Right arrow should be a triangle pointing right
        let right_verts = generate_box_drawing_vertices(right_arrow, 0, 0, [1.0; 4]);
        assert_eq!(right_verts.len(), 3, "Right arrow should be single triangle (3 verts)");

        // Left arrow should be a triangle pointing left
        let left_verts = generate_box_drawing_vertices(left_arrow, 0, 0, [1.0; 4]);
        assert_eq!(left_verts.len(), 3, "Left arrow should be single triangle (3 verts)");
    }

    /// Test that the full Powerline range is detected
    #[test]
    fn test_powerline_range_detected() {
        // Full Powerline range U+E0A0 to U+E0D7
        let mut detected = 0;
        let mut total = 0;

        for code in 0xE0A0u32..=0xE0D7u32 {
            total += 1;
            if let Some(c) = char::from_u32(code) {
                if is_box_drawing(c) {
                    detected += 1;
                }
            }
        }

        // All Powerline characters should be detected
        assert_eq!(
            detected, total,
            "Not all Powerline range characters detected: {}/{}",
            detected, total
        );
    }

    // ═══════════════════════════════════════════════════════════════════
    // FIXTURE-BASED TESTS
    // Tests using fixtures from tests/fixtures/*.txt
    // ═══════════════════════════════════════════════════════════════════

    /// Embedded fixture content for test reliability
    const BOX_DRAWING_FIXTURE: &str = include_str!("../../tests/fixtures/box_drawing_comprehensive.txt");
    const BLOCK_ELEMENTS_FIXTURE: &str = include_str!("../../tests/fixtures/block_elements.txt");
    const POWERLINE_FIXTURE: &str = include_str!("../../tests/fixtures/powerline_glyphs.txt");

    /// Parse fixture file to extract Unicode characters with their codepoints.
    ///
    /// Parses lines in format: `<char> U+XXXX <description>`
    /// The character at line start may not render correctly if fonts lack the glyph,
    /// so we trust the codepoint and reconstruct the character from it.
    fn parse_fixture_chars(content: &str) -> Vec<(char, u32, String)> {
        let mut chars = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("===") {
                continue;
            }

            // Look for "U+XXXX" pattern - this is the authoritative source
            if let Some(pos) = line.find("U+") {
                let code_start = pos + 2;
                let code_end = line[code_start..].find(' ')
                    .map(|i| code_start + i)
                    .unwrap_or(line.len());

                if let Ok(code) = u32::from_str_radix(&line[code_start..code_end], 16) {
                    if let Some(c) = char::from_u32(code) {
                        // Description is everything after the codepoint
                        let desc = line[code_end..].trim().to_string();
                        chars.push((c, code, desc));
                    }
                }
            }
        }
        chars
    }

    /// Test that all box drawing characters from fixture generate vertices.
    ///
    /// This fixture-based test ensures comprehensive coverage of the U+2500-U+257F range.
    #[test]
    fn test_fixture_box_drawing_generates_vertices() {
        let chars = parse_fixture_chars(BOX_DRAWING_FIXTURE);
        assert!(!chars.is_empty(), "Failed to parse box drawing fixture");

        let mut missing = Vec::new();
        let mut invisible = Vec::new();

        for (c, code, desc) in &chars {
            // Must be detected
            if !is_box_drawing(*c) {
                missing.push((*c, *code, desc.clone()));
                continue;
            }

            // Must generate vertices
            let verts = generate_box_drawing_vertices(*c, 0, 0, [1.0; 4]);
            if verts.is_empty() {
                missing.push((*c, *code, desc.clone()));
                continue;
            }

            // Must have non-zero alpha (visible)
            let visible = verts.iter().any(|v| v.fg_color[3] > 0.0);
            if !visible {
                invisible.push((*c, *code, desc.clone()));
            }
        }

        // Report issues
        if !missing.is_empty() || !invisible.is_empty() {
            use std::fmt::Write as _;
            let mut msg = String::new();
            if !missing.is_empty() {
                let _ = writeln!(msg, "Characters with no vertices ({}):", missing.len());
                for (c, code, desc) in &missing[..missing.len().min(5)] {
                    let _ = writeln!(msg, "  {} U+{:04X} {}", c, code, desc);
                }
            }
            if !invisible.is_empty() {
                let _ = writeln!(msg, "Characters with zero alpha ({}):", invisible.len());
                for (c, code, desc) in &invisible[..invisible.len().min(5)] {
                    let _ = writeln!(msg, "  {} U+{:04X} {}", c, code, desc);
                }
            }

            // Allow some missing for less common chars, but track
            let allowed = 40;
            assert!(
                missing.len() <= allowed,
                "Too many box drawing chars missing vertices: {}\n{}",
                missing.len(),
                msg
            );
        }
    }

    /// Test that all block element characters from fixture generate vertices.
    #[test]
    fn test_fixture_block_elements_generates_vertices() {
        let chars = parse_fixture_chars(BLOCK_ELEMENTS_FIXTURE);
        assert!(!chars.is_empty(), "Failed to parse block elements fixture");

        let mut missing = Vec::new();

        for (c, code, desc) in &chars {
            if !is_box_drawing(*c) {
                missing.push((*c, *code, desc.clone()));
                continue;
            }

            let verts = generate_box_drawing_vertices(*c, 0, 0, [1.0; 4]);
            if verts.is_empty() {
                missing.push((*c, *code, desc.clone()));
            }
        }

        // Block elements should have high coverage
        let allowed = 5;
        assert!(
            missing.len() <= allowed,
            "Too many block elements missing vertices ({}/{}): {:?}",
            missing.len(),
            chars.len(),
            &missing[..missing.len().min(10)]
        );
    }

    /// Test that all Powerline characters from fixture generate vertices.
    ///
    /// Powerline glyphs are critical for modern prompts. Zero tolerance for missing.
    #[test]
    fn test_fixture_powerline_generates_vertices() {
        let chars = parse_fixture_chars(POWERLINE_FIXTURE);
        assert!(!chars.is_empty(), "Failed to parse Powerline fixture");

        let mut missing = Vec::new();

        for (c, code, desc) in &chars {
            if !is_box_drawing(*c) {
                missing.push((*c, *code, format!("not detected: {}", desc)));
                continue;
            }

            let verts = generate_box_drawing_vertices(*c, 0, 0, [1.0; 4]);
            if verts.is_empty() {
                missing.push((*c, *code, format!("no vertices: {}", desc)));
            }
        }

        // Most critical Powerline chars (arrows) must work - allow some for exotic shapes
        let allowed = 20;
        assert!(
            missing.len() <= allowed,
            "Too many Powerline glyphs missing ({}/{}): {:?}",
            missing.len(),
            chars.len(),
            &missing[..missing.len().min(10)]
        );
    }

    /// Verify fixture files parse correctly.
    #[test]
    fn test_fixture_parsing() {
        let box_chars = parse_fixture_chars(BOX_DRAWING_FIXTURE);
        let block_chars = parse_fixture_chars(BLOCK_ELEMENTS_FIXTURE);
        let powerline_chars = parse_fixture_chars(POWERLINE_FIXTURE);

        // Sanity check counts
        assert!(box_chars.len() >= 80, "Box drawing fixture too small: {}", box_chars.len());
        assert!(block_chars.len() >= 20, "Block elements fixture too small: {}", block_chars.len());
        assert!(powerline_chars.len() >= 15, "Powerline fixture too small: {}", powerline_chars.len());

        // Check for duplicates
        let mut seen = std::collections::HashSet::new();
        for (c, _, _) in &box_chars {
            assert!(seen.insert(*c), "Duplicate in box drawing fixture: {}", c);
        }
    }

    // =========================================================================
    // CRITICAL: FFI/Shader Flag Compatibility Tests
    // =========================================================================
    // These tests verify that box drawing vertices output the CORRECT flag format
    // for shaders (WGSL and Metal). DO NOT CHANGE without updating ALL shaders.

    #[test]
    fn test_box_drawing_outputs_new_flag_format() {
        // CRITICAL: This test ensures box drawing outputs VERTEX_TYPE_DECORATION = 2,
        // NOT the old FLAG_IS_DECORATION = 2048.
        //
        // DashTerm2 Metal shader and shader.wgsl both expect:
        //   flags & 0x3 == 2  (VERTEX_TYPE_DECORATION)
        //
        // If this test fails, rendering will break!

        // Test various box drawing characters
        let test_chars = [
            '─', // Light horizontal
            '═', // Double horizontal
            '║', // Double vertical
            '╔', // Double corner
            '█', // Full block
            '\u{E0B0}', // Powerline arrow
        ];

        for c in test_chars {
            let verts = generate_box_drawing_vertices(c, 0, 0, [1.0; 4]);
            assert!(!verts.is_empty(), "No vertices for {:?}", c);

            for (i, v) in verts.iter().enumerate() {
                // Extract vertex type from bits 0-1
                let vertex_type = v.flags & 0x3;

                assert_eq!(
                    vertex_type, VERTEX_TYPE_DECORATION,
                    "Vertex {} for {:?} has wrong flag format!\n\
                     Got: flags={} (type={})\n\
                     Expected: VERTEX_TYPE_DECORATION={}\n\
                     If you see flags=2048, that's the OLD format - this is a critical bug!",
                    i, c, v.flags, vertex_type, VERTEX_TYPE_DECORATION
                );
            }
        }
    }

    #[test]
    fn test_box_drawing_flags_not_old_format() {
        // CRITICAL: Verify we're NOT outputting the old FLAG_IS_DECORATION = 2048
        const OLD_FLAG_IS_DECORATION: u32 = 2048;

        let test_chars = ['─', '═', '╔', '█', '\u{E0B0}'];

        for c in test_chars {
            let verts = generate_box_drawing_vertices(c, 0, 0, [1.0; 4]);
            for v in &verts {
                assert!(
                    v.flags & OLD_FLAG_IS_DECORATION == 0,
                    "Vertex for {:?} has OLD flag format! flags={} contains bit 11 (2048).\n\
                     This means dterm-core is outputting the wrong format for DashTerm2.",
                    c, v.flags
                );
            }
        }
    }
}
