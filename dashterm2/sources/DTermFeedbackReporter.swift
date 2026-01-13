//
//  DTermFeedbackReporter.swift
//  DashTerm2
//
//  Crash and feedback reporting via GitHub Issues using gh CLI.
//

import Foundation
import AppKit

@objc enum DTermFeedbackType: Int {
    case feedback = 0
    case crash = 1
}

@objc class DTermFeedbackReporter: NSObject {
    @objc static let shared = DTermFeedbackReporter()

    private let repository = "ayates_dbx/dashterm2"

    // MARK: - Public Interface

    @objc func showFeedbackWindow(with type: DTermFeedbackType) {
        let controller = DTermFeedbackWindowController(feedbackType: type)
        controller.showWindow(nil)
    }

    /// Show feedback window pre-filled with a crash report.
    /// Called from iTermCrashReportIconController when user clicks crash icon.
    @objc func showFeedbackWindow(withCrashReport report: String) {
        let controller = DTermFeedbackWindowController(feedbackType: .crash, prefilledCrashReport: report)
        controller.showWindow(nil)
    }

    // MARK: - GitHub Issue Creation

    func submitFeedback(
        title: String,
        body: String,
        labels: [String],
        completion: @escaping (Result<String, Error>) -> Void
    ) {
        // Check if gh CLI is available
        guard ghCLIAvailable() else {
            completion(.failure(FeedbackError.ghNotInstalled))
            return
        }

        // Check if gh is authenticated
        guard ghAuthenticated() else {
            completion(.failure(FeedbackError.ghNotAuthenticated))
            return
        }

        // Build gh issue create command
        var args = ["issue", "create", "--repo", repository, "--title", title, "--body", body]
        for label in labels {
            args.append(contentsOf: ["--label", label])
        }

        runGHCommand(args: args, completion: completion)
    }

    func submitCrashReport(
        crashLog: String,
        userDescription: String,
        completion: @escaping (Result<String, Error>) -> Void
    ) {
        let systemInfo = collectSystemInfo()
        let crashSummary = extractCrashSummary(from: crashLog)
        let title = "[Crash Report] \(crashSummary)"

        // Truncate crash log if too long (GitHub has body size limits)
        let truncatedLog = String(crashLog.prefix(50000))

        let body = """
        ## Crash Report

        ### User Description
        \(userDescription.isEmpty ? "No description provided" : userDescription)

        ### System Information
        \(systemInfo)

        ### Crash Log
        ```
        \(truncatedLog)
        ```

        ---
        *Submitted via DashTerm2 crash reporter*
        """

        submitFeedback(title: title, body: body, labels: ["crash", "auto-reported"], completion: completion)
    }

    // MARK: - Crash Log Collection

    func findRecentCrashLogs() -> [URL] {
        let diagnosticReports = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Logs/DiagnosticReports")

        // Look for both DashTerm2 and iTerm2 crashes (legacy name might appear)
        let patterns = ["DashTerm2", "iTerm2"]
        var crashes: [URL] = []

        guard let contents = try? FileManager.default.contentsOfDirectory(
            at: diagnosticReports,
            includingPropertiesForKeys: [.contentModificationDateKey],
            options: []
        ) else { return [] }

        for file in contents {
            let name = file.lastPathComponent
            for pattern in patterns {
                if name.hasPrefix(pattern) && (name.hasSuffix(".ips") || name.hasSuffix(".crash")) {
                    crashes.append(file)
                    break
                }
            }
        }

        // Sort by modification date, newest first
        return crashes.sorted { url1, url2 in
            let date1 = (try? url1.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate ?? .distantPast
            let date2 = (try? url2.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate ?? .distantPast
            return date1 > date2
        }
    }

    // MARK: - Private Helpers

    private func ghCLIAvailable() -> Bool {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        task.arguments = ["gh"]
        task.standardOutput = FileHandle.nullDevice
        task.standardError = FileHandle.nullDevice

        do {
            try task.run()
            task.waitUntilExit()
            return task.terminationStatus == 0
        } catch {
            return false
        }
    }

    private func ghAuthenticated() -> Bool {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        task.arguments = ["gh", "auth", "status"]
        task.standardOutput = FileHandle.nullDevice
        task.standardError = FileHandle.nullDevice

        do {
            try task.run()
            task.waitUntilExit()
            return task.terminationStatus == 0
        } catch {
            return false
        }
    }

    private func findGHPath() -> String {
        // Try common locations
        let paths = [
            "/opt/homebrew/bin/gh",
            "/usr/local/bin/gh",
            "/usr/bin/gh"
        ]

        for path in paths {
            if FileManager.default.fileExists(atPath: path) {
                return path
            }
        }

        // Fall back to using env to find it
        return "/usr/bin/env"
    }

    private func runGHCommand(args: [String], completion: @escaping (Result<String, Error>) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }

            let task = Process()
            let ghPath = self.findGHPath()

            if ghPath == "/usr/bin/env" {
                task.executableURL = URL(fileURLWithPath: ghPath)
                task.arguments = ["gh"] + args
            } else {
                task.executableURL = URL(fileURLWithPath: ghPath)
                task.arguments = args
            }

            let outputPipe = Pipe()
            let errorPipe = Pipe()
            task.standardOutput = outputPipe
            task.standardError = errorPipe

            do {
                try task.run()
                task.waitUntilExit()

                let outputData = outputPipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: outputData, encoding: .utf8) ?? ""

                if task.terminationStatus == 0 {
                    // Extract issue URL from output
                    DispatchQueue.main.async {
                        completion(.success(output.trimmingCharacters(in: .whitespacesAndNewlines)))
                    }
                } else {
                    let errorData = errorPipe.fileHandleForReading.readDataToEndOfFile()
                    let errorMsg = String(data: errorData, encoding: .utf8) ?? "Unknown error"
                    DispatchQueue.main.async {
                        completion(.failure(FeedbackError.ghCommandFailed(errorMsg)))
                    }
                }
            } catch {
                DispatchQueue.main.async {
                    completion(.failure(error))
                }
            }
        }
    }

    private func collectSystemInfo() -> String {
        let processInfo = ProcessInfo.processInfo
        let bundle = Bundle.main

        let version = bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"
        let build = bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "unknown"

        return """
        - **App Version:** \(version) (\(build))
        - **macOS Version:** \(processInfo.operatingSystemVersionString)
        - **Machine:** \(getMachineModel())
        - **Memory:** \(processInfo.physicalMemory / 1_073_741_824) GB
        """
    }

    private func getMachineModel() -> String {
        var size = 0
        sysctlbyname("hw.model", nil, &size, nil, 0)
        var model = [CChar](repeating: 0, count: size)
        sysctlbyname("hw.model", &model, &size, nil, 0)
        return String(cString: model)
    }

    private func extractCrashSummary(from log: String) -> String {
        // Try to extract exception type
        if let range = log.range(of: "Exception Type:") {
            let start = log.index(range.upperBound, offsetBy: 1, limitedBy: log.endIndex) ?? range.upperBound
            if let end = log[start...].firstIndex(of: "\n") {
                return String(log[start..<end]).trimmingCharacters(in: .whitespaces)
            }
        }

        // Try to extract crashed thread info
        if let range = log.range(of: "Crashed Thread:") {
            let start = log.index(range.upperBound, offsetBy: 1, limitedBy: log.endIndex) ?? range.upperBound
            if let end = log[start...].firstIndex(of: "\n") {
                return "Thread " + String(log[start..<end]).trimmingCharacters(in: .whitespaces)
            }
        }

        return "Application Crash"
    }

    enum FeedbackError: LocalizedError {
        case ghNotInstalled
        case ghNotAuthenticated
        case ghCommandFailed(String)

        var errorDescription: String? {
            switch self {
            case .ghNotInstalled:
                return "GitHub CLI (gh) is not installed. Install with: brew install gh"
            case .ghNotAuthenticated:
                return "GitHub CLI is not authenticated. Run: gh auth login"
            case .ghCommandFailed(let msg):
                return "GitHub command failed: \(msg)"
            }
        }
    }
}
