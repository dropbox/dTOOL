//
//  DTermFeedbackWindowController.swift
//  DashTerm2
//
//  UI for submitting feedback and crash reports to GitHub.
//

import Cocoa

class DTermFeedbackWindowController: NSWindowController {
    private let feedbackType: DTermFeedbackType
    private var prefilledCrashReport: String?

    // UI Elements
    private var titleField: NSTextField!
    private var descriptionScrollView: NSScrollView!
    private var descriptionView: NSTextView!
    private var crashLogLabel: NSTextField?
    private var crashLogPopup: NSPopUpButton?
    private var crashLogScrollView: NSScrollView?
    private var crashLogView: NSTextView?
    private var submitButton: NSButton!
    private var cancelButton: NSButton!
    private var progressIndicator: NSProgressIndicator!
    private var statusLabel: NSTextField!

    private var crashLogs: [URL] = []
    private var selectedCrashLog: URL?

    init(feedbackType: DTermFeedbackType, prefilledCrashReport: String? = nil) {
        self.feedbackType = feedbackType
        self.prefilledCrashReport = prefilledCrashReport

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 600, height: feedbackType == .crash ? 600 : 400),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = feedbackType == .feedback ? "Send Feedback" : "Report Crash"
        window.center()
        window.minSize = NSSize(width: 500, height: 350)

        super.init(window: window)

        setupUI()

        if feedbackType == .crash {
            loadCrashLogs()
            // Pre-fill the crash log view if provided
            if let report = prefilledCrashReport {
                crashLogView?.string = String(report.prefix(50000))
                crashLogPopup?.addItem(withTitle: "(Pre-filled crash report)")
                crashLogPopup?.selectItem(withTitle: "(Pre-filled crash report)")
            }
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        it_fatalError("init(coder:) has not been implemented")
    }

    private func setupUI() {
        guard let contentView = window?.contentView else { return }

        let margin: CGFloat = 20
        var currentY = contentView.bounds.height - margin

        // Title label
        let titleLabel = NSTextField(labelWithString: feedbackType == .feedback ? "Title:" : "Summary:")
        titleLabel.frame = NSRect(x: margin, y: currentY - 20, width: 100, height: 17)
        contentView.addSubview(titleLabel)
        currentY -= 25

        // Title field
        titleField = NSTextField()
        titleField.frame = NSRect(x: margin, y: currentY - 24, width: contentView.bounds.width - 2 * margin, height: 24)
        titleField.placeholderString = feedbackType == .feedback ? "Brief description of your feedback" : "Brief description of the crash"
        titleField.autoresizingMask = [.width]
        contentView.addSubview(titleField)
        currentY -= 35

        // Description label
        let descLabel = NSTextField(labelWithString: "Description:")
        descLabel.frame = NSRect(x: margin, y: currentY - 17, width: 100, height: 17)
        contentView.addSubview(descLabel)
        currentY -= 25

        // Description text view
        let descHeight: CGFloat = feedbackType == .crash ? 100 : 200
        descriptionScrollView = NSScrollView()
        descriptionScrollView.frame = NSRect(x: margin, y: currentY - descHeight, width: contentView.bounds.width - 2 * margin, height: descHeight)
        descriptionScrollView.hasVerticalScroller = true
        descriptionScrollView.borderType = .bezelBorder
        descriptionScrollView.autoresizingMask = [.width, .height]

        descriptionView = NSTextView()
        descriptionView.minSize = NSSize(width: 0, height: descHeight - 4)
        descriptionView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        descriptionView.isVerticallyResizable = true
        descriptionView.isHorizontallyResizable = false
        descriptionView.autoresizingMask = [.width]
        descriptionView.textContainer?.containerSize = NSSize(width: descriptionScrollView.contentSize.width, height: CGFloat.greatestFiniteMagnitude)
        descriptionView.textContainer?.widthTracksTextView = true
        descriptionView.font = NSFont.systemFont(ofSize: 13)

        descriptionScrollView.documentView = descriptionView
        contentView.addSubview(descriptionScrollView)
        currentY -= (descHeight + 15)

        // Crash log section (only for crash reports)
        if feedbackType == .crash {
            crashLogLabel = NSTextField(labelWithString: "Crash Log:")
            crashLogLabel?.frame = NSRect(x: margin, y: currentY - 17, width: 100, height: 17)
            contentView.addSubview(crashLogLabel!)
            currentY -= 25

            crashLogPopup = NSPopUpButton()
            crashLogPopup?.frame = NSRect(x: margin, y: currentY - 26, width: contentView.bounds.width - 2 * margin, height: 26)
            crashLogPopup?.target = self
            crashLogPopup?.action = #selector(crashLogSelected(_:))
            crashLogPopup?.autoresizingMask = [.width]
            contentView.addSubview(crashLogPopup!)
            currentY -= 35

            let crashHeight: CGFloat = 150
            crashLogScrollView = NSScrollView()
            crashLogScrollView?.frame = NSRect(x: margin, y: currentY - crashHeight, width: contentView.bounds.width - 2 * margin, height: crashHeight)
            crashLogScrollView?.hasVerticalScroller = true
            crashLogScrollView?.borderType = .bezelBorder
            crashLogScrollView?.autoresizingMask = [.width]

            crashLogView = NSTextView()
            crashLogView?.minSize = NSSize(width: 0, height: crashHeight - 4)
            crashLogView?.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
            crashLogView?.isVerticallyResizable = true
            crashLogView?.isHorizontallyResizable = false
            crashLogView?.autoresizingMask = [.width]
            crashLogView?.textContainer?.containerSize = NSSize(width: crashLogScrollView!.contentSize.width, height: CGFloat.greatestFiniteMagnitude)
            crashLogView?.textContainer?.widthTracksTextView = true
            crashLogView?.isEditable = false
            crashLogView?.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)

            crashLogScrollView?.documentView = crashLogView
            contentView.addSubview(crashLogScrollView!)
            currentY -= (crashHeight + 15)
        }

        // Status label
        statusLabel = NSTextField(labelWithString: "")
        statusLabel.frame = NSRect(x: margin, y: 55, width: contentView.bounds.width - 2 * margin - 200, height: 17)
        statusLabel.textColor = .secondaryLabelColor
        statusLabel.font = NSFont.systemFont(ofSize: 11)
        statusLabel.autoresizingMask = [.width]
        contentView.addSubview(statusLabel)

        // Button area at bottom
        let buttonY: CGFloat = 20

        // Progress indicator
        progressIndicator = NSProgressIndicator()
        progressIndicator.style = .spinning
        progressIndicator.frame = NSRect(x: margin, y: buttonY, width: 20, height: 20)
        progressIndicator.isHidden = true
        contentView.addSubview(progressIndicator)

        // Cancel button
        cancelButton = NSButton(title: "Cancel", target: self, action: #selector(cancel(_:)))
        cancelButton.bezelStyle = .rounded
        cancelButton.frame = NSRect(x: contentView.bounds.width - margin - 80 - 10 - 120, y: buttonY, width: 80, height: 32)
        cancelButton.autoresizingMask = [.minXMargin]
        contentView.addSubview(cancelButton)

        // Submit button
        submitButton = NSButton(title: "Submit to GitHub", target: self, action: #selector(submit(_:)))
        submitButton.bezelStyle = .rounded
        submitButton.keyEquivalent = "\r"
        submitButton.frame = NSRect(x: contentView.bounds.width - margin - 120, y: buttonY, width: 120, height: 32)
        submitButton.autoresizingMask = [.minXMargin]
        contentView.addSubview(submitButton)
    }

    private func loadCrashLogs() {
        crashLogs = DTermFeedbackReporter.shared.findRecentCrashLogs()

        crashLogPopup?.removeAllItems()

        if crashLogs.isEmpty {
            crashLogPopup?.addItem(withTitle: "No crash logs found")
            crashLogPopup?.isEnabled = false
        } else {
            let dateFormatter = DateFormatter()
            dateFormatter.dateStyle = .short
            dateFormatter.timeStyle = .short

            for (index, crash) in crashLogs.prefix(10).enumerated() {
                let attrs = try? FileManager.default.attributesOfItem(atPath: crash.path)
                let date = (attrs?[.modificationDate] as? Date) ?? Date()

                crashLogPopup?.addItem(withTitle: "\(crash.lastPathComponent) - \(dateFormatter.string(from: date))")
                crashLogPopup?.lastItem?.representedObject = crash

                if index == 0 {
                    selectCrashLog(crash)
                }
            }
        }
    }

    private func selectCrashLog(_ url: URL) {
        selectedCrashLog = url
        if let content = try? String(contentsOf: url, encoding: .utf8) {
            // Show first 50KB
            crashLogView?.string = String(content.prefix(50000))
        } else {
            crashLogView?.string = "Unable to read crash log"
        }
    }

    @objc private func crashLogSelected(_ sender: NSPopUpButton) {
        if let url = sender.selectedItem?.representedObject as? URL {
            selectCrashLog(url)
        }
    }

    @objc private func cancel(_ sender: Any) {
        close()
    }

    @objc private func submit(_ sender: Any) {
        let title = titleField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty else {
            showAlert(message: "Please enter a title")
            return
        }

        setSubmitting(true)
        statusLabel.stringValue = "Submitting to GitHub..."

        if feedbackType == .crash {
            let crashLog = crashLogView?.string ?? ""
            let description = descriptionView.string

            DTermFeedbackReporter.shared.submitCrashReport(
                crashLog: crashLog,
                userDescription: description
            ) { [weak self] result in
                self?.handleSubmitResult(result)
            }
        } else {
            let body = """
            ## Feedback

            \(descriptionView.string)

            ### System Information
            - **App Version:** \(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") ?? "unknown")
            - **macOS:** \(ProcessInfo.processInfo.operatingSystemVersionString)

            ---
            *Submitted via DashTerm2 feedback reporter*
            """

            DTermFeedbackReporter.shared.submitFeedback(
                title: "[Feedback] \(title)",
                body: body,
                labels: ["feedback", "user-submitted"]
            ) { [weak self] result in
                self?.handleSubmitResult(result)
            }
        }
    }

    private func handleSubmitResult(_ result: Result<String, Error>) {
        setSubmitting(false)

        switch result {
        case .success(let issueURL):
            statusLabel.stringValue = "Issue created successfully!"
            showAlert(
                message: "Issue created successfully!\n\n\(issueURL)",
                isError: false,
                completion: { [weak self] in
                    self?.close()
                }
            )
        case .failure(let error):
            statusLabel.stringValue = "Failed to submit"
            showAlert(message: "Failed to submit: \(error.localizedDescription)")
        }
    }

    private func setSubmitting(_ submitting: Bool) {
        submitButton.isEnabled = !submitting
        cancelButton.isEnabled = !submitting
        titleField.isEnabled = !submitting
        descriptionView.isEditable = !submitting
        crashLogPopup?.isEnabled = !submitting && !crashLogs.isEmpty
        progressIndicator.isHidden = !submitting
        if submitting {
            progressIndicator.startAnimation(nil)
        } else {
            progressIndicator.stopAnimation(nil)
        }
    }

    private func showAlert(message: String, isError: Bool = true, completion: (() -> Void)? = nil) {
        let alert = NSAlert()
        alert.messageText = isError ? "Error" : "Success"
        alert.informativeText = message
        alert.alertStyle = isError ? .warning : .informational

        if let window = self.window {
            alert.beginSheetModal(for: window) { _ in
                completion?()
            }
        } else {
            alert.runModal()
            completion?()
        }
    }
}
