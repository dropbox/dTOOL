/*
 * DTermCoreTests.swift - Tests for DTermCore
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 */

import XCTest
@testable import DTermCore

final class DTermCoreTests: XCTestCase {

    // MARK: - Cell Flags Tests

    func testCellFlagsRawValue() {
        XCTAssertEqual(CellFlags.bold.rawValue, 1)
        XCTAssertEqual(CellFlags.italic.rawValue, 2)
        XCTAssertEqual(CellFlags.underline.rawValue, 4)
    }

    func testCellFlagsOptionSet() {
        let flags: CellFlags = [.bold, .italic]
        XCTAssertTrue(flags.contains(.bold))
        XCTAssertTrue(flags.contains(.italic))
        XCTAssertFalse(flags.contains(.underline))
    }

    // MARK: - RGB Tests

    func testRGBInit() {
        let color = DTermRGB(red: 255, green: 128, blue: 0)
        XCTAssertEqual(color.red, 255)
        XCTAssertEqual(color.green, 128)
        XCTAssertEqual(color.blue, 0)
    }

    func testRGBEquality() {
        let color1 = DTermRGB(red: 255, green: 128, blue: 0)
        let color2 = DTermRGB(red: 255, green: 128, blue: 0)
        let color3 = DTermRGB(red: 0, green: 0, blue: 0)

        XCTAssertEqual(color1, color2)
        XCTAssertNotEqual(color1, color3)
    }

    // MARK: - Mode Enum Tests

    func testMouseModeValues() {
        XCTAssertEqual(MouseMode.none.rawValue, 0)
        XCTAssertEqual(MouseMode.normal.rawValue, 1)
        XCTAssertEqual(MouseMode.buttonEvent.rawValue, 2)
        XCTAssertEqual(MouseMode.anyEvent.rawValue, 3)
    }

    func testMouseEncodingValues() {
        XCTAssertEqual(MouseEncoding.x10.rawValue, 0)
        XCTAssertEqual(MouseEncoding.sgr.rawValue, 1)
    }

    func testCursorStyleValues() {
        XCTAssertEqual(CursorStyle.default.rawValue, 0)
        XCTAssertEqual(CursorStyle.blinkingBlock.rawValue, 1)
        XCTAssertEqual(CursorStyle.steadyBlock.rawValue, 2)
        XCTAssertEqual(CursorStyle.blinkingUnderline.rawValue, 3)
        XCTAssertEqual(CursorStyle.steadyUnderline.rawValue, 4)
        XCTAssertEqual(CursorStyle.blinkingBar.rawValue, 5)
        XCTAssertEqual(CursorStyle.steadyBar.rawValue, 6)
    }

    func testShellStateValues() {
        XCTAssertEqual(ShellState.ground.rawValue, 0)
        XCTAssertEqual(ShellState.receivingPrompt.rawValue, 1)
        XCTAssertEqual(ShellState.enteringCommand.rawValue, 2)
        XCTAssertEqual(ShellState.executing.rawValue, 3)
    }

    func testLineSizeValues() {
        XCTAssertEqual(LineSize.singleWidth.rawValue, 0)
        XCTAssertEqual(LineSize.doubleWidth.rawValue, 1)
        XCTAssertEqual(LineSize.doubleHeightTop.rawValue, 2)
        XCTAssertEqual(LineSize.doubleHeightBottom.rawValue, 3)
    }

    // MARK: - Terminal Tests (require linked library)

    // These tests require the dterm-core library to be linked.
    // They are disabled by default since the library might not be available.

    #if DTERM_LINKED
    func testTerminalCreation() {
        let terminal = DTermTerminal(rows: 24, cols: 80)
        XCTAssertEqual(terminal.rows, 24)
        XCTAssertEqual(terminal.cols, 80)
        XCTAssertEqual(terminal.cursorRow, 0)
        XCTAssertEqual(terminal.cursorCol, 0)
    }

    func testTerminalResize() {
        let terminal = DTermTerminal(rows: 24, cols: 80)
        terminal.resize(rows: 40, cols: 120)
        XCTAssertEqual(terminal.rows, 40)
        XCTAssertEqual(terminal.cols, 120)
    }

    func testTerminalProcessASCII() {
        let terminal = DTermTerminal(rows: 24, cols: 80)
        terminal.process(data: Data("Hello, World!".utf8))

        // Check cursor moved
        XCTAssertEqual(terminal.cursorCol, 13)
        XCTAssertEqual(terminal.cursorRow, 0)

        // Check cells contain the text
        let cell0 = terminal.getCell(row: 0, col: 0)
        XCTAssertNotNil(cell0)
        XCTAssertEqual(cell0?.character, "H")

        let cell6 = terminal.getCell(row: 0, col: 6)
        XCTAssertEqual(cell6?.character, " ")
    }

    func testTerminalProcessNewline() {
        let terminal = DTermTerminal(rows: 24, cols: 80)
        terminal.process(data: Data("Line1\nLine2".utf8))

        XCTAssertEqual(terminal.cursorRow, 1)
        XCTAssertEqual(terminal.cursorCol, 5)
    }

    func testTerminalAlternateScreen() {
        let terminal = DTermTerminal(rows: 24, cols: 80)
        XCTAssertFalse(terminal.isAlternateScreen)

        // Enter alternate screen (DECSET 1049)
        terminal.process(data: Data("\u{1b}[?1049h".utf8))
        XCTAssertTrue(terminal.isAlternateScreen)

        // Exit alternate screen
        terminal.process(data: Data("\u{1b}[?1049l".utf8))
        XCTAssertFalse(terminal.isAlternateScreen)
    }

    func testTerminalTitle() {
        let terminal = DTermTerminal(rows: 24, cols: 80)
        XCTAssertNil(terminal.title)

        // Set title via OSC 0
        terminal.process(data: Data("\u{1b}]0;Test Title\u{07}".utf8))
        XCTAssertEqual(terminal.title, "Test Title")
    }

    private final class PaletteChangeDelegate: DTermTerminalDelegate {
        var indices: [Int] = []

        func terminalColorDidChange(_ terminal: DTermTerminal, index: Int?) {
            guard let index = index else { return }
            indices.append(index)
        }
    }

    func testTerminalPaletteChangeCallback() {
        let terminal = DTermTerminal(rows: 24, cols: 80)
        let delegate = PaletteChangeDelegate()
        terminal.delegate = delegate

        terminal.setPaletteColor(index: 1, color: DTermRGB(red: 1, green: 2, blue: 3))

        XCTAssertTrue(delegate.indices.contains(1))
    }

    func testTerminalReset() {
        let terminal = DTermTerminal(rows: 24, cols: 80)
        terminal.process(data: Data("Hello".utf8))
        XCTAssertEqual(terminal.cursorCol, 5)

        terminal.reset()
        XCTAssertEqual(terminal.cursorCol, 0)
    }

    func testTerminalCursorVisibility() {
        let terminal = DTermTerminal(rows: 24, cols: 80)
        XCTAssertTrue(terminal.cursorVisible)

        // Hide cursor (DECTCEM)
        terminal.process(data: Data("\u{1b}[?25l".utf8))
        XCTAssertFalse(terminal.cursorVisible)

        // Show cursor
        terminal.process(data: Data("\u{1b}[?25h".utf8))
        XCTAssertTrue(terminal.cursorVisible)
    }

    func testTerminalScrollback() {
        let terminal = DTermTerminal(rows: 24, cols: 80)

        // Generate some scrollback by outputting many lines
        for i in 0..<100 {
            terminal.process(data: Data("Line \(i)\n".utf8))
        }

        XCTAssertGreaterThan(terminal.scrollbackLines, 0)
        XCTAssertGreaterThan(terminal.totalLines, 24)
    }

    func testTerminalDamageTracking() {
        let terminal = DTermTerminal(rows: 24, cols: 80)
        terminal.clearDamage()

        terminal.process(data: Data("X".utf8))
        XCTAssertTrue(terminal.rowIsDamaged(0))

        terminal.clearDamage()
        XCTAssertFalse(terminal.rowIsDamaged(0))
    }

    // MARK: - Search Tests

    func testSearchCreation() {
        let search = DTermSearch()
        XCTAssertEqual(search.lineCount, 0)
        XCTAssertTrue(search.isEmpty)
    }

    func testSearchWithCapacity() {
        let search = DTermSearch(expectedLines: 1000)
        XCTAssertEqual(search.lineCount, 0)
        XCTAssertTrue(search.isEmpty)
    }

    func testSearchIndexLine() {
        let search = DTermSearch()
        search.indexLine("hello world")
        XCTAssertEqual(search.lineCount, 1)
        XCTAssertFalse(search.isEmpty)

        search.indexLine("goodbye world")
        XCTAssertEqual(search.lineCount, 2)
    }

    func testSearchIndexLines() {
        let search = DTermSearch()
        search.indexLines(["line 1", "line 2", "line 3"])
        XCTAssertEqual(search.lineCount, 3)
    }

    func testSearchMightContain() {
        let search = DTermSearch()
        search.indexLine("hello world")

        XCTAssertTrue(search.mightContain("hello"))
        XCTAssertTrue(search.mightContain("world"))
        // Bloom filter may return true for non-existent strings (false positive)
        // but should be unlikely for random strings
    }

    func testSearchFind() {
        let search = DTermSearch()
        search.indexLine("hello world")
        search.indexLine("goodbye world")
        search.indexLine("hello there")

        let worldMatches = search.find("world")
        XCTAssertEqual(worldMatches.count, 2)

        let helloMatches = search.find("hello")
        XCTAssertEqual(helloMatches.count, 2)

        let noMatches = search.find("xyz123")
        XCTAssertEqual(noMatches.count, 0)
    }

    func testSearchFindPositions() {
        let search = DTermSearch()
        search.indexLine("hello hello")

        let matches = search.find("hello")
        XCTAssertEqual(matches.count, 2)

        // First match at position 0
        XCTAssertEqual(matches[0].line, 0)
        XCTAssertEqual(matches[0].startCol, 0)
        XCTAssertEqual(matches[0].endCol, 5)

        // Second match at position 6
        XCTAssertEqual(matches[1].line, 0)
        XCTAssertEqual(matches[1].startCol, 6)
        XCTAssertEqual(matches[1].endCol, 11)
    }

    func testSearchFindOrdered() {
        let search = DTermSearch()
        search.indexLine("test line 0")
        search.indexLine("test line 1")
        search.indexLine("test line 2")

        let forward = search.findOrdered("test", direction: .forward)
        XCTAssertEqual(forward.count, 3)
        XCTAssertEqual(forward[0].line, 0)
        XCTAssertEqual(forward[1].line, 1)
        XCTAssertEqual(forward[2].line, 2)

        let backward = search.findOrdered("test", direction: .backward)
        XCTAssertEqual(backward.count, 3)
        XCTAssertEqual(backward[0].line, 2)
        XCTAssertEqual(backward[1].line, 1)
        XCTAssertEqual(backward[2].line, 0)
    }

    func testSearchFindNext() {
        let search = DTermSearch()
        search.indexLine("match here")
        search.indexLine("no hit")
        search.indexLine("match again")

        // Find next from line 0, col 0 (after "match" at 0,0)
        let next = search.findNext("match", afterLine: 0, afterCol: 5)
        XCTAssertNotNil(next)
        XCTAssertEqual(next?.line, 2)
    }

    func testSearchFindPrev() {
        let search = DTermSearch()
        search.indexLine("match here")
        search.indexLine("no hit")
        search.indexLine("match again")

        // Find prev from line 2
        let prev = search.findPrev("match", beforeLine: 2, beforeCol: 0)
        XCTAssertNotNil(prev)
        XCTAssertEqual(prev?.line, 0)
    }

    func testSearchClear() {
        let search = DTermSearch()
        search.indexLine("test")
        XCTAssertFalse(search.isEmpty)

        search.clear()
        XCTAssertTrue(search.isEmpty)
        XCTAssertEqual(search.lineCount, 0)
    }

    func testSearchMatchProperties() {
        let match = DTermSearchMatch(line: 5, startCol: 10, endCol: 15)
        XCTAssertEqual(match.line, 5)
        XCTAssertEqual(match.startCol, 10)
        XCTAssertEqual(match.endCol, 15)
        XCTAssertEqual(match.length, 5)
        XCTAssertFalse(match.isEmpty)
        XCTAssertTrue(match.containsColumn(12))
        XCTAssertFalse(match.containsColumn(9))
        XCTAssertFalse(match.containsColumn(15))
    }

    func testSearchMatchEquality() {
        let match1 = DTermSearchMatch(line: 0, startCol: 5, endCol: 10)
        let match2 = DTermSearchMatch(line: 0, startCol: 5, endCol: 10)
        let match3 = DTermSearchMatch(line: 1, startCol: 5, endCol: 10)

        XCTAssertEqual(match1, match2)
        XCTAssertNotEqual(match1, match3)
    }

    func testSearchEmptyQuery() {
        let search = DTermSearch()
        search.indexLine("test content")

        // Empty query should return empty results (not infinite loop)
        let matches = search.find("")
        XCTAssertTrue(matches.isEmpty)
    }
    #endif
}
