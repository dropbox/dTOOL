//
//  AccountPicker.swift
//  DashTerm2
//
//  Created by George Nachman on 11/25/25.
//

import AppKit

class AccountPicker {
    struct Account: Codable {
        var title: String?
        var accountID: String?
    }

    static func askUserToSelect(from accounts: [Account]) -> String {
        DLog("begin")
        let alert = NSAlert()
        alert.messageText = "Select an Account"
        alert.informativeText = "Please choose an account:"
        alert.alertStyle = .informational

        var ids = [String]()
        for account in accounts {
            if let email = account.title, let uuid = account.accountID {
                alert.addButton(withTitle: email)
                ids.append(uuid)
            }
        }
        if ids.count == 1 {
            return ids[0]
        }
        // BUG-470: Handle case where no valid accounts found instead of crashing
        guard ids.count > 1 else {
            DLog("Error: No valid accounts with both title and accountID found")
            // Return empty string to indicate no selection possible
            return ""
        }

        // Can't present a sheet modal within a sheet modal so go app modal instead.
        let response = alert.runModal()

        let selectedIndex = response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue

        // BUG-353: Guard against out-of-bounds array access
        // User may cancel dialog (Escape key) or system may return unexpected response
        // which would result in selectedIndex being negative or >= ids.count
        guard selectedIndex >= 0 && selectedIndex < ids.count else {
            DLog("Invalid response index \(selectedIndex), returning first account")
            return ids[0]  // Safe: ids.count > 1 verified above
        }

        let uuid = ids[selectedIndex]
        return uuid
    }
}
