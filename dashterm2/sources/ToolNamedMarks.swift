//
//  ToolNamedMarks.swift
//  DashTerm2
//
//  Created by George Nachman on 5/21/23.
//

import AppKit

fileprivate let buttonHeight = 23.0

func makeToolbeltButton(imageName: String?, title: String, target: AnyObject, selector: Selector, frame: NSRect) -> NSButton {
    let button = NSButton(frame: NSRect(x: 0.0, y: frame.size.height - buttonHeight, width: frame.width, height: buttonHeight))
    button.setButtonType(.momentaryPushIn)
    if let imageName {
        if #available(macOS 10.16, *) {
            button.image = NSImage.it_image(forSymbolName: imageName, accessibilityDescription: title)
        } else {
            button.image = NSImage(named: imageName)
        }
    } else {
        button.title = title
    }
    button.target = target
    button.action = selector
    if #available(macOS 10.16, *) {
        button.bezelStyle = .regularSquare
        button.isBordered = false
        button.imageScaling = .scaleProportionallyUpOrDown
        button.imagePosition = .imageOnly
    } else {
        button.bezelStyle = .smallSquare
    }
    button.sizeToFit()
    button.autoresizingMask = [.minYMargin]

    return button
}

@objc
class ToolNamedMarks: NSView, ToolbeltTool, NSTableViewDelegate, NSTableViewDataSource, NSTextFieldDelegate {
    private var scrollView: NSScrollView!
    private var _tableView: NSTableView!
    private var addButton: NSButton!
    private var removeButton: NSButton!
    private var editButton: NSButton!

    private var marks = [iTermGenericNamedMarkReading]()

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)

        scrollView = NSScrollView.scrollViewWithTableViewForToolbelt(container: self,
                                                                     insets: NSEdgeInsets(),
                                                                     rowHeight: NSTableView.heightForTextCell(using: .it_toolbelt()), keyboardNavigable: false)

        guard let tableView = scrollView.documentView as? NSTableView else {
            // BUG-f557: Log error and create a fallback tableView instead of crashing
            DLog("Expected NSTableView as documentView but got \(String(describing: scrollView.documentView))")
            let fallbackTable = NSTableView()
            scrollView.documentView = fallbackTable
            _tableView = fallbackTable
            return
        }
        _tableView = tableView
        _tableView.allowsMultipleSelection = true
        _tableView.perform(#selector(scrollToEndOfDocument(_:)), with: nil, afterDelay: 0)
        _tableView.reloadData()
        _tableView.backgroundColor = .clear

        addButton = makeToolbeltButton(imageName: SFSymbol.plus.rawValue,
                                       title: "Add",
                                       target: self,
                                       selector: #selector(add(_:)),
                                       frame: frameRect)
        addSubview(addButton)
        removeButton = makeToolbeltButton(imageName: SFSymbol.minus.rawValue,
                                          title: "Remove",
                                          target: self,
                                          selector: #selector(remove(_:)),
                                          frame: frameRect)
        addSubview(removeButton)
        editButton = makeToolbeltButton(imageName: SFSymbol.pencil.rawValue,
                                        title: "Edit",
                                        target: self,
                                        selector: #selector(edit(_:)),
                                        frame: frameRect)
        addSubview(editButton)

        relayout()
        updateEnabled()
    }

    static func isDynamic() -> Bool {
        return false
    }
    
    required init!(frame: NSRect, url: URL!, identifier: String!) {
        // BUG-f855: Return nil instead of crashing for unused URL-based initializer
        DLog("ToolNamedMarks URL-based init is not supported")
        return nil
    }

    required init?(coder: NSCoder) {
        // BUG-f825: Return nil instead of crashing for unused coder initializer
        DLog("ToolNamedMarks init(coder:) is not supported")
        return nil
    }

    static var supportedProfileTypes: ProfileType {
        ProfileType(rawValue: ProfileType.terminal.rawValue | ProfileType.browser.rawValue)
    }

    @objc func shutdown() {
        // BUG-1698: Cancel pending delayed operations to prevent crashes after shutdown
        NSObject.cancelPreviousPerformRequests(withTarget: _tableView as Any)
    }

    @objc override func resizeSubviews(withOldSize oldSize: NSSize) {
        super.resizeSubviews(withOldSize: oldSize)
        relayout()
    }

    func updateEnabled() {
        editButton?.isEnabled = _tableView.selectedRowIndexes.count == 1
        removeButton?.isEnabled = !_tableView.selectedRowIndexes.isEmpty
    }

    @objc func relayout() {
        var margin = -1.0
        if #available(macOS 10.16, *) {
            margin = 2
        }
        var x = frame.width
        for button in [ addButton, removeButton, editButton ].compactMap({ $0 }) {
            button.sizeToFit()
            var width = 0.0
            if #available(macOS 10.16, *) {
                width = button.frame.width
            } else {
                width = max(buttonHeight, button.frame.width)
            }
            x -= width + margin
            button.frame = NSRect(x: x, y: frame.height - buttonHeight, width: width, height: buttonHeight)
        }
        let bottomMargin = 4.0
        scrollView.frame = NSRect(x: 0.0, y: 0.0, width: frame.width, height: frame.height - buttonHeight - bottomMargin)
        let contentSize = self.contentSize()
        _tableView.frame = NSRect(origin: .zero, size: contentSize)
    }

    @objc func minimumHeight() -> CGFloat {
        return 60.0
    }

    @objc override var isFlipped: Bool { true }

    @objc(setNamedMarks:)
    func set(marks: [iTermGenericNamedMarkReading]) {
        // For browser sessions, preserve the database ordering which already sorts current page marks first
        // For terminal sessions, sort by namedMarkSort
        if toolWrapper()?.delegate?.delegate?.toolbeltCurrentSessionIsBrowser() == true {
            self.marks = marks
        } else {
            self.marks = marks.sorted(by: { lhs, rhs in
                return (lhs.namedMarkSort) < (rhs.namedMarkSort)
            })
        }
        _tableView.reloadData()
    }

    @objc override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if _tableView.window?.firstResponder === _tableView && event.keyCode == kVK_Delete {
            remove(self)
            return true

        }
        return super.performKeyEquivalent(with: event)
    }

    @objc func contentSize() -> NSSize {
        var size = scrollView.contentSize
        size.height = _tableView.intrinsicContentSize.height
        return size
    }

    func numberOfRows(in tableView: NSTableView) -> Int {
        return marks.count
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        // BUG-f1021: Bounds check before accessing marks array
        // Row index can be stale if marks array was modified between numberOfRows and this call
        guard row >= 0 && row < marks.count else {
            DLog("ToolNamedMarks tableView(_:viewFor:row:): row \(row) out of bounds for count \(marks.count)")
            return nil
        }
        let cell = tableView.newTableCellViewWithTextField(usingIdentifier: "ToolNamedMarks",
                                                           font: NSFont.it_toolbelt(),
                                                           string: marks[row].name ?? "(Unnamed)")
        cell.textField?.isEditable = true
        cell.textField?.delegate = self
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        updateEnabled()
        let row = _tableView.selectedRow
        guard row >= 0 && row < marks.count else {
            return
        }
        // BUG-7199: Use optional chaining for toolWrapper() since it returns IUO from ObjC
        toolWrapper()?.delegate?.delegate?.toolbeltDidSelectNamedMark(marks[row])
    }

    @objc func add(_ sender: Any) {
        toolWrapper()?.delegate?.delegate?.toolbeltAddNamedMark()
    }

    @objc func remove(_ sender: Any) {
        let selectedMarks = _tableView.selectedRowIndexes.compactMap { i -> iTermGenericNamedMarkReading? in
            guard i >= 0 && i < self.marks.count else { return nil }
            return self.marks[i]
        }
        for mark in selectedMarks {
            toolWrapper()?.delegate?.delegate?.toolbeltRemoveNamedMark(mark)
        }
    }

    @objc func edit(_ sender: Any) {
        let selectedMarks = _tableView.selectedRowIndexes.compactMap { i -> iTermGenericNamedMarkReading? in
            guard i >= 0 && i < self.marks.count else { return nil }
            return self.marks[i]
        }
        for mark in selectedMarks {
            toolWrapper()?.delegate?.delegate?.toolbeltRenameNamedMark(mark, to: nil)
        }
    }

    func controlTextDidEndEditing(_ obj: Notification) {
        guard
            let textField = obj.object as? NSTextField,
            let cell = textField.superview as? NSTableCellView,
            let tableView = _tableView else {
            return
        }
        let row = tableView.row(for: cell)
        guard row >= 0 && row < marks.count else {
            return
        }
        let newValue = textField.stringValue
        tableView.reloadData()
        toolWrapper()?.delegate?.delegate?.toolbeltRenameNamedMark(marks[row], to: newValue)
    }
}
