//
//  Data+iTerm.swift
//  DashTerm2
//
//  Created by George Nachman on 6/5/25.
//

extension Data {
    var lossyString: String {
        return String(decoding: self, as: UTF8.self)
    }
}

extension Data {
    func nonEmptyBase64EncodedString() -> String {
        if isEmpty {
            return "="
        }
        return base64EncodedString()
    }
}

extension Data {
    func last(_ n: Int) -> Data {
        if count < n {
            return self
        }
        let i = count - n
        return self[i...]
    }

    var semiVerboseDescription: String {
        if count > 32 {
            return self[..<16].semiVerboseDescription + "…" + self.last(16).semiVerboseDescription
        }
        if let string = String(data: self, encoding: .utf8) {
            let safe = (string as NSString).escapingControlCharactersAndBackslash()
            return "“\(safe)”"
        }
        return (self as NSData).it_hexEncoded()
    }
}

extension Data {
    /// Split data into chunks of the specified size.
    /// - Parameter size: The maximum size of each chunk. Must be > 0.
    /// - Returns: Array of SubData chunks. Returns a single chunk containing all data if size <= 0.
    func chunks(of size: Int) -> [SubData] {
        // BUG-12001: Guard against division by zero and infinite loop when size <= 0
        guard size > 0 else {
            // Return entire data as single chunk if size is invalid
            return isEmpty ? [] : [SubData(data: self, range: 0..<count)]
        }
        var result: [SubData] = []
        result.reserveCapacity(count / size + 1)

        var index = 0

        while index < count {
            let end = Swift.min(index + size, count)
            let range = index..<end
            result.append(SubData(data: self, range: range))
            index = end
        }

        return result
    }
}
