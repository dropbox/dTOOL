//
//  ChatViewControllerModel.swift
//  DashTerm2
//
//  Created by George Nachman on 2/24/25.
//

protocol ChatViewControllerModelDelegate: AnyObject {
    func chatViewControllerModel(didInsertItemAtIndex: Int)
    func chatViewControllerModel(didRemoveItemsInRange range: Range<Int>)
    func chatViewControllerModel(didModifyItemsAtIndexes indexSet: IndexSet)
}

class ChatViewControllerModel {
    weak var delegate: ChatViewControllerModelDelegate?
    private let listModel: ChatListModel
    // Avoid streaming so quickly that we bog down recalculating textview geometry and parsing markdown.
    private let rateLimit = iTermRateLimitedUpdate(name: "reloadCell", minimumInterval: 1)
    private var pendingItemIdentities = Set<ChatViewControllerModel.Item.Identity>()
    var lastStreamingState = ClientLocal.Action.StreamingState.stopped

    enum Item: CustomDebugStringConvertible {
        var debugDescription: String {
            switch self {
            case .message(let message): "<Message: \(message.message.content.shortDescription), pending: \(message.pending?.content.shortDescription ?? "(nil)")>"
            case .date(let date): "<Date: \(date)>"
            case .agentTyping: "<AgentTyping>"
            }
        }

        class UpdatableMessage {
            private(set) var message: Message
            var pending: Message?

            init(_ message: Message) {
                self.message = message
            }
            func commit() {
                if let pending {
                    message = pending
                }
            }
        }
        case message(UpdatableMessage)
        case date(DateComponents)
        case agentTyping

        enum Identity: Hashable {
            case message(UUID)
            case date(DateComponents)
            case agentTyping
        }

        var identity: Identity {
            switch self {
            case .message(let message): .message(message.message.uniqueID)
            case .date(let date): .date(date)
            case .agentTyping: .agentTyping
            }
        }

        var hasButtons: Bool {
            guard case .message(let message) = self else {
                return false
            }
            return !message.message.buttons.isEmpty
        }

        var existingMessage: UpdatableMessage? {
            switch self {
            case .message(let existing): existing
            default: nil
            }
        }
    }

    private(set) var items = NotifyingArray<Item>()
    private let chatID: String

    var showTypingIndicator = false {
        didSet {
            if showTypingIndicator == oldValue {
                return
            }
            if showTypingIndicator {
                items.append(.agentTyping)
            } else if case .agentTyping = items.last {
                items.removeLast()
            }
        }
    }

    var terminalSessionGuid: String? {
        get {
            listModel.chat(id: chatID)?.terminalSessionGuid
        }
    }

    var browserSessionGuid: String? {
        get {
            listModel.chat(id: chatID)?.browserSessionGuid
        }
    }

    func setTerminalSessionGuid(_ newValue: String?) throws {
        try listModel.setTerminalGuid(for: chatID, to: newValue)
    }

    func setBrowserSessionGuid(_ newValue: String?) throws {
        try listModel.setBrowserGuid(for: chatID, to: newValue)
    }

    private let alwaysAppendDate = false

    static func assertMessageTypeAllowed(_ message: Message?) {
        switch message {
        case .none:
            DLog("Nil message in model")
            break
        case .some(let justMessage):
            switch justMessage.content {
            case .userCommand:
                // BUG-f558: Log error instead of crashing - userCommand should not be in model
                DLog("ERROR: user command not allowed in model - ignoring")
            case .append, .appendAttachment:
                // BUG-f559: Log error instead of crashing - append types should not be in model
                DLog("ERROR: Append type messages not allowed in model - ignoring")
            case .plainText, .markdown, .explanationRequest, .explanationResponse,
                    .remoteCommandRequest, .remoteCommandResponse, .selectSessionRequest,
                    .clientLocal, .renameChat, .commit, .setPermissions, .terminalCommand,
                    .multipart, .vectorStoreCreated:
                return
            }
        }
    }

    init(chat: Chat, listModel: ChatListModel) {
        self.listModel = listModel
        chatID = chat.id
        var lastDate: DateComponents?
        if let messages = listModel.messages(forChat: chatID, createIfNeeded: false) {
            for message in messages {
                if message.hiddenFromClient {
                    continue
                }
                let date = message.dateErasingTime
                if alwaysAppendDate || lastDate != date {
                    items.append(.date(date))
                }
                ChatViewControllerModel.assertMessageTypeAllowed(message)
                items.append(.message(Item.UpdatableMessage(message)))
                lastDate = Calendar.current.dateComponents([.year, .month, .day], from: message.sentDate)
                if case .clientLocal(let cl) = message.content,
                   case .streamingChanged(let state) = cl.action {
                    lastStreamingState = state
                }
            }
        }
        initializeItemsDelegate()
    }

    private func initializeItemsDelegate() {
        items.didInsert = { [weak self] i in
            self?.delegate?.chatViewControllerModel(didInsertItemAtIndex: i)
        }
        items.didRemove = { [weak self] range in
            self?.delegate?.chatViewControllerModel(didRemoveItemsInRange: range)
        }
        items.didModify = { [weak self] i in
            self?.delegate?.chatViewControllerModel(didModifyItemsAtIndexes: IndexSet(integer: i))
        }
    }

    private func scheduleCommit(_ item: Item) {
        if pendingItemIdentities.contains(item.identity) {
            return
        }
        pendingItemIdentities.insert(item.identity)
        rateLimit.performRateLimitedBlock { [weak self] in
            guard let self else {
                return
            }
            let indexes = pendingItemIdentities.compactMap {
                self.index(of: $0)
            }
            pendingItemIdentities.removeAll()
            guard !indexes.isEmpty else {
                return
            }
            for i in indexes {
                // BUG-f993: Use safe subscript to avoid crash if index is stale
                guard let item = self.items[safe: i] else {
                    DLog("WARNING: scheduleCommit: stale index \(i) in items array")
                    continue
                }
                if case .message(let message) = item {
                    message.commit()
                }
            }
            delegate?.chatViewControllerModel(didModifyItemsAtIndexes: IndexSet(indexes))
        }
    }

    private func didAppend(toMessageID messageID: UUID) {
        // BUG-f993: Use safe subscript to avoid crash if index becomes stale
        if let i = index(ofMessageID: messageID),
           let item = items[safe: i],
           let existing = item.existingMessage,
           let canonicalMessages = listModel.messages(forChat: chatID, createIfNeeded: false),
           let updated = canonicalMessages.firstIndex(where: { $0.uniqueID == messageID }) {
            // Streaming update. Place modified message in second position so rate limited
            // updates can be applied atomically.
            existing.pending = canonicalMessages[updated]
            scheduleCommit(item)
        }
    }

    func appendMessage(_ message: Message) {
        switch message.content {
        case .append(string: _, uuid: let uuid), .appendAttachment(attachment: _, uuid: let uuid):
            didAppend(toMessageID: uuid)
            return
        case .explanationResponse(_, let update, markdown: _):
            if let messageID = update?.messageID {
                didAppend(toMessageID: messageID)
                return
            }
        case .plainText, .markdown, .explanationRequest, .remoteCommandRequest,
                .remoteCommandResponse, .selectSessionRequest, .clientLocal, .renameChat, .commit,
                .setPermissions, .terminalCommand, .multipart, .vectorStoreCreated,
                .userCommand:
            break
        }
        let saved = showTypingIndicator
        showTypingIndicator = false
        defer {
            showTypingIndicator = saved
        }
        if let last = items.last,
           case .message(let lastMessage) = last,
           (alwaysAppendDate || message.dateErasingTime != lastMessage.message.dateErasingTime) {
            items.append(.date(message.dateErasingTime))
        }
        Self.assertMessageTypeAllowed(message)
        items.append(.message(Item.UpdatableMessage(message)))
    }

    func commit() {
        rateLimit.force()
    }

    func index(of identity: Item.Identity) -> Int? {
        return items.firstIndex {
            $0.identity == identity
        }
    }

    // Returns true for all messages message[j] for j>i, test(message[j]) is false. Returns true if there are no messages after i.
    private func indexIsLastMessage(_ i: Int, passingTest test: (Message) -> Bool) -> Bool {
        // BUG-f993: Use safe subscript to avoid crash if index is invalid
        guard let item = items[safe: i], case .message = item else {
            return false
        }
        for j in (i + 1)..<items.count {
            // BUG-f993: Use safe subscript for inner loop access
            guard let innerItem = items[safe: j] else {
                continue
            }
            if case .message(let message) = innerItem, test(message.message) {
                return false
            }
        }
        return true
    }

    func indexIsLastMessage(_ i: Int) -> Bool {
        return indexIsLastMessage(i, passingTest: { _ in true })
    }

    func index(ofMessageID messageID: UUID) -> Int? {
        return items.firstIndex {
            switch $0 {
            case .message(let candidate):
                return candidate.message.uniqueID == messageID
            default:
                return false
            }
        }
    }

    func deleteFrom(index i: Int) {
        // BUG-f987: Guard against invalid index to prevent crash
        // If i is negative or greater than items.count, items.count - i could be
        // negative (UB) or greater than array size (crash)
        guard i >= 0 && i <= items.count else {
            DLog("ERROR: deleteFrom called with invalid index \(i) for items.count \(items.count)")
            return
        }
        let countToRemove = items.count - i
        guard countToRemove > 0 else {
            // Nothing to delete
            return
        }
        let removed = items.removeLast(countToRemove)
        let messageIDs = removed.compactMap {
            switch $0 {
            case .message(let message):
                message.message.uniqueID
            case .date, .agentTyping:
                nil
            }
        }
        listModel.delete(chatID: chatID, messageIDs: messageIDs)
    }
}

extension Message {
    var dateErasingTime: DateComponents {
        Calendar.current.dateComponents([.year, .month, .day], from: sentDate)
    }
}
