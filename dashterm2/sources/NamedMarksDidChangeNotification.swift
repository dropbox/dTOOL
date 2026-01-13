//
//  NamedMarksDidChangeNotification.swift
//  DashTerm2
//
//  Created by George Nachman on 5/21/23.
//

import Foundation

@objc(iTermNamedMarksDidChangeNotification)
class NamedMarksDidChangeNotification: iTermBaseNotification {
    // If nil, reload all in browser sessions
    @objc var sessionGuid: String?

    @objc init(sessionGuid: String?) {
        self.sessionGuid = sessionGuid
        super.init(private: ())
    }

    @objc static func subscribe(owner: NSObject, block: @escaping (NamedMarksDidChangeNotification) -> Void) {
        internalSubscribe(owner) { notif in
            // BUG-1686: Use guard with as? instead of force cast for notification type
            guard let typedNotif = notif as? NamedMarksDidChangeNotification else { return }
            block(typedNotif)
        }
    }
}
