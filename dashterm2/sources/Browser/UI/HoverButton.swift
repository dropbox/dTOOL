//
//  HoverButton.swift
//  DashTerm2
//
//  Created by George Nachman on 7/1/25.
//

@available(macOS 11, *)
class HoverButton: NSButton {
    private var trackingArea: NSTrackingArea?

    init(symbolName: String, accessibilityDescription: String) {
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 4
        bezelStyle = .regularSquare
        isBordered = false
        if let cell = cell as? NSButtonCell {
            cell.showsBorderOnlyWhileMouseInside = true
        }
        let config = NSImage.SymbolConfiguration(pointSize: 16, weight: .semibold)
        // BUG-1662: Use guard let instead of force unwrap for NSImage creation
        guard let symbolImage = NSImage(systemSymbolName: symbolName,
                                        accessibilityDescription: accessibilityDescription) else {
            return
        }
        image = symbolImage.withSymbolConfiguration(config)
    }
    
    required init?(coder: NSCoder) {
        // BUG-f844: Return nil instead of crashing for unused coder initializer
        DLog("HoverButton init(coder:) is not supported")
        return nil
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let ta = trackingArea {
            removeTrackingArea(ta)
        }
        let newTrackingArea = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInKeyWindow],
            owner: self,
            userInfo: nil
        )
        trackingArea = newTrackingArea
        addTrackingArea(newTrackingArea)
    }

    override func awakeFromNib() {
        super.awakeFromNib()
        wantsLayer = true
        layer?.cornerRadius = 4
    }

    override func mouseEntered(with event: NSEvent) {
        layer?.backgroundColor = NSColor.controlAccentColor
            .withAlphaComponent(0.2)
            .cgColor
    }

    override func mouseExited(with event: NSEvent) {
        layer?.backgroundColor = nil
    }

    override func sizeToFit() {
        var frame = self.frame
        frame.size = NSSize(width: 33.0, height: 28.8)
        self.frame = frame
    }
}
