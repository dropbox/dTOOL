//
//  GradientView.swift
//  DashTerm2
//
//  Created by George Nachman on 2/20/25.
//

import Cocoa

class GradientView: NSView {
    struct Stop {
        var color: NSColor
        var location: CGFloat
    }
    struct Gradient {
        var stops: [Stop]
    }

    private var gradientLayer: CAGradientLayer? {
        // BUG-1709: Use as? instead of as! for layer cast (safe due to makeBackingLayer but more defensive)
        return layer as? CAGradientLayer
    }

    var gradient: Gradient {
        didSet {
            updateGradient()
        }
    }

    init(gradient: Gradient) {
        self.gradient = gradient
        super.init(frame: .zero)
        setupLayer()
    }

    required init?(coder: NSCoder) {
        // BUG-f801: Return nil instead of crashing for unused coder initializer
        DLog("GradientView init(coder:) is not supported")
        return nil
    }

    override func makeBackingLayer() -> CALayer {
        return CAGradientLayer()
    }

    private func setupLayer() {
        wantsLayer = true
        updateGradient()
    }

    private func updateGradient() {
        effectiveAppearance.it_perform {
            let sorted = gradient.stops.sorted { lhs, rhs in
                lhs.location < rhs.location
            }
            let colors = sorted.map { $0.color.cgColor }
            let locations = sorted.map { NSNumber(value: $0.location) }
            DLog("colors=\(colors)")
            DLog("locations=\(locations)")
            gradientLayer?.colors = colors
            gradientLayer?.locations = locations
        }
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        updateGradient()
    }
}
