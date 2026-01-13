//! Build structured fuzzing corpus from vttest and terminal sequences.
//!
//! This script generates a seed corpus for the parser fuzzer based on:
//! - vttest conformance sequences
//! - Real-world terminal output patterns
//! - Edge cases from terminal specifications
//!
//! ## Usage
//!
//! ```bash
//! cd crates/dterm-core/fuzz
//! cargo run --release --bin build_corpus
//! ```
//!
//! ## Gap 21: Comparative Fuzzing Corpus
//!
//! This addresses Gap 21 from DTERM_CORE_GAPS.md by providing a structured
//! corpus that complements random fuzzing with known-interesting sequences.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::Path;

fn main() {
    let corpus_dir = Path::new("corpus/parser");
    let diff_corpus_dir = Path::new("corpus/parser_diff");

    // Ensure directories exist
    fs::create_dir_all(corpus_dir).expect("Failed to create parser corpus dir");
    fs::create_dir_all(diff_corpus_dir).expect("Failed to create parser_diff corpus dir");

    let mut count = 0;

    // Write sequences to both corpus directories
    let write_seq = |name: &str, data: &[u8], count: &mut usize| {
        let hash = {
            let mut h = DefaultHasher::new();
            data.hash(&mut h);
            format!("{:016x}", h.finish())
        };

        let filename = format!("vttest_{:03}_{}", count, hash);

        for dir in &[corpus_dir, diff_corpus_dir] {
            let path = dir.join(&filename);
            let mut f = fs::File::create(&path).expect("Failed to create corpus file");
            f.write_all(data).expect("Failed to write corpus file");
        }

        println!("  [{}] {} ({} bytes)", count, name, data.len());
        *count += 1;
    };

    println!("Building vttest-based fuzzing corpus...\n");

    // ========================================================================
    // VTTEST Menu 1: Cursor Movements
    // ========================================================================
    println!("Category 1: Cursor Movements");

    // CUP - Cursor Position
    write_seq("CUP home", b"\x1b[H", &mut count);
    write_seq("CUP explicit home", b"\x1b[1;1H", &mut count);
    write_seq("CUP row 10 col 20", b"\x1b[10;20H", &mut count);
    write_seq("CUP large values", b"\x1b[999;999H", &mut count);
    write_seq("CUP zero params", b"\x1b[0;0H", &mut count);
    write_seq("CUP omitted params", b"\x1b[;H", &mut count);

    // CUU - Cursor Up
    write_seq("CUU default", b"\x1b[A", &mut count);
    write_seq("CUU explicit 1", b"\x1b[1A", &mut count);
    write_seq("CUU move 10", b"\x1b[10A", &mut count);

    // CUD - Cursor Down
    write_seq("CUD default", b"\x1b[B", &mut count);
    write_seq("CUD explicit 1", b"\x1b[1B", &mut count);
    write_seq("CUD move 10", b"\x1b[10B", &mut count);

    // CUF - Cursor Forward
    write_seq("CUF default", b"\x1b[C", &mut count);
    write_seq("CUF move 10", b"\x1b[10C", &mut count);

    // CUB - Cursor Backward
    write_seq("CUB default", b"\x1b[D", &mut count);
    write_seq("CUB move 10", b"\x1b[10D", &mut count);

    // CNL - Cursor Next Line
    write_seq("CNL default", b"\x1b[E", &mut count);
    write_seq("CNL move 5", b"\x1b[5E", &mut count);

    // CPL - Cursor Previous Line
    write_seq("CPL default", b"\x1b[F", &mut count);
    write_seq("CPL move 5", b"\x1b[5F", &mut count);

    // CHA - Cursor Horizontal Absolute
    write_seq("CHA default", b"\x1b[G", &mut count);
    write_seq("CHA col 40", b"\x1b[40G", &mut count);

    // VPA - Vertical Position Absolute
    write_seq("VPA row 10", b"\x1b[10d", &mut count);

    // HPA - Horizontal Position Absolute
    write_seq("HPA col 30", b"\x1b[30`", &mut count);

    // HPR - Horizontal Position Relative
    write_seq("HPR move 5", b"\x1b[5a", &mut count);

    // VPR - Vertical Position Relative
    write_seq("VPR move 3", b"\x1b[3e", &mut count);

    // Index and Reverse Index
    write_seq("IND (ESC D)", b"\x1bD", &mut count);
    write_seq("RI (ESC M)", b"\x1bM", &mut count);
    write_seq("NEL (ESC E)", b"\x1bE", &mut count);

    // ========================================================================
    // VTTEST Menu 2: Screen Features
    // ========================================================================
    println!("\nCategory 2: Screen Features");

    // DECSTBM - Set Top and Bottom Margins
    write_seq("DECSTBM reset", b"\x1b[r", &mut count);
    write_seq("DECSTBM 5-15", b"\x1b[5;15r", &mut count);
    write_seq("DECSTBM 1-24", b"\x1b[1;24r", &mut count);

    // ED - Erase in Display
    write_seq("ED to end", b"\x1b[J", &mut count);
    write_seq("ED to end explicit", b"\x1b[0J", &mut count);
    write_seq("ED to start", b"\x1b[1J", &mut count);
    write_seq("ED entire", b"\x1b[2J", &mut count);
    write_seq("ED scrollback", b"\x1b[3J", &mut count);

    // EL - Erase in Line
    write_seq("EL to end", b"\x1b[K", &mut count);
    write_seq("EL to end explicit", b"\x1b[0K", &mut count);
    write_seq("EL to start", b"\x1b[1K", &mut count);
    write_seq("EL entire", b"\x1b[2K", &mut count);

    // Scroll Up/Down
    write_seq("SU scroll up", b"\x1b[S", &mut count);
    write_seq("SU scroll up 5", b"\x1b[5S", &mut count);
    write_seq("SD scroll down", b"\x1b[T", &mut count);
    write_seq("SD scroll down 5", b"\x1b[5T", &mut count);

    // DECALN - Screen Alignment Pattern
    write_seq("DECALN fill E", b"\x1b#8", &mut count);

    // Mode sets
    write_seq("DECAWM enable", b"\x1b[?7h", &mut count);
    write_seq("DECAWM disable", b"\x1b[?7l", &mut count);
    write_seq("DECOM enable", b"\x1b[?6h", &mut count);
    write_seq("DECOM disable", b"\x1b[?6l", &mut count);
    write_seq("IRM enable", b"\x1b[4h", &mut count);
    write_seq("IRM disable", b"\x1b[4l", &mut count);

    // ========================================================================
    // VTTEST Menu 3: Character Sets
    // ========================================================================
    println!("\nCategory 3: Character Sets");

    // G0/G1/G2/G3 designation
    write_seq("G0 ASCII", b"\x1b(B", &mut count);
    write_seq("G0 DEC Special", b"\x1b(0", &mut count);
    write_seq("G0 UK", b"\x1b(A", &mut count);
    write_seq("G1 ASCII", b"\x1b)B", &mut count);
    write_seq("G1 DEC Special", b"\x1b)0", &mut count);
    write_seq("G2 designate", b"\x1b*0", &mut count);
    write_seq("G3 designate", b"\x1b+0", &mut count);

    // Locking shifts
    write_seq("LS0 (SI)", &[0x0F], &mut count);
    write_seq("LS1 (SO)", &[0x0E], &mut count);
    write_seq("LS2", b"\x1bn", &mut count);
    write_seq("LS3", b"\x1bo", &mut count);

    // Single shifts
    write_seq("SS2 (ESC N)", b"\x1bN", &mut count);
    write_seq("SS3 (ESC O)", b"\x1bO", &mut count);

    // Box drawing in DEC Special Graphics
    let box_seq = b"\x1b(0lqqqk\r\nx   x\r\nmqqqj\x1b(B";
    write_seq("box drawing", box_seq, &mut count);

    // ========================================================================
    // VTTEST Menu 6: Terminal Reports
    // ========================================================================
    println!("\nCategory 6: Terminal Reports");

    // DA - Device Attributes
    write_seq("DA primary", b"\x1b[c", &mut count);
    write_seq("DA explicit", b"\x1b[0c", &mut count);
    write_seq("DA2 secondary", b"\x1b[>c", &mut count);
    write_seq("DA3 tertiary", b"\x1b[=c", &mut count);

    // DSR - Device Status Report
    write_seq("DSR status", b"\x1b[5n", &mut count);
    write_seq("DSR cursor pos", b"\x1b[6n", &mut count);
    write_seq("DSR extended pos", b"\x1b[?6n", &mut count);

    // DECRQSS - Request Selection or Setting
    write_seq("DECRQSS SGR", b"\x1bP$qm\x1b\\", &mut count);
    write_seq("DECRQSS DECSTBM", b"\x1bP$qr\x1b\\", &mut count);
    write_seq("DECRQSS DECSCUSR", b"\x1bP$q q\x1b\\", &mut count);

    // ========================================================================
    // VTTEST Menu 8: VT102 Features
    // ========================================================================
    println!("\nCategory 8: VT102 Features");

    // ICH - Insert Character
    write_seq("ICH default", b"\x1b[@", &mut count);
    write_seq("ICH insert 5", b"\x1b[5@", &mut count);

    // DCH - Delete Character
    write_seq("DCH default", b"\x1b[P", &mut count);
    write_seq("DCH delete 5", b"\x1b[5P", &mut count);

    // IL - Insert Line
    write_seq("IL default", b"\x1b[L", &mut count);
    write_seq("IL insert 3", b"\x1b[3L", &mut count);

    // DL - Delete Line
    write_seq("DL default", b"\x1b[M", &mut count);
    write_seq("DL delete 3", b"\x1b[3M", &mut count);

    // ECH - Erase Character
    write_seq("ECH default", b"\x1b[X", &mut count);
    write_seq("ECH erase 5", b"\x1b[5X", &mut count);

    // ========================================================================
    // SGR - Select Graphic Rendition
    // ========================================================================
    println!("\nCategory: SGR (Text Attributes)");

    write_seq("SGR reset", b"\x1b[0m", &mut count);
    write_seq("SGR bold", b"\x1b[1m", &mut count);
    write_seq("SGR dim", b"\x1b[2m", &mut count);
    write_seq("SGR italic", b"\x1b[3m", &mut count);
    write_seq("SGR underline", b"\x1b[4m", &mut count);
    write_seq("SGR blink", b"\x1b[5m", &mut count);
    write_seq("SGR rapid blink", b"\x1b[6m", &mut count);
    write_seq("SGR inverse", b"\x1b[7m", &mut count);
    write_seq("SGR hidden", b"\x1b[8m", &mut count);
    write_seq("SGR strikethrough", b"\x1b[9m", &mut count);

    // Basic colors
    write_seq("SGR fg black", b"\x1b[30m", &mut count);
    write_seq("SGR fg red", b"\x1b[31m", &mut count);
    write_seq("SGR fg green", b"\x1b[32m", &mut count);
    write_seq("SGR fg yellow", b"\x1b[33m", &mut count);
    write_seq("SGR fg blue", b"\x1b[34m", &mut count);
    write_seq("SGR fg magenta", b"\x1b[35m", &mut count);
    write_seq("SGR fg cyan", b"\x1b[36m", &mut count);
    write_seq("SGR fg white", b"\x1b[37m", &mut count);
    write_seq("SGR fg default", b"\x1b[39m", &mut count);

    write_seq("SGR bg black", b"\x1b[40m", &mut count);
    write_seq("SGR bg red", b"\x1b[41m", &mut count);
    write_seq("SGR bg default", b"\x1b[49m", &mut count);

    // 256-color mode
    write_seq("SGR 256 fg", b"\x1b[38;5;196m", &mut count);
    write_seq("SGR 256 bg", b"\x1b[48;5;21m", &mut count);

    // 24-bit true color
    write_seq("SGR RGB fg", b"\x1b[38;2;255;128;0m", &mut count);
    write_seq("SGR RGB bg", b"\x1b[48;2;0;128;255m", &mut count);

    // Combined attributes
    write_seq("SGR bold red", b"\x1b[1;31m", &mut count);
    write_seq("SGR combined", b"\x1b[1;4;31;44m", &mut count);

    // ========================================================================
    // Private Modes (DECSET/DECRST)
    // ========================================================================
    println!("\nCategory: Private Modes");

    // Cursor
    write_seq("DECTCEM show cursor", b"\x1b[?25h", &mut count);
    write_seq("DECTCEM hide cursor", b"\x1b[?25l", &mut count);
    write_seq("DECSCUSR block", b"\x1b[2 q", &mut count);
    write_seq("DECSCUSR underline", b"\x1b[4 q", &mut count);
    write_seq("DECSCUSR bar", b"\x1b[6 q", &mut count);

    // Application mode
    write_seq("DECCKM app cursor", b"\x1b[?1h", &mut count);
    write_seq("DECCKM normal cursor", b"\x1b[?1l", &mut count);

    // Alternate screen
    write_seq("Alt screen enable", b"\x1b[?1049h", &mut count);
    write_seq("Alt screen disable", b"\x1b[?1049l", &mut count);

    // Bracketed paste
    write_seq("Bracketed paste on", b"\x1b[?2004h", &mut count);
    write_seq("Bracketed paste off", b"\x1b[?2004l", &mut count);

    // Mouse modes
    write_seq("Mouse X10", b"\x1b[?9h", &mut count);
    write_seq("Mouse normal", b"\x1b[?1000h", &mut count);
    write_seq("Mouse button", b"\x1b[?1002h", &mut count);
    write_seq("Mouse any", b"\x1b[?1003h", &mut count);
    write_seq("Mouse SGR", b"\x1b[?1006h", &mut count);
    write_seq("Mouse off", b"\x1b[?1000l", &mut count);

    // Focus reporting
    write_seq("Focus on", b"\x1b[?1004h", &mut count);
    write_seq("Focus off", b"\x1b[?1004l", &mut count);

    // Synchronized output
    write_seq("Sync on", b"\x1b[?2026h", &mut count);
    write_seq("Sync off", b"\x1b[?2026l", &mut count);

    // ========================================================================
    // OSC - Operating System Commands
    // ========================================================================
    println!("\nCategory: OSC");

    write_seq("OSC title BEL", b"\x1b]0;Window Title\x07", &mut count);
    write_seq("OSC title ST", b"\x1b]0;Window Title\x1b\\", &mut count);
    write_seq("OSC icon title", b"\x1b]1;Icon Title\x07", &mut count);
    write_seq("OSC window title", b"\x1b]2;Window Title\x07", &mut count);
    write_seq("OSC CWD", b"\x1b]7;file:///Users/test/dir\x07", &mut count);

    // Hyperlinks
    write_seq(
        "OSC hyperlink",
        b"\x1b]8;id=link1;https://example.com\x07Link\x1b]8;;\x07",
        &mut count,
    );

    // OSC 133 shell integration
    write_seq("OSC 133 A", b"\x1b]133;A\x07", &mut count);
    write_seq("OSC 133 B", b"\x1b]133;B\x07", &mut count);
    write_seq("OSC 133 C", b"\x1b]133;C\x07", &mut count);
    write_seq("OSC 133 D;0", b"\x1b]133;D;0\x07", &mut count);

    // Color queries/sets
    write_seq("OSC 4 query", b"\x1b]4;1;?\x07", &mut count);
    write_seq("OSC 10 query fg", b"\x1b]10;?\x07", &mut count);
    write_seq("OSC 11 query bg", b"\x1b]11;?\x07", &mut count);

    // Clipboard (OSC 52)
    write_seq("OSC 52 copy", b"\x1b]52;c;SGVsbG8=\x07", &mut count);
    write_seq("OSC 52 query", b"\x1b]52;c;?\x07", &mut count);

    // ========================================================================
    // DCS - Device Control Strings
    // ========================================================================
    println!("\nCategory: DCS");

    // DECRQSS
    write_seq("DCS DECRQSS m", b"\x1bP$qm\x1b\\", &mut count);
    write_seq("DCS DECRQSS r", b"\x1bP$qr\x1b\\", &mut count);

    // Sixel (minimal)
    write_seq("DCS sixel intro", b"\x1bPq\x1b\\", &mut count);
    write_seq("DCS sixel data", b"\x1bPq#0;2;0;0;0~-\x1b\\", &mut count);

    // XTGETTCAP
    write_seq("DCS XTGETTCAP", b"\x1bP+q544e\x1b\\", &mut count);

    // ========================================================================
    // C1 Control Codes (8-bit)
    // ========================================================================
    println!("\nCategory: C1 Control Codes");

    write_seq("C1 IND", &[0x84], &mut count);
    write_seq("C1 NEL", &[0x85], &mut count);
    write_seq("C1 HTS", &[0x88], &mut count);
    write_seq("C1 RI", &[0x8D], &mut count);
    write_seq("C1 SS2", &[0x8E], &mut count);
    write_seq("C1 SS3", &[0x8F], &mut count);
    write_seq("C1 DCS", &[0x90, b'q', 0x9C], &mut count);
    write_seq("C1 CSI", &[0x9B, b'H'], &mut count);
    write_seq("C1 CSI full", &[0x9B, b'1', b'0', b';', b'2', b'0', b'H'], &mut count);
    write_seq("C1 ST", &[0x9C], &mut count);
    write_seq("C1 OSC", &[0x9D, b'0', b';', b'T', b'i', b't', b'l', b'e', 0x9C], &mut count);

    // ========================================================================
    // Tabs
    // ========================================================================
    println!("\nCategory: Tab Handling");

    write_seq("HT tab", &[0x09], &mut count);
    write_seq("HTS set tab", b"\x1bH", &mut count);
    write_seq("TBC clear tab", b"\x1b[0g", &mut count);
    write_seq("TBC clear all", b"\x1b[3g", &mut count);
    write_seq("CHT forward tabs", b"\x1b[2I", &mut count);
    write_seq("CBT backward tab", b"\x1b[Z", &mut count);

    // ========================================================================
    // Reset Sequences
    // ========================================================================
    println!("\nCategory: Reset");

    write_seq("RIS hard reset", b"\x1bc", &mut count);
    write_seq("DECSTR soft reset", b"\x1b[!p", &mut count);

    // ========================================================================
    // Save/Restore State
    // ========================================================================
    println!("\nCategory: Save/Restore");

    write_seq("DECSC save cursor", b"\x1b7", &mut count);
    write_seq("DECRC restore cursor", b"\x1b8", &mut count);
    write_seq("CSI s save cursor", b"\x1b[s", &mut count);
    write_seq("CSI u restore cursor", b"\x1b[u", &mut count);

    // ========================================================================
    // Window Manipulation (XTWINOPS)
    // ========================================================================
    println!("\nCategory: Window Ops");

    write_seq("XTWINOPS iconify", b"\x1b[2t", &mut count);
    write_seq("XTWINOPS deiconify", b"\x1b[1t", &mut count);
    write_seq("XTWINOPS move", b"\x1b[3;100;100t", &mut count);
    write_seq("XTWINOPS resize px", b"\x1b[4;480;640t", &mut count);
    write_seq("XTWINOPS resize cells", b"\x1b[8;24;80t", &mut count);
    write_seq("XTWINOPS raise", b"\x1b[5t", &mut count);
    write_seq("XTWINOPS lower", b"\x1b[6t", &mut count);
    write_seq("XTWINOPS query state", b"\x1b[11t", &mut count);
    write_seq("XTWINOPS query pos", b"\x1b[13t", &mut count);
    write_seq("XTWINOPS query size px", b"\x1b[14t", &mut count);
    write_seq("XTWINOPS query size ch", b"\x1b[18t", &mut count);
    write_seq("XTWINOPS push title", b"\x1b[22;0t", &mut count);
    write_seq("XTWINOPS pop title", b"\x1b[23;0t", &mut count);

    // ========================================================================
    // REP - Repeat Character
    // ========================================================================
    println!("\nCategory: REP");

    write_seq("REP repeat 5", b"X\x1b[5b", &mut count);
    write_seq("REP repeat 100", b"A\x1b[100b", &mut count);

    // ========================================================================
    // Kitty Keyboard Protocol
    // ========================================================================
    println!("\nCategory: Kitty Keyboard");

    write_seq("Kitty KB push 1", b"\x1b[>1u", &mut count);
    write_seq("Kitty KB push all", b"\x1b[>31u", &mut count);
    write_seq("Kitty KB pop", b"\x1b[<u", &mut count);
    write_seq("Kitty KB query", b"\x1b[?u", &mut count);

    // ========================================================================
    // Real-World Sequences
    // ========================================================================
    println!("\nCategory: Real-World Patterns");

    // Git status colored output
    write_seq(
        "git status",
        b"\x1b[32mmodified:   src/main.rs\x1b[0m\r\n",
        &mut count,
    );

    // ls colored output
    write_seq(
        "ls colored",
        b"\x1b[1;34mdir/\x1b[0m  \x1b[1;32mfile.txt\x1b[0m\r\n",
        &mut count,
    );

    // Prompt with OSC 133
    write_seq(
        "shell prompt",
        b"\x1b]133;A\x07\x1b[1;32muser@host\x1b[0m:\x1b[1;34m~/dir\x1b[0m$ \x1b]133;B\x07",
        &mut count,
    );

    // Progress bar
    write_seq(
        "progress bar",
        b"\r\x1b[K[=====>          ] 30%",
        &mut count,
    );

    // ANSI art (simple)
    write_seq(
        "ansi art",
        b"\x1b[31m*\x1b[33m*\x1b[32m*\x1b[36m*\x1b[34m*\x1b[35m*\x1b[0m\r\n",
        &mut count,
    );

    // vim mode line
    write_seq(
        "vim status",
        b"\x1b[1;1H\x1b[7m-- INSERT --\x1b[0m\x1b[K",
        &mut count,
    );

    // tmux status bar
    write_seq(
        "tmux status",
        b"\x1b[24;1H\x1b[42;30m[0] 0:bash*\x1b[0m",
        &mut count,
    );

    // ========================================================================
    // Edge Cases and Malformed Sequences
    // ========================================================================
    println!("\nCategory: Edge Cases");

    // Incomplete sequences
    write_seq("incomplete ESC", b"\x1b", &mut count);
    write_seq("incomplete CSI", b"\x1b[", &mut count);
    write_seq("incomplete CSI params", b"\x1b[1;", &mut count);
    write_seq("incomplete OSC", b"\x1b]", &mut count);
    write_seq("incomplete DCS", b"\x1bP", &mut count);

    // Overlong params
    write_seq("overlong params", b"\x1b[9999999999m", &mut count);
    write_seq("many params", b"\x1b[1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16;17m", &mut count);
    write_seq("many semicolons", b"\x1b[;;;;;;;;;;m", &mut count);

    // CAN/SUB interrupts
    write_seq("CAN interrupt", b"\x1b[1;2\x18H", &mut count);
    write_seq("SUB interrupt", b"\x1b[1;2\x1aH", &mut count);

    // Mixed valid/invalid
    write_seq("mixed seq", b"\x1b[1m\x1b[\x1b[2m", &mut count);

    // UTF-8 in sequences
    write_seq("utf8 title", b"\x1b]0;\xc3\xa9\xc3\xa0\xc3\xb9\x07", &mut count);
    write_seq("utf8 text", b"Hello \xc3\xa9\xc3\xa0\xc3\xb9 World", &mut count);

    // Binary data (to test robustness)
    write_seq("binary mixed", &[0x00, 0x1b, b'[', b'm', 0xFF, 0x7F], &mut count);

    println!("\n================================================");
    println!("Generated {} corpus entries", count);
    println!("Corpus written to:");
    println!("  - corpus/parser/");
    println!("  - corpus/parser_diff/");
}
