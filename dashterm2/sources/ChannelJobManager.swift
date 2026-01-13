//
//  ChannelJobManager.swift
//  DashTerm2
//
//  Created by George Nachman on 4/14/25.
//

@objc(iTermChannelJobManager)
class ChannelJobManager: NSObject, iTermJobManager {
    @objc var ioBuffer: IOBuffer?
    var fd: Int32 = -1
    // DESIGN_LIMITATION: TTY name is hardcoded as "(embedded)" for channel-based jobs.
    // This could be plumbed through the protocol if needed for diagnostic purposes.
    var tty = "(embedded)"
    var externallyVisiblePid: pid_t { 0 }
    var hasJob: Bool { true }
    var sessionRestorationIdentifier: Any? { nil }
    var pidToWaitOn: pid_t { 0 }
    var isSessionRestorationPossible: Bool { false }
    var ioAllowed: Bool { ioBuffer?.isValid ?? false }
    var queue: dispatch_queue_t? { nil }
    var isReadOnly: Bool { true }
    static func available() -> Bool {
        true
    }

    // BUG-f576: Return failure status instead of crashing for unsupported operations
    func forkAndExec(with ttyState: iTermTTYState,
                     argpath: String!,
                     argv: [String]!,
                     initialPwd: String!,
                     newEnviron: [String]!,
                     task: (any iTermTask)!,
                     completion: ((iTermJobManagerForkAndExecStatus, NSNumber?) -> Void)!) {
        DLog("ChannelJobManager.forkAndExec() is not supported - returning failure")
        completion?(.failedToFork, nil)
    }

    // BUG-f577: Return failure results instead of crashing for unsupported attach operations
    func attach(toServer serverConnection: iTermGeneralServerConnection,
                withProcessID thePid: NSNumber!,
                task: (any iTermTask)!,
                completion: ((iTermJobManagerAttachResults) -> Void)!) {
        DLog("ChannelJobManager.attach(completion:) is not supported - returning empty results")
        completion?(iTermJobManagerAttachResults())
    }

    // BUG-f578: Return empty results instead of crashing for unsupported synchronous attach
    func attach(toServer serverConnection: iTermGeneralServerConnection,
                withProcessID thePid: NSNumber!,
                task: (any iTermTask)!) -> iTermJobManagerAttachResults {
        DLog("ChannelJobManager.attach() is not supported - returning empty results")
        return iTermJobManagerAttachResults()
    }

    // DESIGN_LIMITATION: Kill is not implemented for channel-based jobs.
    // Channel jobs don't have a real process to kill - they communicate over a protocol channel.
    // Supporting kill would require sending a termination message through the channel protocol.
    func kill(with mode: iTermJobManagerKillingMode) {
        DLog("Kill requested for channel job (mode: \(mode)) - not implemented")
    }

    func closeFileDescriptor() -> Bool {
        guard let ioBuffer, ioBuffer.isValid else {
            return false
        }
        ioBuffer.invalidate()
        return true
    }


    @objc(initWithQueue:)
    required init(queue: DispatchQueue) {
        super.init()
    }

    override var description: String {
        return "<\(Self.self): \(it_addressString) ioBuffer=\(ioBuffer.d)>"
    }

    
}
