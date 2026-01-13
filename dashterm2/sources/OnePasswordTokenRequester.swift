//
//  OnePasswordTokenRequester.swift
//  DashTerm2SharedARC
//
//  Created by George Nachman on 3/20/22.
//

import Foundation
import UniformTypeIdentifiers

class OnePasswordUtils {
    static let basicEnvironment = ["HOME": NSHomeDirectory()]
    private static var _customPathToCLI: String? = nil
    private(set) static var usable: Bool? = nil

    // BUG-f986: Default CLI path constant - prevents index out of bounds if normalPaths were empty
    private static let defaultCLIPath = "/usr/local/bin/op"

    // Standard paths to look for 1Password CLI
    private static let normalPaths = ["/usr/local/bin/op", "/opt/homebrew/bin/op"]

    // BUG-CLI-1: Check if CLI exists without showing dialogs
    static var cliExistsAtStandardPath: Bool {
        return normalPaths.anySatisfies { FileManager.default.fileExists(atPath: $0) }
    }

    /// Returns path to 1Password CLI without prompting the user (for availability checks)
    /// Returns nil if CLI is not found or not usable
    static var pathToCLISilent: String? {
        return pathToCLI(promptIfNotFound: false)
    }

    /// Returns path to 1Password CLI, prompting user if not found
    static var pathToCLI: String {
        return pathToCLI(promptIfNotFound: true) ?? defaultCLIPath
    }

    /// Internal implementation that controls whether to prompt the user
    private static func pathToCLI(promptIfNotFound: Bool) -> String? {
        if let customPath = _customPathToCLI {
            return customPath
        }
        // BUG-f986: Use .first with fallback instead of [0] to prevent potential index out of bounds
        let defaultPath = normalPaths.first ?? defaultCLIPath
        lazy var anyNormalPathExists = {
            return normalPaths.anySatisfies {
                FileManager.default.fileExists(atPath: $0)
            }
        }()
        if anyNormalPathExists {
            DLog("normal path exists")
            let goodPath = normalPaths.first {
                FileManager.default.fileExists(atPath: $0) && checkUsability($0)
            }
            if let goodPath {
                DLog("normal path ok")
                usable = true
                return goodPath
            }
            // File exists but not usable (wrong version)
            if usable == nil {
                DLog("usability fail")
                usable = false
                if promptIfNotFound {
                    showUnavailableMessage(normalPaths.joined(separator: " or "))
                }
            }
            // Return nil for silent mode if not usable
            return promptIfNotFound ? defaultPath : nil
        }
        // CLI not found at standard paths
        guard promptIfNotFound else {
            // Silent mode - just return nil
            return nil
        }
        if showCannotFindCLIMessage() {
            _customPathToCLI = askUserToFindCLI()
            if let path = _customPathToCLI {
                usable = checkUsability(path)
                if usable == false {
                    showUnavailableMessage()
                }
            }
        }
        return _customPathToCLI ?? defaultPath
    }

    static func throwIfUnusable() throws {
        // BUG-CLI-FIX: Use pathToCLISilent to avoid triggering dialogs
        // Only throw if CLI is not found or not usable
        guard pathToCLISilent != nil, usable == true else {
            throw OnePasswordDataSource.OPError.unusableCLI
        }
    }

    static func resetErrors() {
        if usable == false {
            usable = nil
            _customPathToCLI = nil
        }
        _majorVersion = nil
    }
    static func checkUsability() -> Bool {
        return checkUsability(pathToCLI)
    }

    /// Check usability without prompting the user (for availability checks)
    /// Returns false if CLI is not found or not usable
    static func checkUsabilitySilent() -> Bool {
        guard let path = pathToCLISilent else {
            return false
        }
        return checkUsability(path)
    }

    private static func checkUsability(_ path: String) -> Bool {
        return majorVersionNumber(path) == 2
    }

    static func showUnavailableMessage(_ path: String? = nil) {
        // BUG-151: OnePassword dialog messages updated to DashTerm2 branding
        let alert = NSAlert()
        alert.messageText = "OnePassword Unavailable"
        if let path = path {
            alert.informativeText = "The existing installation of the OnePassword CLI at \(path) is incompatible. The DashTerm2 integration requires version 2."
        } else {
            alert.informativeText = "Version 2 of the OnePassword CLI could not be found. Check that \(OnePasswordUtils.pathToCLI) is installed and has version 2.x."
        }
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    // Returns true to show an open panel to locate it.
    private static func showCannotFindCLIMessage() -> Bool {
        let alert = NSAlert()
        alert.messageText = "Can’t Find 1Password CLI"
        alert.informativeText = "In order to use the 1Password integration, DashTerm2 needs to know where to find the CLI app named \"op\". It's normally in /usr/local/bin. If you have installed it elsewhere, please select Locate to provide its location."
        alert.addButton(withTitle: "Locate")
        alert.addButton(withTitle: "Cancel")
        return alert.runModal() == .alertFirstButtonReturn
    }

    private static func askUserToFindCLI() -> String? {
        class OnePasswordCLIFinderOpenPanelDelegate: NSObject, NSOpenSavePanelDelegate {
            func panel(_ sender: Any, shouldEnable url: URL) -> Bool {
                if FileManager.default.itemIsDirectory(url.path) {
                    return true
                }
                return url.lastPathComponent == "op"
            }
        }
        let panel = NSOpenPanel()
        panel.directoryURL = URL(fileURLWithPath: "/usr/local/bin")
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [ UTType.unixExecutable ]
        let delegate = OnePasswordCLIFinderOpenPanelDelegate()
        return withExtendedLifetime(delegate) {
            panel.delegate = delegate
            if panel.runModal() == .OK,
                let url = panel.url,
                url.lastPathComponent == "op" {
                return url.path
            }
            return nil
        }
    }

    static func standardEnvironment(token: OnePasswordTokenRequester.Auth) -> [String: String] {
        var result = OnePasswordUtils.basicEnvironment
        switch token {
        case .biometric:
            break
        case .token(let token):
            result["OP_SESSION_my"] = token
        }
        if !iTermAdvancedSettingsModel.onePasswordAccount().isEmpty {
            result["OP_ACCOUNT"] = iTermAdvancedSettingsModel.onePasswordAccount()
        }
        return result
    }

    static func majorVersionNumber() -> Int? {
        return majorVersionNumber(pathToCLI)
    }

    static var _majorVersion: Int?
    private static func majorVersionNumber(_ pathToCLI: String) -> Int? {
        if let _majorVersion {
            return _majorVersion
        }
        let maybeData = try? CommandLinePasswordDataSource.InteractiveCommandRequest(
            command: pathToCLI,
            args: ["-v"],
            env: [:]).exec().stdout
        if let data = maybeData, let string = String(data: data, encoding: .utf8) {
            var value = 0
            DLog("version string is \(string)")
            if Scanner(string: string).scanInt(&value) {
                DLog("scan returned \(value)")
                _majorVersion = value
                return value
            }
            DLog("scan failed")
            return nil
        }
        DLog("Didn't get a version number")
        return nil
    }
}

class OnePasswordAccountPicker {
    static func askUserToSelect(from accounts: [Account]) {
        let pickerAccounts = accounts.map {
            AccountPicker.Account(title: $0.email, accountID: $0.account_uuid)
        }
        let identifier = AccountPicker.askUserToSelect(from: pickerAccounts)
        iTermAdvancedSettingsModel.setOnePasswordAccount(identifier)
    }

    struct Account: Codable {
        var url: String?
        var email: String?
        var user_uuid: String?
        var account_uuid: String?
    }
    static func asyncGetAccountList(_ completion: @escaping (Result<[Account], Error>) -> ()) {
        DLog("Read account list")
        // BUG-CLI-FIX: Check CLI availability silently before trying to use it
        guard let cli = OnePasswordUtils.pathToCLISilent else {
            DLog("1Password CLI not available (silent check)")
            completion(.failure(OnePasswordDataSource.OPError.unusableCLI))
            return
        }
        let command = CommandLinePasswordDataSource.CommandRequestWithInput(
            command: cli,
            args: ["account", "list", "--format=json"],
            env: OnePasswordUtils.basicEnvironment,
            input: Data())
        DLog("Will execute account list")
        command.execAsync { (output: Output?, error: (any Error)?) in
            handle(output: output, error: error, completion: completion)
        }
    }

    private static func handle(output: Output?,
                               error: (any Error)?,
                               completion: (Result<[Account], Error>) -> ()) {
        DLog("account list finished")
        guard let output = output else {
            DLog("But there is no output")
            completion(.failure(error ?? OnePasswordDataSource.OPError.unexpectedError))
            return
        }
        guard output.returnCode == 0 else {
            DLog("But the return code is nonzero")
            completion(.failure(OnePasswordDataSource.OPError.unexpectedError))
            return
        }
        let decoder = JSONDecoder()
        guard let accounts = try? decoder.decode([Account].self, from: output.stdout) else {
            DLog("Failed to parse \(output)")
            completion(.failure(OnePasswordDataSource.OPError.unexpectedError))
            return
        }
        completion(.success(accounts))
    }
}

class OnePasswordTokenRequester {
    private var token = ""
    private static var biometricsAvailable: Bool? = nil

    enum Auth {
        case biometric
        case token(String)
    }

    private func argsByAddingAccountArg(_ argsIn: [String]) -> [String] {
        var args = argsIn
        let account = iTermAdvancedSettingsModel.onePasswordAccount() ?? ""
        if !account.isEmpty {
            args += ["--account", account]
        }
        return args
    }

    private var passwordPrompt: String {
        let account = iTermAdvancedSettingsModel.onePasswordAccount() ?? ""
        if account.isEmpty {
            return "Enter your 1Password master password:"
        }
        return "Enter the 1Password master password for account “\(account)”:"
    }

    func asyncGet(_ completion: @escaping (Result<Auth, Error>) -> ()) {
        DLog("Begin asyncGet")
        switch Self.biometricsAvailable {
        case .none:
            asyncCheckBiometricAvailability() { [weak self] availability in
                guard let self = self else {
                    DLog("Biometrics check finished but self is dealloced")
                    completion(.failure(OnePasswordDataSource.OPError.canceledByUser))
                    return
                }
                switch availability {
                case .some(true):
                    DLog("biometrics are available")
                    Self.biometricsAvailable = true
                    completion(.success(.biometric))
                case .some(false):
                    DLog("biometrics unavailable, continue with regular auth")
                    Self.biometricsAvailable = false
                    self.asyncGetWithoutBiometrics(completion)
                case .none:
                    DLog("Failed to look up biometrics")
                    completion(.failure(OnePasswordDataSource.OPError.canceledByUser))
                }
            }
        case .some(true):
            completion(.success(.biometric))
        case .some(false):
            asyncGetWithoutBiometrics(completion)
        }
    }

    private func asyncGetWithoutBiometrics(_ completion: @escaping (Result<Auth, Error>) -> ()) {
        dispatchPrecondition(condition: .onQueue(.main))
        guard let password = self.requestPassword(prompt: self.passwordPrompt) else {
            completion(.failure(OnePasswordDataSource.OPError.canceledByUser))
            return
        }
        self.asyncGet(password: password, completion)
    }

    private func asyncGet(password: String, _ completion: @escaping (Result<Auth, Error>) -> ()) {
        DLog("Read password from user entry")
        // BUG-CLI-FIX: Check CLI availability silently before trying to use it
        guard let cli = OnePasswordUtils.pathToCLISilent else {
            DLog("1Password CLI not available (silent check)")
            completion(.failure(OnePasswordDataSource.OPError.unusableCLI))
            return
        }
        // BUG-1627: Use nil coalescing instead of force unwrap for data encoding
        let command = CommandLinePasswordDataSource.CommandRequestWithInput(
            command: cli,
            args: argsByAddingAccountArg(["signin", "--raw"]),
            env: OnePasswordUtils.basicEnvironment,
            input: (password + "\n").data(using: .utf8) ?? Data())
        DLog("Will execute signin --raw")
        command.execAsync { [weak self] output, error in
            DLog("signin --raw finished")
            guard let self = self else {
                DLog("But I have been dealloced")
                return
            }
            guard let output = output else {
                DLog("But there is no output")
                completion(.failure(error ?? OnePasswordDataSource.OPError.unexpectedError))
                return
            }
            guard output.returnCode == 0 else {
                DLog("But the return code is nonzero")
                DLog("signin failed")
                let reason = String(data: output.stderr, encoding: .utf8) ?? "An unknown error occurred."
                DLog("Failure reason is: \(reason)")
                if reason.contains("connecting to desktop app timed out") {
                    completion(.failure(OnePasswordDataSource.OPError.unusableCLI))
                    return
                }
                self.showErrorMessage(reason)
                completion(.failure(OnePasswordDataSource.OPError.needsAuthentication))
                return
            }
            guard let token = String(data: output.stdout, encoding: .utf8) else {
                DLog("got garbage output")
                self.showErrorMessage("The 1Password CLI app produced garbled output instead of an auth token.")
                completion(.failure(OnePasswordDataSource.OPError.badOutput))
                return
            }
            DLog("Got a token, yay")
            completion(.success(.token(token.trimmingCharacters(in: CharacterSet.whitespacesAndNewlines))))
        }
    }

    private func showErrorMessage(_ reason: String) {
        let alert = NSAlert()
        alert.messageText = "Authentication Error"
        alert.informativeText = reason
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    private func requestPassword(prompt: String) -> String? {
        DLog("requesting master password")
        return ModalPasswordAlert(prompt).run(window: nil)
    }

    // Returns nil if it was canceled by the user.
    func checkBiometricAvailability() -> Bool? {
        // BUG-CLI-FIX: Check usability BEFORE accessing pathToCLI to avoid showing dialogs
        // when CLI is not installed
        guard let cli = OnePasswordUtils.pathToCLISilent else {
            DLog("No usable version of 1password's op utility was found (silent check)")
            return nil
        }
        if OnePasswordUtils.usable != true {
           DLog("No usable version of 1password's op utility was found")
            // Don't ask for the master password if we don't have a good CLI to use.
            return nil
        }
        var command = CommandLinePasswordDataSource.InteractiveCommandRequest(
            command: cli,
            args: argsByAddingAccountArg(["user", "get", "--me"]),
            env: OnePasswordUtils.basicEnvironment)
        command.useTTY = true
        guard let output = try? command.exec() else {
            DLog("command.exec() threw an error")
            return nil
        }
        if output.returnCode == 0 {
            DLog("op user get --me succeeded so biometrics must be available")
            return true
        }
        guard let string = String(data: output.stderr, encoding: .utf8) else {
            DLog("garbage output")
            return false
        }
        DLog("op signin returned \(string)")
        if string.contains("error initializing client: authorization prompt dismissed, please try again") {
            return nil
        }
        return false
    }

    func asyncCheckBiometricAvailability(_ completion: @escaping (Bool?) -> ()) {
        // BUG-CLI-FIX: Check usability BEFORE accessing pathToCLI to avoid showing dialogs
        // when CLI is not installed
        guard let cli = OnePasswordUtils.pathToCLISilent else {
            DLog("No usable version of 1password's op utility was found (silent check)")
            completion(nil)
            return
        }
        if OnePasswordUtils.usable != true {
           DLog("No usable version of 1password's op utility was found")
            // Don't ask for the master password if we don't have a good CLI to use.
            completion(nil)
            return
        }
        var command = CommandLinePasswordDataSource.InteractiveCommandRequest(
            command: cli,
            args: argsByAddingAccountArg(["user", "get", "--me"]),
            env: OnePasswordUtils.basicEnvironment)
        command.useTTY = true
        command.execAsync { output, error in
            DispatchQueue.main.async {
                guard let output = output else {
                    completion(false)
                    return
                }
                if output.returnCode == 0 {
                    DLog("op user get --me succeeded so biometrics must be available")
                    completion(true)
                    return
                }
                guard let string = String(data: output.stderr, encoding: .utf8) else {
                    DLog("garbage output")
                    completion(false)
                    return
                }
                DLog("op signin returned \(string)")
                if string.contains("error initializing client: authorization prompt dismissed, please try again") {
                    completion(nil)
                    return
                }
                completion(false)
            }
        }
    }
}

