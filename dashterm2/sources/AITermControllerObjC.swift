//
//  AITermControllerObjC.swift
//  DashTerm2
//
//  Created by George Nachman on 6/5/25.
//


@objc
class AITermControllerObjC: NSObject, AITermControllerDelegate, iTermObject {
    private struct CachedKey {
        var valid = false
        var value: String?
    }
    private let controller: AITermController
    private var handler: ((Result<String, Error>) -> ())?
    private let ownerWindow: NSWindow
    private let query: String
    private let pleaseWait: PleaseWaitWindow
    private static let apiKeyQueue = DispatchQueue(label: "com.dashterm2.aiterm-set-key")
    // BUG-5: Keychain service names updated to DashTerm2
    private static let apiKeyService = "DashTerm2 API Keys"
    private static let apiKeyAccount = "OpenAI API Key for DashTerm2"
    private static let legacyService = "DashTerm2 API Keys"
    private static let legacyAccount = "OpenAI API Key for DashTerm2"
    private static var shouldMigrateLegacyAPIKey: Bool {
        return legacyService != apiKeyService || legacyAccount != apiKeyAccount
    }
    private static var cachedKey = MutableAtomicObject(CachedKey())

    @objc static var haveCachedAPIKey: Bool {
        return cachedKey.value.valid
    }

    private static func readAPIKeyFromKeychain() -> String? {
        if let value = try? SSKeychain.password(forService: apiKeyService,
                                                account: apiKeyAccount) {
            return value
        }
        guard shouldMigrateLegacyAPIKey,
              let legacyValue = try? SSKeychain.password(forService: legacyService,
                                                         account: legacyAccount) else {
            return nil
        }
        // Best-effort migration so future reads use the new service/account pair.
        _ = SSKeychain.setPassword(legacyValue,
                                   forService: apiKeyService,
                                   account: apiKeyAccount)
        if shouldMigrateLegacyAPIKey {
            _ = SSKeychain.deletePassword(forService: legacyService,
                                           account: legacyAccount)
        }
        return legacyValue
    }

    private static func storeAPIKey(_ key: String?) {
        _ = SSKeychain.setPassword(key ?? "",
                                   forService: apiKeyService,
                                   account: apiKeyAccount)
        if shouldMigrateLegacyAPIKey {
            _ = SSKeychain.deletePassword(forService: legacyService,
                                           account: legacyAccount)
        }
    }

    @objc static var apiKey: String? {
        get {
            if cachedKey.value.valid {
                return cachedKey.value.value
            }
            return apiKeyQueue.sync {
                if !cachedKey.value.valid {
                    let value = readAPIKeyFromKeychain()
                    cachedKey.set(CachedKey(valid: true, value: value))
                }
                return cachedKey.value.value
            }
        }
        set {
            cachedKey.set(CachedKey(valid: true, value: newValue))
            apiKeyQueue.sync {
                cachedKey.set(CachedKey(valid: true, value: newValue))
                storeAPIKey(newValue)
            }
        }
    }

    @objc static func setAPIKeyAsync(_ key: String?) {
        cachedKey.set(CachedKey(valid: true, value: key))
        apiKeyQueue.async {
            cachedKey.set(CachedKey(valid: true, value: key))
            storeAPIKey(key)
        }
    }

    // handler([…], nil): Valid response
    // handler(nil, …): Error
    // handler(nil, nil): User canceled
    @objc(initWithQuery:scope:inWindow:completion:)
    init(query: String,
         scope: iTermVariableScope,
         window: NSWindow,
         handler: @escaping (iTermOr<NSString, NSError>) -> ()) {
        // BUG-1765: Use nil coalescing with fallback instead of force unwrap
        let pleaseWait = PleaseWaitWindow(owningWindow: window,
                                          message: "Thinking…",
                                          image: NSImage.it_imageNamed("aiterm", for: AITermControllerObjC.self) ?? NSImage())
        self.pleaseWait = pleaseWait
        var cancel: (() -> ())?
        var shouldCancel = false
        self.handler = { result in
            if !pleaseWait.canceled {
                result.handle { choice in
                    handler(iTermOr.first(choice as NSString))
                } failure: { error in
                    handler(iTermOr.second(error as NSError))
                }
            } else {
                shouldCancel = true
                cancel?()
            }
        }
        pleaseWait.didCancel = {
            shouldCancel = true
            cancel?()
        }
        self.ownerWindow = window
        self.query = query

        let registration = AITermControllerRegistrationHelper.instance.registration
        controller = AITermController(registration: registration)
        super.init()

        controller.delegate = self

        let template = iTermPreferences.string(forKey: kPreferenceKeyAIPrompt) ?? ""
        let sanitizedPrompt = query.trimmingCharacters(in: .whitespacesAndNewlines)

        // BUG-1706: Use guard with as? instead of as! for scope.copy() cast
        guard let myScope = scope.copy() as? iTermVariableScope else {
            return
        }
        let frame = iTermVariables(context: [], owner: self)
        myScope.add(frame, toScopeNamed: "ai")
        myScope.setValue(sanitizedPrompt, forVariableNamed: "ai.prompt")
        let swiftyString = iTermSwiftyString(string: template, scope: myScope, sideEffectsAllowed: false)
        swiftyString.evaluateSynchronously(false, sideEffectsAllowed: false, with: myScope) { maybeResult, maybeError, _ in
            if let prompt = maybeResult {
                Timer.scheduledTimer(withTimeInterval: 0, repeats: false) { _ in
                    if !shouldCancel {
                        cancel = { [weak self] in
                            self?.controller.cancel()
                        }
                        self.controller.request(query: prompt)
                    }
                }
            }
        }
    }

    // Ensures handler will never be called.
    @objc func invalidate() {
        dispatchPrecondition(condition: .onQueue(.main))
        handler = nil
    }

    func aitermControllerWillSendRequest(_ sender: AITermController) {
        pleaseWait.run()
    }

    func aitermController(_ sender: AITermController, didStreamUpdate update: String?) {
        // BUG-f700: Don't crash if streaming is accidentally used with ObjC interface
        DLog("WARNING: Streaming not supported in the ObjC interface - ignoring stream update")
    }

    func aitermController(_ sender: AITermController, didStreamAttachment: LLM.Message.Attachment) {
        // BUG-f701: Don't crash if streaming attachment is received in ObjC interface
        DLog("WARNING: Streaming not supported in the ObjC interface - ignoring attachment")
    }

    func aitermController(_ sender: AITermController, offerChoice choice: String) {
        pleaseWait.stop()
        DispatchQueue.main.async {
            self.handler?(.success(choice))
        }
    }

    func aitermController(_ sender: AITermController, didFailWithError error: Error) {
        pleaseWait.stop()
        DispatchQueue.main.async {
            self.handler?(.failure(error))
        }

    }

    // BUG-f702 to BUG-f709: These delegate methods aren't used by the non-streaming ObjC interface.
    // They should not crash if accidentally called - just log and ignore.
    func aitermController(_ sender: AITermController, didCreateVectorStore id: String, withName name: String) {
        DLog("WARNING: Vector store created but not supported in ObjC interface")
    }

    func aitermControllerDidAddFileToVectorStore(_ sender: AITermController) {
        DLog("WARNING: File added to vector store but not supported in ObjC interface")
    }

    func aitermControllerDidFailToAddFileToVectorStore(_ sender: AITermController, error: any Error) {
        DLog("WARNING: Failed to add file to vector store: \(error)")
    }

    func aitermController(_ sender: AITermController, didFailToCreateVectorStoreWithError: any Error) {
        DLog("WARNING: Failed to create vector store: \(didFailToCreateVectorStoreWithError)")
    }

    func aitermController(_ sender: AITermController, didUploadFileWithID id: String) {
        DLog("WARNING: File uploaded but not supported in ObjC interface")
    }

    func aitermController(_ sender: AITermController, didFailToUploadFileWithError: any Error) {
        DLog("WARNING: Failed to upload file: \(didFailToUploadFileWithError)")
    }

    func aitermControllerDidAddFilesToVectorStore(_ sender: AITermController) {
        DLog("WARNING: Files added to vector store but not supported in ObjC interface")
    }

    func aitermControllerDidFailToAddFilesToVectorStore(_ sender: AITermController, error: any Error) {
        if error as? PluginError == PluginError.cancelled {
            return
        }
        DLog("WARNING: Failed to add files to vector store: \(error)")
    }

    func aitermController(_ sender: AITermController, willInvokeFunction function: any LLM.AnyFunction) {
    }

    func aitermControllerDidCancelOutstandingRequest(_ sender: AITermController) {
    }


    func aitermControllerRequestRegistration(_ sender: AITermController,
                                             completion: @escaping (AITermController.Registration) -> ()) {
        AITermControllerRegistrationHelper.instance.requestRegistration(in: ownerWindow) { [weak self] registration in
            guard let self else {
                return
            }
            if let registration {
                completion(registration)
            } else {
                handler?(.failure(AIError("AI features are not enabled or the API key is missing.")))
            }
        }
    }

    func objectMethodRegistry() -> iTermBuiltInFunctions? {
        return nil
    }

    func objectScope() -> iTermVariableScope? {
        return nil
    }
}
