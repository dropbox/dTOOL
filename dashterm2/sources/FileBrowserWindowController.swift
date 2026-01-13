//
//  FileBrowserWindowController.swift
//  DashTerm2
//
//  Created by George Nachman on 10/6/22.
//

import AppKit

@objc(FileBrowserSourceListViewController)
class FileBrowserSourceListViewController: NSViewController {
    @objc(FileBrowserSourceListViewControllerView)
    class View: NSView {

    }

    override func loadView() {
        view = View()
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.red.cgColor
    }
}

@objc(FileBrowserContainerViewController)
class FileBrowserContainerViewController: NSViewController {
    @objc(FileBrowserContainerViewControllerView)
    class View: NSView {
    }

    override func loadView() {
        view = View()
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.green.cgColor
    }
}

@objc(FileBrowserSplitViewController)
class FileBrowserSplitViewController: NSSplitViewController {
    private let splitViewResorationIdentifier = "com.dashterm.dashterm2:FileBrowserSplitViewController"

    lazy var sourceListViewController = FileBrowserSourceListViewController()
    lazy var containerViewController = FileBrowserContainerViewController()

    override init(nibName nibNameOrNil: NSNib.Name?, bundle nibBundleOrNil: Bundle?) {
        super.init(nibName: nibNameOrNil, bundle: nibBundleOrNil)
        setupUI()
        setupLayout()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
    }

    private func setupUI() {
        view.wantsLayer = true

        splitView.dividerStyle = .paneSplitter
        splitView.autosaveName = NSSplitView.AutosaveName(splitViewResorationIdentifier)
        splitView.identifier = NSUserInterfaceItemIdentifier(rawValue: splitViewResorationIdentifier)

        sourceListViewController.view.widthAnchor.constraint(greaterThanOrEqualToConstant: 80).isActive = true
        containerViewController.view.widthAnchor.constraint(greaterThanOrEqualToConstant: 100).isActive = true
    }

    private func setupLayout() {
        let sidebarItem = NSSplitViewItem(sidebarWithViewController: sourceListViewController)
        sidebarItem.canCollapse = true
        sidebarItem.holdingPriority = NSLayoutConstraint.Priority(NSLayoutConstraint.Priority.defaultLow.rawValue + 1)
        addSplitViewItem(sidebarItem)

        let containerItem = NSSplitViewItem(viewController: containerViewController)
        addSplitViewItem(containerItem)
    }
}

@objc(FileBrowserToolbar)
class FileBrowserToolbar: NSToolbar, NSToolbarDelegate {
    init() {
        super.init(identifier: "com.dashterm.dashterm2:FileBrowserToolbar")
        delegate = self
        allowsUserCustomization = true
        autosavesConfiguration = true
        displayMode = .iconOnly
    }
}

extension NSView {
    func anchorToSuperviewBounds() {
        // BUG-1589: Use guard to safely access superview instead of force unwrap
        guard let superview = superview else {
            DLog("anchorToSuperviewBounds called on view without superview")
            return
        }
        heightAnchor.constraint(equalTo: superview.heightAnchor).isActive = true
        widthAnchor.constraint(equalTo: superview.widthAnchor).isActive = true
        topAnchor.constraint(equalTo: superview.topAnchor).isActive = true
        leadingAnchor.constraint(equalTo: superview.leadingAnchor).isActive = true
    }
}

@objc(FileBrowserWindowController)
@available(macOS 11.0, *)
class FileBrowserWindowController: NSWindowController {
    private let splitViewController = FileBrowserSplitViewController()
    private let toolbar: NSToolbar

    private struct ToolbarItems {
        let backForward: NSToolbarItem

        init() {
            // BUG-1667: Use guard with fallback instead of force unwrap for NSImage
            let ellipsisCircleImage = NSImage(systemSymbolName: SFSymbol.ellipsisCircle.rawValue,
                                              accessibilityDescription: "Action") ?? NSImage()
            let backForwardView = NSPopUpButton(image: ellipsisCircleImage, target: nil, action: nil)
            backForwardView.pullsDown = true
            backForward = NSToolbarItem(itemIdentifier: NSToolbarItem.Identifier(rawValue: "back and forward"))
            backForward.view = backForwardView
        }
    }
    private let toolbarItems = ToolbarItems()

    init() {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 922.0, height: 437.0),
                              styleMask: [.closable, .miniaturizable, .resizable, .titled, .unifiedTitleAndToolbar, .fullSizeContentView],
                              backing: .buffered,
                              defer: true,
                              screen: nil)
        window.contentView?.autoresizesSubviews = true
        window.toolbarStyle = .unified
        window.contentView?.addSubview(splitViewController.splitView)
        splitViewController.splitView.anchorToSuperviewBounds()

        toolbar = NSToolbar()
        toolbar.insertItem(withItemIdentifier: toolbarItems.backForward.itemIdentifier, at: 0)
        window.toolbar = toolbar
        toolbar.validateVisibleItems()

        super.init(window: window)
    }

    required init?(coder: NSCoder) {
        // BUG-f832: Return nil instead of crashing for unused coder initializer
        DLog("FileBrowserWindowController init(coder:) is not supported")
        return nil
    }
}

