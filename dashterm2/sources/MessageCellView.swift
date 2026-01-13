import Cocoa

@objc
class MessageCellView: NSView {
    var textSelectable = true
    var customConstraints = [NSLayoutConstraint]()
    var rightClickMonitor: Any?
    var editable: Bool = false
    // store the messageUniqueID so that the edit button can pass it along.
    var messageUniqueID: UUID?
    static let topInset: CGFloat = 8
    static let bottomInset: CGFloat = 8
    // Callback for the edit button.
    var editButtonClicked: ((UUID) -> Void)?
    var forkButtonClicked: ((UUID) -> Void)?
    var maxWidthConstraint: NSLayoutConstraint?

    override var description: String {
        "<\(Self.self): \(it_addressString) editable=\(editable)>"
    }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupViews()
    }

    required init?(coder: NSCoder) {
        // BUG-f831: Return nil instead of crashing for unused coder initializer
        DLog("MessageCellView init(coder:) is not supported")
        return nil
    }

    deinit {
        if let monitor = rightClickMonitor {
            NSEvent.removeMonitor(monitor)
        }
    }


    func add(constraint: NSLayoutConstraint) {
        customConstraints.append(constraint)
        constraint.isActive = true
    }


    override func hitTest(_ point: NSPoint) -> NSView? {
        if textSelectable {
            return super.hitTest(point)
        }
        return self
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateColors()
    }

    func setupViews() {
    }

    func updateColors() {
    }

    func maxBubbleWidth(tableViewWidth: CGFloat) -> CGFloat {
        return max(16, tableViewWidth * 0.7)
    }

    override var menu: NSMenu? {
        get {
            DLog("menu \(self)")
            let menu = NSMenu(title: "Context Menu")
            if editable {
                let editItem = NSMenuItem(title: "Edit", action: #selector(editMenuItemClicked(_:)), keyEquivalent: "")
                editItem.target = self
                menu.addItem(editItem)
            }

            let copyItem = NSMenuItem(title: "Copy", action: #selector(copyMenuItemClicked(_:)), keyEquivalent: "")
            copyItem.target = self
            menu.addItem(copyItem)

            if editable {
                let forkItem = NSMenuItem(title: "Fork", action: #selector(forkMenuItemClicked(_:)), keyEquivalent: "")
                forkItem.target = self
                menu.addItem(forkItem)
            }

            return menu
        }
        set {
            DLog("Unexpected call to set menu")
        }
    }

    @objc func copyMenuItemClicked(_ sender: Any) {
        // BUG-439: Replace it_fatalError with DLog - subclass should implement but shouldn't crash if not
        DLog("Warning: \(type(of: self)).copyMenuItemClicked not implemented. Subclasses should override this method.")
    }
    @objc func forkMenuItemClicked(_ sender: Any) {
        if let id = messageUniqueID {
            forkButtonClicked?(id)
        }
    }
    @objc func editMenuItemClicked(_ sender: Any) {
        if let id = messageUniqueID {
            editButtonClicked?(id)
        }
    }

    func configure(with rendition: MessageRendition,
                   tableViewWidth: CGFloat) {
        configure(with: rendition,
                  maxBubbleWidth: self.maxBubbleWidth(tableViewWidth: tableViewWidth))
    }

    // BUG-f513, BUG-f674: Log error instead of crashing - subclasses must override this method
    func configure(with rendition: MessageRendition, maxBubbleWidth: CGFloat) {
        DLog("BUG-f513: MessageCellView.configure(with:maxBubbleWidth:) must be overridden by subclass \(type(of: self)) - no-op in base class")
    }
}

