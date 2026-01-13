@testable import DashTerm2SharedARC
import XCTest

private final class RemoteCommandTestSession: PTYSession {
    var stubCurrentCommand: String?

    override var currentCommandUpToCursor: String! {
        stubCurrentCommand
    }

    init(stubCommand: String?) {
        self.stubCurrentCommand = stubCommand
        super.init(synthetic: true)
    }

    required init!(coder: NSCoder!) {
        it_fatalError("init(coder:) has not been implemented")
    }
}

private final class FileCreationTestSession: PTYSession {
    private let stubWorkingDirectory: String
    var revealedFileURLs = [URL]()

    init(workingDirectory: URL) {
        self.stubWorkingDirectory = workingDirectory.path
        super.init(synthetic: true)
    }

    required init!(coder: NSCoder!) {
        it_fatalError("init(coder:) has not been implemented")
    }

    override var currentLocalWorkingDirectory: String! {
        stubWorkingDirectory
    }

    override func revealCreatedFile(at url: URL) {
        revealedFileURLs.append(url)
    }
}

final class RemoteCommandTests: XCTestCase {
    private let commandHistoryKey = "NoSyncCommandHistoryHasEverBeenUsed"
    private var historyKeyWasPresent = false
    private var previousHistoryValue = false

    override func setUp() {
        super.setUp()
        let defaults = UserDefaults.standard
        historyKeyWasPresent = defaults.object(forKey: commandHistoryKey) != nil
        if historyKeyWasPresent {
            previousHistoryValue = defaults.bool(forKey: commandHistoryKey)
        }
        defaults.set(true, forKey: commandHistoryKey)
    }

    override func tearDown() {
        let defaults = UserDefaults.standard
        if historyKeyWasPresent {
            defaults.set(previousHistoryValue, forKey: commandHistoryKey)
        } else {
            defaults.removeObject(forKey: commandHistoryKey)
        }
        super.tearDown()
    }

    func testGetCommandBeforeCursorReturnsCurrentCommand() throws {
        let session = RemoteCommandTestSession(stubCommand: "echo hello")
        var response: (text: String, detail: String)?
        try session.getCommandBeforeCursorRemoteCommand(getCommandBeforeCursor: RemoteCommand.GetCommandBeforeCursor()) { text, detail in
            response = (text, detail)
        }
        XCTAssertEqual(response?.text, "echo hello")
        XCTAssertEqual(response?.detail, "Current command provided to AI.")
    }

    func testGetCommandBeforeCursorWhenNotAtPrompt() throws {
        let session = RemoteCommandTestSession(stubCommand: nil)
        var response: (text: String, detail: String)?
        try session.getCommandBeforeCursorRemoteCommand(getCommandBeforeCursor: RemoteCommand.GetCommandBeforeCursor()) { text, detail in
            response = (text, detail)
        }
        XCTAssertEqual(response?.text, "The user is not at the prompt.")
        XCTAssertEqual(response?.detail, "The contents of the shell prompt could not be provided because it appears the session is not currently at a prompt.")
    }
}

final class RemoteCommandFileCreationTests: XCTestCase {
    private let fileManager = FileManager.default
    private var workingDirectory: URL!
    private var outsideDirectory: URL!

    override func setUp() {
        super.setUp()
        workingDirectory = makeTemporaryDirectory()
        outsideDirectory = makeTemporaryDirectory()
    }

    override func tearDown() {
        if let workingDirectory {
            try? fileManager.removeItem(at: workingDirectory)
        }
        if let outsideDirectory {
            try? fileManager.removeItem(at: outsideDirectory)
        }
        workingDirectory = nil
        outsideDirectory = nil
        super.tearDown()
    }

    func testCreateFileRejectsSymlinkEscapes() throws {
        let safeDir = workingDirectory.appendingPathComponent("safe", isDirectory: true)
        try fileManager.createDirectory(at: safeDir, withIntermediateDirectories: true)
        let symlink = safeDir.appendingPathComponent("link", isDirectory: true)
        try fileManager.createSymbolicLink(at: symlink, withDestinationURL: outsideDirectory)

        let session = FileCreationTestSession(workingDirectory: workingDirectory)
        var response: (text: String, detail: String)?
        try session.createFileCommand(createFile: RemoteCommand.CreateFile(filename: "safe/link/escape.txt",
                                                                          content: "secret")) { text, detail in
            response = (text: text, detail: detail)
        }

        XCTAssertEqual(response?.text, "Error: Invalid path")
        XCTAssertEqual(response?.detail, "Resolved path escapes base directory")
        let escapedPath = outsideDirectory.appendingPathComponent("escape.txt").path
        XCTAssertFalse(fileManager.fileExists(atPath: escapedPath))
        XCTAssertTrue(session.revealedFileURLs.isEmpty)
    }

    func testCreateFileWritesInsideWorkingDirectory() throws {
        let notesDir = workingDirectory.appendingPathComponent("notes", isDirectory: true)
        try fileManager.createDirectory(at: notesDir, withIntermediateDirectories: true)

        let session = FileCreationTestSession(workingDirectory: workingDirectory)
        let relativePath = "notes/result.txt"
        let expectedURL = notesDir.appendingPathComponent("result.txt")
        var response: (text: String, detail: String)?
        try session.createFileCommand(createFile: RemoteCommand.CreateFile(filename: relativePath,
                                                                          content: "safe output")) { text, detail in
            response = (text: text, detail: detail)
        }

        XCTAssertEqual(response?.text, "Ok")
        XCTAssertEqual(response?.detail, "Created notes/result.txt and revealed in Finder.")
        XCTAssertTrue(fileManager.fileExists(atPath: expectedURL.path))
        XCTAssertEqual(session.revealedFileURLs.first, expectedURL.resolvingSymlinksInPath())
        let stored = try String(contentsOf: expectedURL, encoding: .utf8)
        XCTAssertEqual(stored, "safe output")
    }

    private func makeTemporaryDirectory() -> URL {
        let url = fileManager.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        do {
            try fileManager.createDirectory(at: url, withIntermediateDirectories: true)
        } catch {
            XCTFail("Failed to create temporary directory: \(error)")
        }
        return url
    }
}
