// DashTermCoreTests.swift
// DashTerm2Tests
//
// Tests for the DashTermCore Swift wrapper around the Rust FFI.

import XCTest
@testable import DashTerm2SharedARC

final class DashTermCoreTests: XCTestCase {

    // MARK: - Basic Creation

    func test_DashTermCore_creation_default() {
        let core = DashTermCore()
        XCTAssertEqual(core.rows, 24, "Default rows should be 24")
        XCTAssertEqual(core.cols, 80, "Default cols should be 80")
    }

    func test_DashTermCore_creation_custom() {
        let core = DashTermCore(rows: 40, cols: 120, scrollback: 5000)
        XCTAssertEqual(core.rows, 40)
        XCTAssertEqual(core.cols, 120)
    }

    // MARK: - Input/Output

    func test_DashTermCore_write_string() {
        let core = DashTermCore()
        core.write("Hello, World!")

        let text = core.visibleText
        XCTAssertTrue(text.contains("Hello, World!"), "Written text should appear in visible content")
    }

    func test_DashTermCore_write_data() {
        let core = DashTermCore()
        let data = "Test Data".data(using: .utf8)!
        core.write(data)

        let text = core.visibleText
        XCTAssertTrue(text.contains("Test Data"), "Written data should appear in visible content")
    }

    func test_DashTermCore_write_escapeSequence() {
        let core = DashTermCore()
        // Write with color escape sequence (red text)
        core.write("\u{001B}[31mRed Text\u{001B}[0m")

        let text = core.visibleText
        XCTAssertTrue(text.contains("Red Text"), "Text should appear even with escape sequences")
    }

    // MARK: - Cursor

    func test_DashTermCore_cursor_initialPosition() {
        let core = DashTermCore()
        let (row, col) = core.cursorPosition
        XCTAssertEqual(row, 0, "Initial cursor row should be 0")
        XCTAssertEqual(col, 0, "Initial cursor col should be 0")
    }

    func test_DashTermCore_cursor_afterWrite() {
        let core = DashTermCore()
        core.write("ABC")

        let (row, col) = core.cursorPosition
        XCTAssertEqual(row, 0, "Cursor should still be on row 0")
        XCTAssertEqual(col, 3, "Cursor should advance to column 3 after 'ABC'")
    }

    func test_DashTermCore_cursor_afterNewline() {
        let core = DashTermCore()
        core.write("Line1\nLine2")

        let (row, col) = core.cursorPosition
        XCTAssertEqual(row, 1, "Cursor should be on row 1 after newline")
        XCTAssertEqual(col, 5, "Cursor should be at column 5 after 'Line2'")
    }

    // MARK: - Line Access

    func test_DashTermCore_line_basic() {
        let core = DashTermCore()
        core.write("First Line\nSecond Line")

        let line0 = core.line(at: 0)
        let line1 = core.line(at: 1)

        XCTAssertNotNil(line0)
        XCTAssertNotNil(line1)
        XCTAssertTrue(line0?.contains("First Line") ?? false)
        XCTAssertTrue(line1?.contains("Second Line") ?? false)
    }

    func test_DashTermCore_line_outOfBounds() {
        let core = DashTermCore()
        let line = core.line(at: 100)
        // Should return nil or empty for out of bounds
        XCTAssertTrue(line == nil || line?.isEmpty == true)
    }

    // MARK: - Resize

    func test_DashTermCore_resize() {
        let core = DashTermCore()
        XCTAssertEqual(core.rows, 24)
        XCTAssertEqual(core.cols, 80)

        core.resize(rows: 50, cols: 132)

        XCTAssertEqual(core.rows, 50)
        XCTAssertEqual(core.cols, 132)
    }

    // MARK: - Title

    func test_DashTermCore_title_escapeSequence() {
        let core = DashTermCore()
        // OSC 0 - Set title
        core.write("\u{001B}]0;My Terminal Title\u{0007}")

        XCTAssertEqual(core.title, "My Terminal Title")
    }

    // MARK: - AI Agent API

    func test_DashTermCore_snapshot() {
        let core = DashTermCore()
        core.write("Hello")

        let snapshot = core.snapshot
        XCTAssertNotNil(snapshot)
        XCTAssertEqual(snapshot?.rows, 24)
        XCTAssertEqual(snapshot?.cols, 80)
        XCTAssertTrue(snapshot?.visibleContent.contains("Hello") ?? false)
    }

    func test_DashTermCore_commandHistory_empty() {
        let core = DashTermCore()
        let history = core.commandHistory()
        // Empty terminal should have no command history
        XCTAssertTrue(history.isEmpty)
    }

    // MARK: - Version

    func test_DashTermCore_version() {
        let version = DashTermCore.version
        XCTAssertFalse(version.isEmpty, "Version should not be empty")
        XCTAssertNotEqual(version, "unknown", "Version should be known")
    }

    // MARK: - Memory Management

    func test_DashTermCore_multipleInstances() {
        // Create multiple instances to test memory management
        var instances: [DashTermCore] = []
        for i in 0..<10 {
            let core = DashTermCore()
            core.write("Instance \(i)")
            instances.append(core)
        }

        // Verify each instance is independent
        for (i, core) in instances.enumerated() {
            let text = core.visibleText
            XCTAssertTrue(text.contains("Instance \(i)"))
        }

        // Let them deallocate
        instances.removeAll()
        // If we get here without crash, memory management works
        XCTAssertTrue(true)
    }

    func test_DashTermCore_manualFree() {
        let core = DashTermCore()
        core.write("Before free")

        // Manual free
        core.freeTerminal()

        // After free, operations should be no-ops (not crash)
        core.write("After free")
        XCTAssertEqual(core.rows, 0)
        XCTAssertEqual(core.cols, 0)
    }

    // MARK: - Performance

    func test_DashTermCore_performanceWrite() {
        let core = DashTermCore()
        let testData = String(repeating: "X", count: 1000)

        measure {
            for _ in 0..<100 {
                core.write(testData)
            }
        }
    }

    func test_DashTermCore_performanceSnapshot() {
        let core = DashTermCore()
        // Fill with data
        for i in 0..<100 {
            core.write("Line \(i): " + String(repeating: "X", count: 70) + "\n")
        }

        measure {
            for _ in 0..<1000 {
                _ = core.snapshot
            }
        }
    }
}
