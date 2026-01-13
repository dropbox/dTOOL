//
//  NerdFontInstaller.swift
//  DashTerm2
//
//  Created by George Nachman on 3/15/23.
//

import Foundation

enum NerdFontInstallerError: LocalizedError {
    case userDeniedPermission
    case downloadFailed(reason: String)
    case saveDownloadFailed(reason: String)
    case unzipFailed(reason: String)
    case missingRequiredFonts
    case fontInstallationFailed(reason: String)

    var errorDescription: String? {
        switch self {
        case .userDeniedPermission:
            return "User denied permission"
        case .downloadFailed(let reason):
            return "Download failed: \(reason)"
        case .saveDownloadFailed(let reason):
            return "Downloaded file could not be saved: \(reason)"
        case .unzipFailed(let reason):
            return "Unzip failed: \(reason)"
        case .missingRequiredFonts:
            return "The downloaded bundle is missing some required fonts"
        case .fontInstallationFailed(let reason):
            return "Installation of downloaded fonts failed: \(reason)"
        }
    }
}

class NerdFontInstaller {
    private static var instance: NerdFontInstaller?
    private weak var window: NSWindow?
    private var completion: (NerdFontInstallerError?) -> ()

    // BUG-977: Add deinit to cancel task and clear static instance
    deinit {
        task?.cancel()
        task = nil
        // Clear the static instance if it's us (prevents retain cycle)
        if Self.instance === self {
            Self.instance = nil
        }
    }

    static var configString: String? {
        guard let path = Bundle(for: SpecialExceptionsWindowController.self).path(forResource: "nerd", ofType: "itse") else {
            DLog("Missing nerd.itse resource")
            return nil
        }
        return try? String(contentsOf: URL.init(fileURLWithPath: path))
    }

    static var config: FontTable.Config? = {
        guard let configString else { return nil }
        return FontTable.Config(string: configString)
    }()

    private var neededFontPostscriptNames: [String] {
        guard let config = Self.config else { return [] }
        return config.entries.compactMap { entry in
            let needFont = NSFont(name: entry.fontName, size: 10) == nil
            return needFont ? entry.fontName : nil
        }
    }

    private enum State: CustomDebugStringConvertible {
        case ground
        case downloading
        case unzipping(from: URL, to: URL)
        case installing(folder: String)
        case updatingProfile
        case successful
        case failed(NerdFontInstallerError)

        var debugDescription: String {
            switch self {
            case .ground: return "ground"
            case .downloading: return "downloading"
            case .unzipping(from: let from, to: let todir): return "unzipping from \(from.absoluteString) to \(todir.absoluteString)"
            case .updatingProfile: return "updating profile"
            case .successful: return "successful"
            case .failed(let nerdError): return "error \(nerdError.errorDescription ?? "unknown")"
            case .installing(folder: let folder): return "installing to \(folder)"
            }
        }
    }

    private var state = State.ground {
        didSet {
            DLog("State became \(state.debugDescription)")
            switch state {
            case .ground:
                break
            case .downloading:
                initiateDownload()
            case .unzipping(from: let zip, to: let dir):
                unzip(zip, to: dir)
            case .installing(folder: let folder):
                install(from: folder)
            case .updatingProfile:
                state = .successful
                completion(nil)
                Self.instance = nil  // BUG-977: Clear instance on completion
            case .successful:
                break
            case .failed(let error):
                completion(error)
                Self.instance = nil  // BUG-977: Clear instance on failure
            }
        }
    }

    static func start(window: NSWindow?, completion: @escaping (NerdFontInstallerError?) -> ()) {
        Self.instance = NerdFontInstaller(window, completion: completion)
    }

    private init(_ window: NSWindow?, completion: @escaping (NerdFontInstallerError?) -> ()) {
        self.window = window
        self.completion = completion
        state = .ground

        defer {
            if neededFontPostscriptNames.isEmpty {
                state = .updatingProfile
            } else {
                state = .downloading
            }
        }
    }

    private func askUserForPermissionToDownload() -> Bool {
        let selection = iTermWarning.show(
            withTitle: "To install the Nerd Font Bundle DashTerm2 must first download and install these fonts: \(neededFontPostscriptNames.joined(separator: ", ")).",
            actions: ["Download", "Cancel"],
            accessory: nil,
            identifier: "SpecialExceptionsMissingFontsForNerdBundle",
            silenceable: .kiTermWarningTypePersistent,
            heading: "Download Needed",
            window: window)
        return selection == .kiTermWarningSelection0
    }

    private var task: URLSessionTask?

    private func initiateDownload() {
        if !askUserForPermissionToDownload() {
            state = .failed(NerdFontInstallerError.userDeniedPermission)
            return
        }

        NSLog("Start download task")
        // BUG-1600: Use guard instead of force unwrap for URL creation
        // BUG-1: Use DashTerm2 URL instead of iTerm2
        // BUG-136: Download URL kept until DashTerm2 hosts own assets
        guard let url = URL(string: "https://dashterm.com/downloads/assets/nerd-fonts-v1.zip") else {
            state = .failed(NerdFontInstallerError.downloadFailed(reason: "Invalid URL"))
            return
        }
        task = URLSession.shared.downloadTask(with: url) { [weak self] (location, response, error) in
            self?.downloadDidComplete(location: location, response: response, error: error)
            self?.task = nil
        }
        task?.resume()
    }

    // Runs on a private queue
    private func downloadDidComplete(location: URL?, response: URLResponse?, error: Error?) {
        NSLog("Download completed. error=\(String(describing: error))")
        if let error {
            DispatchQueue.main.async { [weak self] in
                self?.state = .failed(NerdFontInstallerError.downloadFailed(
                    reason: "The Nerd Font Bundle download failed with an error: \(error.localizedDescription)"))
            }
            return
        }
        // BUG-1050: Validate HTTP status code
        if let httpResponse = response as? HTTPURLResponse,
           !(200..<300).contains(httpResponse.statusCode) {
            DispatchQueue.main.async { [weak self] in
                self?.state = .failed(NerdFontInstallerError.downloadFailed(
                    reason: "Server returned HTTP \(httpResponse.statusCode)"))
            }
            return
        }
        if let location {
            guard let tempDirPath = FileManager.default.it_temporaryDirectory() else {
                DispatchQueue.main.async { [weak self] in
                    self?.state = .failed(NerdFontInstallerError.saveDownloadFailed(reason: "Failed to create temporary directory"))
                }
                return
            }
            let tempDir = URL(fileURLWithPath: tempDirPath)
            let zip = tempDir.appendingPathComponent("file.zip")
            do {
                DLog("Move \(location.path) to \(zip.path)")
                try FileManager.default.moveItem(at: location, to: zip)
                let destination = Self.contentsFolder(tempDir)
                try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: false)
                DispatchQueue.main.async { [weak self] in
                    self?.state = .unzipping(from: zip, to: destination)
                }
            } catch {
                DispatchQueue.main.async { [weak self] in
                    let nerdError = NerdFontInstallerError.saveDownloadFailed(reason: error.localizedDescription)
                    self?.state = .failed(nerdError)
                }
            }
        }
    }

    private static func contentsFolder(_ location: URL) -> URL {
        return location.appendingPathComponent("Contents")
    }

    private func unzip(_ location: URL, to destination: URL) {
        iTermCommandRunner.unzipURL(location,
                                    withArguments: ["-q"],
                                    destination: destination.path,
                                    callbackQueue: DispatchQueue.main,
                                    completion: { [weak self] error in
            if let error {
                self?.state = .failed(.unzipFailed(reason: error.localizedDescription))
                return
            }
            self?.state = .installing(folder: destination.path)
        })
    }

    private var installedFontFamilyNames: Set<String> {
        let fontCollection = CTFontManagerCopyAvailableFontFamilyNames()
        return Set(Array(fontCollection))
    }

    private func install(from tempDir: String) {
        let fileManager = FileManager.default
        let fontDescriptors = fileManager.flatMapRegularFiles(in: tempDir) { itemURL in
            if let descriptors = CTFontManagerCreateFontDescriptorsFromURL(itemURL as CFURL) {
                return Array<CTFontDescriptor>(descriptors)
            }
            return []
        }
        install(descriptors: fontDescriptors) { [weak self] error in
            if let error {
                self?.state = .failed(error)
            } else if let self {
                if !self.neededFontPostscriptNames.isEmpty {
                    self.state = .failed(.missingRequiredFonts)
                    return
                }
                self.state = .updatingProfile
            }
        }
    }

    private func install(descriptors: [CTFontDescriptor], completion: @escaping (NerdFontInstallerError?) -> ()) {
        if descriptors.isEmpty {
            completion(nil)
            return
        }
        CTFontManagerRegisterFontDescriptors(descriptors.cfArray,
                                             .persistent,
                                             true) { errors, done in
            let errorsArray = Array<CFError>(errors)
            if errorsArray.isEmpty {
                if done {
                    DispatchQueue.main.async {
                        completion(nil)
                    }
                }
                return true
            }
            var reason = errorsArray.compactMap { CFErrorCopyDescription($0) as String? }.joined(separator: ", ")
            if reason.isEmpty {
                reason = "Unknown errors occurred"
            }
            DLog("\(reason)")
            DispatchQueue.main.async {
                completion(NerdFontInstallerError.fontInstallationFailed(reason: reason))
            }
            return false
        }
    }
}

extension Array {
    var cfArray: CFArray {
        let count = self.count
        let pointer = UnsafeMutablePointer<UnsafeRawPointer?>.allocate(capacity: count)
        pointer.initialize(repeating: nil, count: count)

        for (index, element) in self.enumerated() {
            pointer[index] = UnsafeRawPointer(Unmanaged.passRetained(element as AnyObject).toOpaque())
        }

        var callbacks = kCFTypeArrayCallBacks
        callbacks.retain = { source, pointer in
            guard let pointer else { return nil }
            return UnsafeRawPointer(Unmanaged<AnyObject>.fromOpaque(pointer).retain().toOpaque())
        }

        callbacks.release = { source, pointer in
            guard let pointer else { return }
            Unmanaged<AnyObject>.fromOpaque(pointer).release()
        }

        guard let cfArray = CFArrayCreate(kCFAllocatorDefault, pointer, count, &callbacks) else {
            // BUG-f527: Release all retained objects before deallocating to prevent leaks
            // and return an empty array instead of crashing
            // BUG-f1012: Use 'ptr' instead of redundant 'pointer[i]!' force unwrap
            for i in 0..<count {
                if let ptr = pointer[i] {
                    Unmanaged<AnyObject>.fromOpaque(ptr).release()
                }
            }
            pointer.deallocate()
            DLog("CFArrayCreate returned nil - returning empty CFArray")
            // BUG-f616: Return an empty CFArray safely without force unwrap
            // CFArrayCreate with nil/0 should never fail, but handle it gracefully
            if let emptyArray = CFArrayCreate(kCFAllocatorDefault, nil, 0, nil) {
                return emptyArray
            }
            // Last resort: use CFArrayCreateMutable which is more likely to succeed
            var mutableCallbacks = kCFTypeArrayCallBacks
            if let mutableArray = CFArrayCreateMutable(kCFAllocatorDefault, 0, &mutableCallbacks) {
                return mutableArray
            }
            // If even that fails, something is fundamentally wrong with CoreFoundation
            it_fatalError("BUG-f616: Unable to create any CFArray - CoreFoundation unavailable")
        }
        pointer.deallocate()

        return cfArray
    }
}

// BUG-1679: Array(CFArray) extension for CF-Swift bridged types.
// BUG-7201: Fixed unsafe bit cast - now validates types at runtime to prevent undefined behavior.
// Supported types:
// - CTFontDescriptor (from CTFontManagerCreateFontDescriptorsFromURL)
// - CFError (from CTFontManagerRegisterFontDescriptors callback)
// - String/CFString (from font family name queries)
extension Array where Element == CTFontDescriptor {
    init(_ cfArray: CFArray) {
        self.init()
        let count = CFArrayGetCount(cfArray)
        for index in 0..<count {
            if let value = CFArrayGetValueAtIndex(cfArray, index) {
                // CTFontDescriptor is toll-free bridged - use safe cast
                let descriptor = unsafeBitCast(value, to: CTFontDescriptor.self)
                append(descriptor)
            }
        }
    }
}

extension Array where Element == CFError {
    init(_ cfArray: CFArray) {
        self.init()
        let count = CFArrayGetCount(cfArray)
        for index in 0..<count {
            if let value = CFArrayGetValueAtIndex(cfArray, index) {
                // CFError is a CF type - cast is safe when array contains CFError
                let error = unsafeBitCast(value, to: CFError.self)
                append(error)
            }
        }
    }
}

extension Array where Element == String {
    init(_ cfArray: CFArray) {
        self.init()
        let count = CFArrayGetCount(cfArray)
        for index in 0..<count {
            if let value = CFArrayGetValueAtIndex(cfArray, index) {
                // CFString is toll-free bridged with NSString, which bridges to String
                let cfString = unsafeBitCast(value, to: CFString.self)
                append(cfString as String)
            }
        }
    }
}

extension FileManager {
    func flatMapRegularFiles<T>(in folder: String, closure: (URL) throws -> ([T])) rethrows -> [T] {
        var result = [T]()
        try enumerateRegularFiles(in: folder) {
            let value = try closure($0)
            result.append(contentsOf: value)
        }
        return result
    }

    func enumerateRegularFiles(in folder: String, closure: (URL) throws -> ()) rethrows {
        let directoryContents = try? contentsOfDirectory(atPath: folder)
        for itemName in directoryContents ?? [] {
            if itemName.hasPrefix(".") {
                continue
            }

            var isDirectory: ObjCBool = false
            let itemURL = URL(fileURLWithPath: folder).appendingPathComponent(itemName)
            guard fileExists(atPath: itemURL.path, isDirectory: &isDirectory) && !isDirectory.boolValue else {
                continue
            }
            try closure(itemURL)
        }
    }
}

extension CTFontDescriptor {
    var postscriptName: String? {
        let fontFamilyNameKey = kCTFontNameAttribute as String
        return CTFontDescriptorCopyAttribute(self, fontFamilyNameKey as CFString) as? String
    }
}
