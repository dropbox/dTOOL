//
//  ConductorFileTransfer.swift
//  DashTerm2
//
//  Created by George Nachman on 2/2/23.
//

import Foundation

@available(macOS 11, *)
@MainActor
protocol ConductorFileTransferDelegate: AnyObject {
    func beginDownload(fileTransfer: ConductorFileTransfer)
    func beginUpload(fileTransfer: ConductorFileTransfer)
}

@available(macOS 11, *)
@MainActor
@objc
class ConductorFileTransfer: TransferrableFile {
    // BUG-2769: Add reasonable file size limits to prevent resource exhaustion
    // 10 GB max file size - allows large files but prevents DoS
    static let maxFileSize: Int64 = 10 * 1024 * 1024 * 1024
    // 100 MB max in-memory data size - prevents memory exhaustion for data-based transfers
    static let maxInMemoryDataSize: Int = 100 * 1024 * 1024

    @objc var path: SCPPath
    weak var delegate: ConductorFileTransferDelegate?
    private var _error = ""
    private var _localPath: String?
    let data: Data?

    private enum State {
        case idle
        case downloading
        case uploading
        case downloadComplete
        case uploadComplete
        case failed
    }
    private var state = State.idle

    init(path: SCPPath,
         localPath: String?,
         data: Data?,
         delegate: ConductorFileTransferDelegate) {
        self.path = path
        self.data = data
        self._localPath = localPath
        self.delegate = delegate
        // BUG-4006: Must call super.init() to initialize parent class TransferrableFile
        super.init()
    }

    override func displayName() -> String! {
        return """
        DashTerm2 SSH Integration Protocol
        User name: \(path.username ?? "(unknown)")")
        Host: \(path.hostname ?? "(unknown)")
        File: \(path.path ?? "(unknown)")"
        """
    }

    override func shortName() -> String! {
        return path.path?.lastPathComponent ?? "(unknown)"
    }

    override func subheading() -> String! {
        (path.hostname ?? "unknown") + " via SSH Integration"
    }

    override func authRequestor() -> String! {
        let hostname = path.hostname ?? "unknown"
        if let username = path.username {
            return username + "@" + hostname
        }
        return hostname
    }

    override func protocolName() -> String! {
        return "SSH Integration"
    }

    private var chunked = false

    func downloadChunked() -> Bool {
        chunked = true
        download()
        return status == .transferring
    }

    override func download() {
        state = .downloading
        status = .starting
        do {
            _localPath = try temporaryFilePath()
            FileTransferManager.sharedInstance().files.add(self)
            FileTransferManager.sharedInstance().transferrableFileDidStartTransfer(self)
            status = .transferring
            if !chunked {
                delegate?.beginDownload(fileTransfer: self)
            }
        } catch {
            state = .failed
        }
    }

    private func temporaryFilePath() throws -> String {
        guard let downloads = FileManager.default.downloadsDirectory() else {
            throw ConductorFileTransferError("Unable to find Downloads folder")
        }
        let tempFileName = ".DashTerm2.\(UUID().uuidString)"
        return downloads.appendingPathComponent(tempFileName)
    }

    final class ConductorFileTransferError: NSObject, LocalizedError {
        private let reason: String
        init(_ reason: String) {
            self.reason = reason
        }
        override var description: String {
            get {
                return reason
            }
        }
        var errorDescription: String? {
            get {
                return reason
            }
        }
    }

    func fail(reason: String) {
        _error = reason
        FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: ConductorFileTransferError(reason))
        state = .failed
    }

    // BUG-1677: Use optional URL to avoid force unwrap crash
    private var url: URL? {
        let components = NSURLComponents()
        components.host = path.hostname
        components.user = path.username
        components.path = path.path
        components.scheme = "ssh"
        return components.url
    }

    @MainActor
    func didTransferBytes(_ count: UInt) {
        self.bytesTransferred += count
        if status == .transferring {
            FileTransferManager.sharedInstance().transferrableFileProgressDidChange(self)
        }
    }

    @MainActor
    func abort() {
        FileTransferManager.sharedInstance().transferrableFileDidStopTransfer(self)
    }

    @MainActor
    func didFinishSuccessfully() {
        if state == .downloading {
            // BUG-1677: Use guard instead of force unwraps for _localPath, url, and path.path
            guard let localPath = _localPath else {
                _error = "No local path"
                FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: ConductorFileTransferError(_error))
                return
            }
            guard let sourceURL = url else {
                _error = "Invalid source URL"
                FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: ConductorFileTransferError(_error))
                return
            }
            guard let remotePath = path.path else {
                _error = "Invalid remote path"
                FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: ConductorFileTransferError(_error))
                return
            }
            if !quarantine(localPath, sourceURL: sourceURL) {
                _error = "Failed to quarantine"
                FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: ConductorFileTransferError(_error))
                return
            }
            guard let attributes = try? FileManager.default.attributesOfItem(atPath: localPath) else {
                _error = "Could not get attributes of \(localPath)"
                FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: ConductorFileTransferError(_error))
                return
            }
            let size = attributes[FileAttributeKey.size] as? UInt ?? 0
            self.bytesTransferred = size
            self.fileSize = Int(size)

            guard let finalDestination = self.finalDestination(
                forPath: remotePath.lastPathComponent,
                destinationDirectory: localPath.deletingLastPathComponent,
                prompt: true) else {
                _error = "Could not determine final destination"
                FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: ConductorFileTransferError(_error))
                return
            }
            do {
                if FileManager.default.fileExists(atPath: finalDestination) {
                    try FileManager.default.replaceItem(at: URL(fileURLWithPath: finalDestination),
                                                        withItemAt: URL(fileURLWithPath: localPath),
                                                        backupItemName: nil,
                                                        resultingItemURL: nil)
                } else {
                    try FileManager.default.moveItem(at: URL(fileURLWithPath: localPath),
                                                     to: URL(fileURLWithPath: finalDestination))
                }
            } catch {
                _error = error.localizedDescription
            }
            try? FileManager.default.removeItem(at: URL(fileURLWithPath: localPath))
            _localPath = finalDestination
            state = .downloadComplete
            FileTransferManager.sharedInstance().transferrableFile(
                self,
                didFinishTransmissionWithError: nil)
        } else if state == .uploading {
            state = .uploadComplete
            FileTransferManager.sharedInstance().transferrableFile(
                self,
                didFinishTransmissionWithError: nil)
        }
    }

    // Name for uploads once established.
    var remoteName: String?
    // BUG-1677: Use nil-coalescing instead of force unwrap for _localPath
    override func destination() -> String! {
        switch state {
        case .downloading, .downloadComplete:
            return _localPath ?? path.path
        case .uploading, .uploadComplete:
            return remoteName ?? path.path
        case .failed, .idle:
            return path.path
        }
    }

    private func sizeToUpload() -> Int? {
        if let data {
            // BUG-2769: Validate in-memory data size to prevent memory exhaustion
            if data.count > ConductorFileTransfer.maxInMemoryDataSize {
                _error = "Data size \(data.count) exceeds maximum allowed \(ConductorFileTransfer.maxInMemoryDataSize) bytes"
                state = .failed
                FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: ConductorFileTransferError(_error))
                return nil
            }
            return data.count
        }
        // BUG-1677: Use guard instead of force unwrap for localPath()
        guard let path = localPath() else {
            return nil
        }
        do {
            let attrs = try FileManager.default.attributesOfItem(atPath: path)
            guard let size = attrs[FileAttributeKey.size] as? Int else {
                _error = "Could not get size of file: \(path)"
                state = .failed
                FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: ConductorFileTransferError(_error))
                return nil
            }
            // BUG-2769: Validate file size to prevent resource exhaustion
            if Int64(size) > ConductorFileTransfer.maxFileSize {
                _error = "File size \(size) exceeds maximum allowed \(ConductorFileTransfer.maxFileSize) bytes"
                state = .failed
                FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: ConductorFileTransferError(_error))
                return nil
            }
            return size
        } catch {
            _error = "No such file: \(path)"
            FileTransferManager.sharedInstance().transferrableFile(self, didFinishTransmissionWithError: error)
            state = .failed
            return nil
        }
    }

    override func upload() {
        state = .uploading
        status = .starting
        if let size = sizeToUpload() {
            fileSize = size
        } else {
            return
        }
        FileTransferManager.sharedInstance().files.add(self)
        FileTransferManager.sharedInstance().transferrableFileDidStartTransfer(self)
        status = .transferring
        delegate?.beginUpload(fileTransfer: self)
    }

    override func isDownloading() -> Bool {
        return state == .downloading
    }

    var isStopped: Bool {
        switch state {
        case .downloading, .uploading:
            return false
        default:
            return true
        }
    }

    override func stop() {
        FileTransferManager.sharedInstance().transferrableFileWillStop(self)
        state = .failed
    }

    override func error() -> String! {
        return _error
    }

    override func localPath() -> String! {
        if data != nil {
            return "(In memory)"
        }
        return _localPath
    }
}
