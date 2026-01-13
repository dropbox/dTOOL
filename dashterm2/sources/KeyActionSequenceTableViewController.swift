//
//  KeyActionSequenceTableViewController.swift
//  DashTerm2SharedARC
//
//  Created by George Nachman on 4/10/22.
//

import Foundation

@objc
protocol iTermKeyActionSequenceTableViewControllerDelegate: AnyObject {
    func keyActionSequenceTableViewController(_ sender: KeyActionSequenceTableViewController,
                                              selectionDidChange action: iTermKeyBindingAction?)
    func keyActionSequenceTableViewControllerDidChange(
        _ sender: KeyActionSequenceTableViewController,
        actions: [iTermKeyBindingAction])
}

@objc(iTermKeyActionSequenceTableViewController)
class KeyActionSequenceTableViewController: NSObject {
    private static let pasteboardType = NSPasteboard.PasteboardType("com.dashterm.dashterm2.key-action")

    @IBOutlet weak var tableView: BackspaceDeletingTableView?
    @IBOutlet weak var addRemoveControl: NSSegmentedControl?
    @objc weak var delegate: iTermKeyActionSequenceTableViewControllerDelegate?

    private var _actions: [iTermKeyBindingAction] = []
    @objc var actions: [iTermKeyBindingAction] {
        get {
            return _actions
        }
        set {
            _actions = newValue
            tableView?.reloadData()
            updateRemoveEnabled()
        }
    }

    @objc var hasSelection: Bool {
        return tableView?.numberOfSelectedRows == 1
    }

    @objc var selectedItem: iTermKeyBindingAction? {
        guard hasSelection, let tableView = tableView else {
            return nil
        }
        let row = tableView.selectedRow
        guard row >= 0 && row < _actions.count else {
            return nil
        }
        return _actions[row]
    }

    override func awakeFromNib() {
        super.awakeFromNib()
        tableView?.backspace = { [weak self] in self?.removeSelected() }
        tableView?.registerForDraggedTypes([Self.pasteboardType])
        updateRemoveEnabled()
    }

    @IBAction @objc(addRemove:) func addRemove(_ sender: Any) {
        guard let control = sender as? NSSegmentedControl else {
            return
        }
        switch control.selectedSegment {
        case 0:
            add()
        case 1:
            removeSelected()
        default:
            break
        }
    }

    @objc func setActionForCurrentItem(_ action: KEY_ACTION) {
        guard hasSelection, let tableView = tableView else {
            return
        }
        let replacement = iTermKeyBindingAction.withAction(action,
                                                           parameter: "",
                                                           escaping: .none,
                                                           applyMode: .currentSession)
        let row = tableView.selectedRow
        // BUG-f620: Use guard instead of it_assert - assertions are stripped in release builds
        // causing crash if row is out of bounds
        guard row >= 0 && row < _actions.count else {
            DLog("Invalid row selection \(row) for action replacement (count: \(_actions.count))")
            return
        }
        _actions[row] = replacement
        reloadCurrentItem(nil)
    }

    @objc(reloadCurrentItem:)
    func reloadCurrentItem(_ item: iTermKeyBindingAction?) {
        guard hasSelection, let tableView = tableView else {
            return
        }
        let row = tableView.selectedRow
        // BUG-f621: Guard against out-of-bounds row access - selectedRow can return -1
        // or may be stale after async operations
        guard row >= 0 && row < _actions.count else {
            DLog("Invalid row \(row) in reloadCurrentItem (count: \(_actions.count))")
            return
        }
        pushUndo(row)
        if let item = item {
            _actions[row] = item
        }
        tableView.reloadData(forRowIndexes: IndexSet(integer: row),
                             columnIndexes: IndexSet(integer: 0))
    }

    private func add() {
        pushUndo()
        let insertAfter = maybeSelectedRow ?? actions.count - 1
        let action = iTermKeyBindingAction.withAction(.ACTION_NEXT_SESSION,
                                                      parameter: "",
                                                      escaping: .none,
                                                      applyMode: .currentSession)
        let row = insertAfter + 1
        _actions.insert(action, at: row)
        delegate?.keyActionSequenceTableViewControllerDidChange(self, actions: _actions)
        tableView?.beginUpdates()
        tableView?.insertRows(at: IndexSet(integer: row))
        tableView?.endUpdates()
        tableView?.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
    }

    private func removeSelected() {
        pushUndo()
        let indexes = tableView?.selectedRowIndexes ?? IndexSet()
        _actions.remove(at: indexes)
        tableView?.beginUpdates()
        tableView?.removeRows(at: indexes)
        tableView?.endUpdates()
        delegate?.keyActionSequenceTableViewControllerDidChange(self, actions: _actions)
    }

    private func updateRemoveEnabled() {
        addRemoveControl?.setEnabled((tableView?.selectedRow ?? -1) != -1,
                                     forSegment: 1)
    }

    private func pushUndo() {
        let savedActions = actions
        tableView?.undoManager?.registerUndo(withTarget: self) { target in
            target.pushUndo()
            target.actions = savedActions
        }
    }

    private func pushUndo(_ row: Int) {
        let savedActions = actions
        tableView?.undoManager?.registerUndo(withTarget: self) { target in
            if target.hasSelection {
                if let selectedRow = target.tableView?.selectedRow {
                    target.pushUndo(selectedRow)
                }
            } else {
                target.pushUndo()
            }
            target.actions = savedActions
            target.tableView?.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
            // BUG-f622: Guard against out-of-bounds access - row may be invalid after undo operations
            guard row >= 0 && row < savedActions.count else {
                DLog("Invalid row \(row) in undo closure (savedActions.count: \(savedActions.count))")
                return
            }
            target.delegate?.keyActionSequenceTableViewController(target,
                                                                  selectionDidChange: savedActions[row])
        }
    }
}

extension KeyActionSequenceTableViewController: NSTableViewDataSource {
    func numberOfRows(in tableView: NSTableView) -> Int {
        return actions.count
    }

    func tableView(_ tableView: NSTableView,
                   writeRowsWith rowIndexes: IndexSet,
                   to pboard: NSPasteboard) -> Bool {
        pboard.declareTypes([Self.pasteboardType], owner: self)
        let indexes = Array(rowIndexes)
        pboard.setPropertyList(indexes, forType: Self.pasteboardType)
        return true
    }

    func tableView(_ tableView: NSTableView,
                   validateDrop info: NSDraggingInfo,
                   proposedRow row: Int, proposedDropOperation
                   dropOperation: NSTableView.DropOperation) -> NSDragOperation {
        guard info.draggingSource as? NSTableView == tableView else {
            return []
        }
        if info.draggingPasteboard.types?.contains(Self.pasteboardType) ?? false {
            return [.move]
        }
        return []
    }

    func tableView(_ tableView: NSTableView,
                   acceptDrop info: NSDraggingInfo,
                   row: Int,
                   dropOperation: NSTableView.DropOperation) -> Bool {
        guard let indexes = info.draggingPasteboard.propertyList(forType: Self.pasteboardType) as? [Int] else {
            return false
        }

        // BUG-f1406: Validate all indexes before accessing _actions array
        // Indexes come from pasteboard and may be stale or invalid if array changed
        let validIndexes = indexes.filter { $0 >= 0 && $0 < _actions.count }
        guard validIndexes.count == indexes.count else {
            DLog("acceptDrop: Some indexes out of bounds - indexes=\(indexes), count=\(_actions.count)")
            return false
        }

        pushUndo()
        tableView.beginUpdates()
        let movingActions = validIndexes.map { _actions[$0] }
        let countBefore = indexes.filter { $0 < row }.count
        var temp = _actions
        let indexSet = IndexSet(indexes)
        temp.remove(at: indexSet)
        tableView.removeRows(at: indexSet)
        var destination = row - countBefore
        tableView.insertRows(at: IndexSet(integersIn: destination ..< destination + movingActions.count))
        for action in movingActions {
            temp.insert(action, at: destination)
            destination += 1
        }
        _actions = temp
        tableView.endUpdates()
        return true
    }
}

extension KeyActionSequenceTableViewController: NSTableViewDelegate {
    func tableView(_ tableView: NSTableView,
                   viewFor tableColumn: NSTableColumn?,
                   row: Int) -> NSView? {
        // BUG-f1019: Guard against out-of-bounds access - row may be stale if actions changed
        guard row >= 0 && row < actions.count else {
            DLog("KeyActionSequenceTableViewController.tableView(_:viewFor:row:): row \(row) out of bounds (actions.count=\(actions.count))")
            return nil
        }
        guard let tableColumn,
              let cell = tableView.makeView(withIdentifier: tableColumn.identifier,
                                            owner: self) as? NSTableCellView else {
            return nil
        }
        cell.textField?.stringValue = actions[row].displayName
        return cell
    }

    private var maybeSelectedRow: Int? {
        guard let view = tableView else {
            return nil
        }
        if view.numberOfSelectedRows != 1 {
            return nil
        }
        return view.selectedRow
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        let action: iTermKeyBindingAction?
        if let row = maybeSelectedRow {
            // BUG-f1020: Guard against out-of-bounds access - selectedRow may be stale
            if row >= 0 && row < actions.count {
                action = actions[row]
            } else {
                DLog("KeyActionSequenceTableViewController.tableViewSelectionDidChange: row \(row) out of bounds (actions.count=\(actions.count))")
                action = nil
            }
        } else {
            action = nil
        }
        delegate?.keyActionSequenceTableViewController(self, selectionDidChange: action)
        updateRemoveEnabled()
    }
}

@objc class BackspaceDeletingTableView: NSTableView {
    var backspace: (() -> ())? = nil
    override func keyDown(with event: NSEvent) {
        if event.modifierFlags.intersection([.control, .option, .command, .shift]).isEmpty &&
            event.characters == "\u{7F}" {
            backspace?()
            return
        }
        super.keyDown(with: event)
    }
}
