//
//  DonateViewController.swift
//  DashTerm2SharedARC
//
//  Created by George Nachman on 2/27/22.
//

import Foundation

@objc
private class DonateView: NSView {
}

@objc(iTermDonateViewController)
class DonateViewController: NSTitlebarAccessoryViewController {
    // BUG-1600: Use static URL constant with nil coalescing instead of force unwrap
    // BUG-1: Documentation URL updated to dashterm.com
    private static let donateURL = URL(string: "https://dashterm.com/donate.html") ?? URL(fileURLWithPath: "/")

    // BUG-149, BUG-405, BUG-407: Donate view controller strings updated to DashTerm2 branding
    // BUG-405: "Support DashTerm2" string
    // BUG-407: "Keep DashTerm2 alive" string
    private static func textString() -> String {
        // BUG-1601: Use nil coalescing instead of force unwrap for randomElement
        return ["Donate",
                "Support DashTerm2",
                "DashTerm2 is one person's project. Donate now!",
                "Keep DashTerm2 alive — Donate today!",
                "Love using DashTerm2? Help keep it thriving!",
                "DashTerm2 needs your support – Donate here.",
                "Help DashTerm2 grow – Consider donating.",
                "Keep the DashTerm2 dream alive – Donate!",
                "Support the creator of DashTerm2 – Donate now!",
        ].randomElement() ?? "Donate"
    }

    let innerVC = DismissableLinkViewController(userDefaultsKey: "NoSyncHideDonateLabel",
                                                text: DonateViewController.textString(),
                                                url: donateURL,
                                                clickToHide: true)
    init() {
        super.init(nibName: nil, bundle: nil)
        layoutAttribute = .right
    }

    required init?(coder: NSCoder) {
        // BUG-f840: Return nil instead of crashing for unused coder initializer
        DLog("DonateViewController init(coder:) is not supported")
        return nil
    }

    override func loadView() {
        view = DonateView()

        let subview = innerVC.view
        subview.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(subview)

        view.frame = subview.frame

        view.addConstraint(NSLayoutConstraint(item: view,
                                              attribute: .width,
                                              relatedBy: .equal,
                                              toItem: subview,
                                              attribute: .width,
                                              multiplier: 1,
                                              constant: 0))
        view.addConstraint(NSLayoutConstraint(item: view,
                                              attribute: .height,
                                              relatedBy: .greaterThanOrEqual,
                                              toItem: subview,
                                              attribute: .height,
                                              multiplier: 1,
                                              constant: 7.5))
        view.addConstraint(NSLayoutConstraint(item: view,
                                              attribute: .leading,
                                              relatedBy: .equal,
                                              toItem: subview,
                                              attribute: .leading,
                                              multiplier: 1,
                                              constant: 0))
        view.addConstraint(NSLayoutConstraint(item: view,
                                              attribute: .top,
                                              relatedBy: .equal,
                                              toItem: subview,
                                              attribute: .top,
                                              multiplier: 1,
                                              constant: -4
                                             ))
    }
}
