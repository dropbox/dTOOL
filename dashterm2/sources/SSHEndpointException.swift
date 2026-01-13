//
//  SSHEndpointException.swift
//  DashTerm2
//
//  Created by George Nachman on 7/1/25.
//

enum SSHEndpointException: LocalizedError {
    case connectionClosed
    case fileNotFound
    case internalError  // e.g., non-decodable data from fetch
    case transferCanceled
    case notAFile  // BUG-f638: The path is not a regular file (e.g., symlink, socket, etc.)

    var errorDescription: String? {
        get {
            switch self {
            case .connectionClosed:
                return "Connection closed"
            case .fileNotFound:
                return "File not found"
            case .internalError:
                return "Internal error"
            case .transferCanceled:
                return "File transfer canceled"
            case .notAFile:
                return "Not a regular file"
            }
        }
    }
}

