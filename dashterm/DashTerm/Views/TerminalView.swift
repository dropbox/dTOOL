//
//  TerminalView.swift
//  DashTerm
//
//  Terminal emulator view using AppKit + Core Text for rendering
//

import SwiftUI
import AppKit
import Combine

/// SwiftUI wrapper for the AppKit terminal view
struct TerminalView: NSViewRepresentable {
    @ObservedObject var session: TerminalSession

    func makeNSView(context: Context) -> TerminalNSView {
        let view = TerminalNSView(session: session)
        return view
    }

    func updateNSView(_ nsView: TerminalNSView, context: Context) {
        // Only update session reference if changed
        if nsView.session !== session {
            nsView.session = session
            nsView.subscribeToUpdates()
        }
    }
}

/// AppKit view for terminal rendering using Core Text
class TerminalNSView: NSView {
    var session: TerminalSession {
        didSet {
            if oldValue !== session {
                subscribeToUpdates()
            }
        }
    }

    // Font settings
    private var font: NSFont
    private var boldFont: NSFont
    private var cellWidth: CGFloat = 0
    private var cellHeight: CGFloat = 0
    private var fontAscent: CGFloat = 0

    // Colors
    private let defaultForeground = NSColor.white
    private let defaultBackground = NSColor.black
    private let cursorColor = NSColor(white: 0.8, alpha: 1.0)

    // ANSI color palette (pre-computed CGColors for speed)
    private var ansiCGColors: [CGColor] = []
    private let ansiColors: [NSColor] = [
        NSColor(red: 0.0, green: 0.0, blue: 0.0, alpha: 1.0),      // Black
        NSColor(red: 0.8, green: 0.0, blue: 0.0, alpha: 1.0),      // Red
        NSColor(red: 0.0, green: 0.8, blue: 0.0, alpha: 1.0),      // Green
        NSColor(red: 0.8, green: 0.8, blue: 0.0, alpha: 1.0),      // Yellow
        NSColor(red: 0.0, green: 0.0, blue: 0.8, alpha: 1.0),      // Blue
        NSColor(red: 0.8, green: 0.0, blue: 0.8, alpha: 1.0),      // Magenta
        NSColor(red: 0.0, green: 0.8, blue: 0.8, alpha: 1.0),      // Cyan
        NSColor(red: 0.8, green: 0.8, blue: 0.8, alpha: 1.0),      // White
        // Bright variants
        NSColor(red: 0.4, green: 0.4, blue: 0.4, alpha: 1.0),      // Bright Black
        NSColor(red: 1.0, green: 0.0, blue: 0.0, alpha: 1.0),      // Bright Red
        NSColor(red: 0.0, green: 1.0, blue: 0.0, alpha: 1.0),      // Bright Green
        NSColor(red: 1.0, green: 1.0, blue: 0.0, alpha: 1.0),      // Bright Yellow
        NSColor(red: 0.0, green: 0.0, blue: 1.0, alpha: 1.0),      // Bright Blue
        NSColor(red: 1.0, green: 0.0, blue: 1.0, alpha: 1.0),      // Bright Magenta
        NSColor(red: 0.0, green: 1.0, blue: 1.0, alpha: 1.0),      // Bright Cyan
        NSColor(red: 1.0, green: 1.0, blue: 1.0, alpha: 1.0),      // Bright White
    ]

    // Cached grid data
    private var cachedGrid: [[TerminalCell]] = []
    private var cachedCursor: CursorPosition = CursorPosition(row: 0, col: 0, visible: true)
    private var gridDirty = true
    private var pendingDamage: [DamageRegion] = []

    // Cursor blink state
    private var cursorVisible = true
    private var displayLink: CVDisplayLink?

    // Subscriptions
    private var cancellables = Set<AnyCancellable>()
    private var lastBlinkTime: CFTimeInterval = 0

    init(session: TerminalSession) {
        self.session = session
        self.font = NSFont.monospacedSystemFont(ofSize: 14, weight: .regular)
        self.boldFont = NSFont.monospacedSystemFont(ofSize: 14, weight: .bold)

        super.init(frame: .zero)

        // Pre-compute CGColors
        ansiCGColors = ansiColors.map { $0.cgColor }

        calculateFontMetrics()
        setupDisplayLink()
        subscribeToUpdates()

        // Accept first responder for keyboard input
        self.wantsLayer = true
        self.layer?.backgroundColor = defaultBackground.cgColor
        self.layerContentsRedrawPolicy = .onSetNeedsDisplay
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        if let displayLink = displayLink {
            CVDisplayLinkStop(displayLink)
        }
        cancellables.removeAll()
    }

    func subscribeToUpdates() {
        cancellables.removeAll()
        session.updatePublisher
            .receive(on: DispatchQueue.main)
            .sink { [weak self] in
                self?.handleDamageUpdate()
            }
            .store(in: &cancellables)
    }

    private func handleDamageUpdate() {
        // Get damage regions from session
        let damage = session.getDamage()

        if damage.isEmpty {
            // No damage, nothing to redraw
            return
        }

        // Store damage for use in draw()
        pendingDamage = damage
        gridDirty = true

        // Invalidate only damaged regions
        for region in damage {
            let rect = rectForDamageRegion(region)
            setNeedsDisplay(rect)
        }

        // Reset damage tracking in the terminal
        session.resetDamage()
    }

    private func rectForDamageRegion(_ region: DamageRegion) -> NSRect {
        let x = CGFloat(region.left) * cellWidth
        let y = bounds.height - CGFloat(region.line + 1) * cellHeight
        let width = CGFloat(region.right - region.left) * cellWidth
        return NSRect(x: x, y: y, width: width, height: cellHeight)
    }

    private func calculateFontMetrics() {
        let attrs: [NSAttributedString.Key: Any] = [.font: font]
        let size = ("M" as NSString).size(withAttributes: attrs)
        cellWidth = ceil(size.width)
        cellHeight = ceil(font.ascender - font.descender + font.leading)
        fontAscent = font.ascender
    }

    private func setupDisplayLink() {
        var displayLinkRef: CVDisplayLink?
        CVDisplayLinkCreateWithActiveCGDisplays(&displayLinkRef)

        guard let displayLink = displayLinkRef else { return }
        self.displayLink = displayLink

        let callback: CVDisplayLinkOutputCallback = { _, _, _, _, _, userInfo -> CVReturn in
            let view = Unmanaged<TerminalNSView>.fromOpaque(userInfo!).takeUnretainedValue()
            view.handleDisplayLink()
            return kCVReturnSuccess
        }

        CVDisplayLinkSetOutputCallback(displayLink, callback, Unmanaged.passUnretained(self).toOpaque())
        CVDisplayLinkStart(displayLink)
    }

    private func handleDisplayLink() {
        let now = CACurrentMediaTime()
        // Blink cursor every 0.5 seconds
        if now - lastBlinkTime >= 0.5 {
            lastBlinkTime = now
            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                self.cursorVisible.toggle()
                // Only redraw cursor area, not entire view
                let cursor = self.cachedCursor
                let cursorRect = CGRect(
                    x: CGFloat(cursor.col) * self.cellWidth,
                    y: self.bounds.height - CGFloat(cursor.row + 1) * self.cellHeight,
                    width: self.cellWidth,
                    height: self.cellHeight
                )
                self.setNeedsDisplay(cursorRect)
            }
        }
    }

    override var acceptsFirstResponder: Bool { true }

    override func becomeFirstResponder() -> Bool {
        true
    }

    override func keyDown(with event: NSEvent) {
        // Handle special keys first
        let keyCode = event.keyCode
        let modifiers = event.modifierFlags

        // Handle scroll keys with Shift modifier
        if modifiers.contains(.shift) {
            switch keyCode {
            case 116: // Page Up
                session.scrollUp(session.terminalSize.rows)
                gridDirty = true
                needsDisplay = true
                return
            case 121: // Page Down
                session.scrollDown(session.terminalSize.rows)
                gridDirty = true
                needsDisplay = true
                return
            case 115: // Home
                session.scrollToTop()
                gridDirty = true
                needsDisplay = true
                return
            case 119: // End
                session.scrollToBottom()
                gridDirty = true
                needsDisplay = true
                return
            default:
                break
            }
        }

        // Arrow keys and other special keys
        switch keyCode {
        case 123: // Left arrow
            session.write("\u{1b}[D")
            return
        case 124: // Right arrow
            session.write("\u{1b}[C")
            return
        case 125: // Down arrow
            session.write("\u{1b}[B")
            return
        case 126: // Up arrow
            session.write("\u{1b}[A")
            return
        case 36: // Return
            session.write("\r")
            return
        case 51: // Delete/Backspace
            session.write("\u{7f}")
            return
        case 53: // Escape
            session.write("\u{1b}")
            return
        case 48: // Tab
            session.write("\t")
            return
        default:
            break
        }

        guard let chars = event.characters else { return }

        // Handle control keys
        if modifiers.contains(.control) {
            if let scalar = chars.unicodeScalars.first {
                let controlChar = Character(UnicodeScalar(scalar.value & 0x1f)!)
                session.write(String(controlChar))
                return
            }
        }

        // Handle regular input
        session.write(chars)
    }

    override func scrollWheel(with event: NSEvent) {
        let deltaY = event.scrollingDeltaY

        // Determine scroll amount based on whether this is a precise scroll (trackpad) or not (mouse wheel)
        let lines: Int
        if event.hasPreciseScrollingDeltas {
            // Trackpad: convert pixel delta to lines (approx 1 line = cellHeight pixels)
            lines = max(1, Int(abs(deltaY) / cellHeight))
        } else {
            // Mouse wheel: each "click" is about 3 lines
            lines = Int(abs(deltaY) * 3)
        }

        if deltaY > 0 {
            // Scroll up (show older content)
            session.scrollUp(lines)
        } else if deltaY < 0 {
            // Scroll down (show newer content)
            session.scrollDown(lines)
        }

        if lines > 0 {
            gridDirty = true
            needsDisplay = true
        }
    }

    override func flagsChanged(with event: NSEvent) {
        // Handle modifier key changes if needed
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let context = NSGraphicsContext.current?.cgContext else { return }

        // Update cached grid if dirty
        if gridDirty {
            cachedGrid = session.getGrid()
            cachedCursor = session.getCursor()
            gridDirty = false
        }

        // Clear background (only dirty rect)
        context.setFillColor(defaultBackground.cgColor)
        context.fill(dirtyRect)

        // Calculate visible rows based on dirty rect
        let startRow = max(0, Int((bounds.height - dirtyRect.maxY) / cellHeight))
        let endRow = min(cachedGrid.count, Int((bounds.height - dirtyRect.minY) / cellHeight) + 1)
        let startCol = max(0, Int(dirtyRect.minX / cellWidth))
        let endCol = min(cachedGrid.first?.count ?? 0, Int(dirtyRect.maxX / cellWidth) + 1)

        // Don't show cursor when scrolled (it's off-screen)
        let isScrolled = session.isScrolled

        // Draw only visible cells
        for rowIndex in startRow..<endRow {
            guard rowIndex < cachedGrid.count else { continue }
            let row = cachedGrid[rowIndex]

            for colIndex in startCol..<endCol {
                guard colIndex < row.count else { continue }
                let cell = row[colIndex]
                let isCursor = !isScrolled && rowIndex == cachedCursor.row && colIndex == cachedCursor.col && cursorVisible && cachedCursor.visible

                drawCell(
                    cell,
                    row: rowIndex,
                    col: colIndex,
                    context: context,
                    isCursor: isCursor
                )
            }
        }

        // Draw scroll indicator when scrolled
        if isScrolled {
            drawScrollIndicator(context: context)
        }
    }

    private func drawScrollIndicator(context: CGContext) {
        // Draw a subtle indicator at the bottom showing we're scrolled
        let indicatorHeight: CGFloat = 3
        let indicatorRect = CGRect(x: 0, y: 0, width: bounds.width, height: indicatorHeight)

        // Blue indicator bar
        context.setFillColor(CGColor(red: 0.3, green: 0.5, blue: 1.0, alpha: 0.8))
        context.fill(indicatorRect)
    }

    private func drawCell(_ cell: TerminalCell, row: Int, col: Int, context: CGContext, isCursor: Bool) {
        let x = CGFloat(col) * cellWidth
        let y = bounds.height - CGFloat(row + 1) * cellHeight
        let rect = CGRect(x: x, y: y, width: cellWidth, height: cellHeight)

        // Draw background
        let bgColor = isCursor ? cursorColor.cgColor : cgColorForAttribute(cell.background, isBackground: true)
        context.setFillColor(bgColor)
        context.fill(rect)

        // Draw text
        if !cell.content.isEmpty && cell.content != " " {
            let fgColor = isCursor ? defaultBackground : colorForAttribute(cell.foreground, isBackground: false)
            let fontToUse = cell.bold ? boldFont : font

            let attrs: [NSAttributedString.Key: Any] = [
                .font: fontToUse,
                .foregroundColor: fgColor
            ]

            let attrString = NSAttributedString(string: cell.content, attributes: attrs)
            let line = CTLineCreateWithAttributedString(attrString)

            context.textPosition = CGPoint(x: x, y: y + (cellHeight - fontAscent) / 2)
            CTLineDraw(line, context)
        }

        // Draw underline if needed
        if cell.underline {
            context.setStrokeColor(cgColorForAttribute(cell.foreground, isBackground: false))
            context.setLineWidth(1)
            context.move(to: CGPoint(x: x, y: y + 1))
            context.addLine(to: CGPoint(x: x + cellWidth, y: y + 1))
            context.strokePath()
        }
    }

    private func cgColorForAttribute(_ attr: ColorAttribute, isBackground: Bool) -> CGColor {
        switch attr {
        case .default:
            return isBackground ? defaultBackground.cgColor : defaultForeground.cgColor
        case .named(let index):
            return Int(index) < ansiCGColors.count ? ansiCGColors[Int(index)] : (isBackground ? defaultBackground.cgColor : defaultForeground.cgColor)
        case .indexed(let index):
            return color256(Int(index)).cgColor
        case .rgb(let r, let g, let b):
            return CGColor(red: CGFloat(r) / 255, green: CGFloat(g) / 255, blue: CGFloat(b) / 255, alpha: 1)
        }
    }

    private func colorForAttribute(_ attr: ColorAttribute, isBackground: Bool) -> NSColor {
        switch attr {
        case .default:
            return isBackground ? defaultBackground : defaultForeground
        case .named(let index):
            return Int(index) < ansiColors.count ? ansiColors[Int(index)] : (isBackground ? defaultBackground : defaultForeground)
        case .indexed(let index):
            return color256(Int(index))
        case .rgb(let r, let g, let b):
            return NSColor(red: CGFloat(r) / 255, green: CGFloat(g) / 255, blue: CGFloat(b) / 255, alpha: 1)
        }
    }

    private func color256(_ index: Int) -> NSColor {
        if index < 16 {
            return ansiColors[index]
        } else if index < 232 {
            // 6x6x6 color cube
            let n = index - 16
            let r = (n / 36) % 6
            let g = (n / 6) % 6
            let b = n % 6
            return NSColor(
                red: CGFloat(r) / 5,
                green: CGFloat(g) / 5,
                blue: CGFloat(b) / 5,
                alpha: 1
            )
        } else {
            // Grayscale
            let gray = CGFloat(index - 232) / 23
            return NSColor(white: gray, alpha: 1)
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        window?.makeFirstResponder(self)
    }

    override var intrinsicContentSize: NSSize {
        let cols = session.terminalSize.cols
        let rows = session.terminalSize.rows
        return NSSize(
            width: CGFloat(cols) * cellWidth,
            height: CGFloat(rows) * cellHeight
        )
    }
}
