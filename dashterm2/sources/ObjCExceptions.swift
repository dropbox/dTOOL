//
//  ObjCExceptions.swift
//  DashTerm2
//
//  Created by George Nachman on 5/25/22.
//

import Foundation

func ObjCTry<T>(_ closure: () throws -> T) throws -> T {
    var result: Result<T, Error>? = nil
    let error = ObjCTryImpl {
        do {
            result = .success(try closure())
        } catch {
            result = .failure(error)
        }
    }
    switch result {
    case .success(let value):
        return value
    case .failure(let error):
        throw error
    case .none:
        guard let error = error else {
            // BUG-f569, BUG-f676: Throw a generic error instead of crashing
            DLog("BUG-f569: ThrowingWrapper returned .none with nil error - throwing generic error")
            throw NSError(domain: "ObjCExceptions", code: -1, userInfo: [NSLocalizedDescriptionKey: "ThrowingWrapper returned .none with nil error"])
        }
        throw error
    }
}
