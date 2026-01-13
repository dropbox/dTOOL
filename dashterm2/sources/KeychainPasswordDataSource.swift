//
//  KeychainPasswordDataSource.swift
//  DashTerm2SharedARC
//
//  Created by George Nachman on 3/19/22.
//

import AppKit

// BUG-301: Service name constants use DashTerm2 branding for keychain storage
private enum KeychainServiceNames {
    static let modern = "DashTerm2"
    static let modernBrowser = "DashTerm2-Browser"
    static let legacy = "DashTerm2"
    static let legacyBrowser = "DashTerm2-Browser"

    static func legacyService(for browser: Bool, matching modernService: String) -> String? {
        let candidate = browser ? KeychainServiceNames.legacyBrowser : KeychainServiceNames.legacy
        return candidate == modernService ? nil : candidate
    }
}

// Used to store account name in label and username in account. That was a mistake.
// Now it stores username and account name in accountName and account name in label (just for looks in keychain access)
fileprivate class ModernKeychainAccount: NSObject, PasswordManagerAccount {
    var hasOTP: Bool { false }
    var sendOTP: Bool { false }
    private let accountNameUserNameSeparator = "\u{2002}—\u{2002}"
    let accountName: String
    let userName: String
    private var keychainAccountName: String
    private var defective: Bool
    private let serviceName: String

    fileprivate init(serviceName: String, accountName: String, userName: String) {
        self.serviceName = serviceName
        self.accountName = accountName
        self.userName = userName
        defective = false
        keychainAccountName = accountName + accountNameUserNameSeparator + userName
    }

    fileprivate init?(serviceName: String, _ dict: NSDictionary) {
        self.serviceName = serviceName
        if let combinedAccountName = dict[kSecAttrAccount] as? String {
            if let range = combinedAccountName.range(of: accountNameUserNameSeparator) {
                accountName = String(combinedAccountName[..<range.lowerBound])
                userName = String(combinedAccountName[range.upperBound...])
                // Code path for well formed entries in 3.5.1beta3 and later.
                keychainAccountName = accountName + accountNameUserNameSeparator + userName
                defective = false
                DLog("Well-formed modern account username=\(userName) accountName=\(accountName) combined=\(combinedAccountName)")
            } else if let label = dict[kSecAttrLabel] as? String {
                // Code path for misbegotten entries created by 3.5.0.
                // It stored username in account and accountName in label.
                // But label is part of the value, not part of the key, so it's not a good place to store the account name.
                // Unfortunately username ended up being the unique key.
                DLog("Defective modern account label=\(label) combined=\(combinedAccountName)")
                accountName = label
                userName = combinedAccountName;
                keychainAccountName = combinedAccountName
                defective = true
            } else {
                return nil
            }
        } else {
            return nil
        }
    }

    var displayString: String {
        return keychainAccountName
    }

    func fetchPassword(context: RecipeExecutionContext,
                       _ completion: @escaping (String?, String?, Error?) -> ()) {
        do {
            completion(try password(), nil, nil)
        } catch {
            completion(nil, nil, error)
        }
    }

    func set(context: RecipeExecutionContext, password: String, completion: @escaping (Error?) -> ()) {
        do {
            try set(password: password)
            completion(nil)
        } catch {
            completion(error)
        }
    }

    func delete(context: RecipeExecutionContext, _ completion: @escaping (Error?) -> ()) {
        do {
            try delete()
            completion(nil)
        } catch {
            completion(error)
        }
    }

    private func password() throws -> String {
        return try SSKeychain.password(forService: serviceName,
                                       account: keychainAccountName,
                                       label: accountName)
    }

    private func set(password: String) throws {
        if defective {
            // Add a well-formed entry
            let correctKeychainAccountName = userName.isEmpty ? accountName : accountName + accountNameUserNameSeparator + userName
            try SSKeychain.setPassword(password,
                                       forService: serviceName,
                                       account: correctKeychainAccountName,
                                       label: accountName)
            // Delete the defective entry
            try SSKeychain.deletePassword(forService: serviceName,
                                          account: keychainAccountName,
                                          label: accountName)
            // Update internal state to be non-defective.
            keychainAccountName = correctKeychainAccountName
            defective = false
        } else {
            try SSKeychain.setPassword(password,
                                       forService: serviceName,
                                       account: keychainAccountName,
                                       label: accountName)
        }
    }

    private func delete() throws {
        try SSKeychain.deletePassword(forService: serviceName,
                                      account: keychainAccountName,
                                      label: accountName)
    }

    func matches(filter: String) -> Bool {
        return _matches(filter: filter)
    }
}

// Stores account name and user name together in account name and makes label "DashTerm2" or "DashTerm2"
// Supports reading from legacy (DashTerm2) service names and migrating to modern (DashTerm2) on write
fileprivate class LegacyKeychainAccount: NSObject, PasswordManagerAccount {
    private let accountNameUserNameSeparator = "\u{2002}—\u{2002}"
    var hasOTP: Bool { false }
    var sendOTP: Bool { false }

    let accountName: String
    let userName: String
    private let keychainAccountName: String
    private let serviceName: String
    private let originalServiceName: String  // The service name the entry was read from
    private let legacyServiceName: String    // Legacy service name for cleanup

    fileprivate init?(serviceName: String, legacyServiceName: String, _ dict: NSDictionary) {
        self.serviceName = serviceName
        self.legacyServiceName = legacyServiceName

        // Accept entries with either "DashTerm2" or "DashTerm2" labels
        if let combinedAccountName = dict[kSecAttrAccount] as? String,
           let label = dict[kSecAttrLabel] as? String,
           (label == KeychainServiceNames.legacy || label == KeychainServiceNames.modern) {
            // Determine which service this entry came from
            if let service = dict[kSecAttrService] as? String {
                self.originalServiceName = service
            } else {
                self.originalServiceName = serviceName
            }

            if let range = combinedAccountName.range(of: accountNameUserNameSeparator) {
                accountName = String(combinedAccountName[..<range.lowerBound])
                userName = String(combinedAccountName[range.upperBound...])
                DLog("Two-part legacy username=\(userName) account=\(accountName) combined=\(combinedAccountName)")
            } else {
                DLog("One-part legacy combined=\(combinedAccountName), using empty username")
                accountName = combinedAccountName
                userName = ""
            }
            keychainAccountName = combinedAccountName
        } else {
            return nil
        }
    }

    var displayString: String {
        return keychainAccountName
    }

    // BUG-f502: Return an error instead of crashing when OTP toggling is not supported
    func toggleShouldSendOTP(account: any PasswordManagerAccount, completion: @escaping (PasswordManagerAccount?, Error?) -> ()) {
        completion(nil, NSError(domain: "com.dashterm2.keychain",
                               code: -1,
                               userInfo: [NSLocalizedDescriptionKey: "OTP toggling is not supported by Keychain accounts"]))
    }

    func fetchPassword(context: RecipeExecutionContext, _ completion: @escaping (String?, String?, Error?) -> ()) {
        do {
            completion(try password(), nil, nil)
        } catch {
            completion(nil, nil, error)
        }
    }

    func set(context: RecipeExecutionContext, password: String, completion: @escaping (Error?) -> ()) {
        do {
            try set(password: password)
            completion(nil)
        } catch {
            completion(error)
        }
    }

    func delete(context: RecipeExecutionContext, _ completion: @escaping (Error?) -> ()) {
        do {
            try delete()
            completion(nil)
        } catch {
            completion(error)
        }
    }

    private func password() throws -> String {
        // Try to read from the original service name this entry came from
        return try SSKeychain.password(forService: originalServiceName,
                                       account: keychainAccountName)
    }

    private func set(password: String) throws {
        // Always write to modern service name
        try SSKeychain.setPassword(password,
                                   forService: serviceName,
                                   account: keychainAccountName,
                                   error: ())
        // If originally from legacy service, delete the legacy entry (migration)
        if originalServiceName == legacyServiceName {
            _ = try? SSKeychain.deletePassword(forService: legacyServiceName,
                                               account: keychainAccountName,
                                               error: ())
        }
    }

    private func delete() throws {
        // Delete from original service name
        try SSKeychain.deletePassword(forService: originalServiceName,
                                      account: keychainAccountName,
                                      error: ())
        // Also try to delete from the other service name in case it exists in both
        if originalServiceName != serviceName {
            _ = try? SSKeychain.deletePassword(forService: serviceName,
                                               account: keychainAccountName,
                                               error: ())
        }
    }

    func matches(filter: String) -> Bool {
        return _matches(filter: filter)
    }
}

class KeychainPasswordDataSource: NSObject, PasswordManagerDataSource {
    private let browser: Bool
    private var serviceName: String {
        browser ? KeychainServiceNames.modernBrowser : KeychainServiceNames.modern
    }
    private var legacyServiceName: String? {
        KeychainServiceNames.legacyService(for: browser, matching: serviceName)
    }
    init(browser: Bool) {
        self.browser = browser
        super.init()
    }

    // BUG-502 + CRASH-FIX: Make this a designated initializer (not convenience)
    // When called via NSClassFromString().init() / ObjC reflection, a convenience init
    // that delegates to another designated init can crash. Using a designated init
    // that directly initializes all properties and calls super.init() is safer.
    @objc override init() {
        self.browser = false  // Default to terminal mode
        super.init()
    }

    @objc var name: String { "Keychain" }
    @objc var canResetConfiguration: Bool { false }
    @objc func resetConfiguration() { }
    @objc var supportsMultipleAccounts: Bool { false }

    func fetchAccounts(context: RecipeExecutionContext,
                       completion: @escaping ([PasswordManagerAccount]) -> ()) {
        completion(self.accounts)
    }

    // BUG-f503: Return an error instead of crashing when OTP toggling is not supported
    func toggleShouldSendOTP(context: RecipeExecutionContext,
                             account: any PasswordManagerAccount,
                             completion: @escaping ((any PasswordManagerAccount)?, (any Error)?) -> ()) {
        completion(nil, NSError(domain: "com.dashterm2.keychain",
                               code: -1,
                               userInfo: [NSLocalizedDescriptionKey: "OTP toggling is not supported by Keychain"]))
    }

    func add(userName: String,
             accountName: String,
             password: String,
             context: RecipeExecutionContext,
             completion: @escaping (PasswordManagerAccount?, Error?) -> ()) {
        let account = ModernKeychainAccount(serviceName: serviceName,
                                            accountName: accountName,
                                            userName: userName)
        account.set(context: context, password: password) { error in
            if let error = error {
                completion(nil, error)
            } else {
                completion(account, nil)
            }
        }
    }

    func reload(_ completion: () -> ()) {
        completion()
    }

    private var accounts: [PasswordManagerAccount] {
        var seenAccountNames = Set<String>()
        var results: [PasswordManagerAccount] = []

        func appendAccounts(from dicts: [NSDictionary], builder: (NSDictionary) -> PasswordManagerAccount?) {
            for dict in dicts {
                guard let account = builder(dict) else {
                    continue
                }
                if !seenAccountNames.contains(account.accountName) {
                    seenAccountNames.insert(account.accountName)
                    results.append(account)
                }
            }
        }

        if let modernDicts = SSKeychain.accounts(forService: serviceName) as? [NSDictionary] {
            appendAccounts(from: modernDicts) { dict in
                ModernKeychainAccount(serviceName: serviceName, dict)
            }
        }

        if let legacyServiceName,
           let legacyDicts = SSKeychain.accounts(forService: legacyServiceName) as? [NSDictionary] {
            appendAccounts(from: legacyDicts) { dict in
                LegacyKeychainAccount(serviceName: serviceName,
                                      legacyServiceName: legacyServiceName,
                                      dict)
            }
        }
        return results
    }

    var autogeneratedPasswordsOnly: Bool {
        return false
    }

    func checkAvailability() -> Bool {
        return true
    }

    func resetErrors() {
    }

    func consolidateAvailabilityChecks(_ block: () -> ()) {
        block()
    }
    func switchAccount(completion: @escaping () -> ()) {
        completion()
    }
}
