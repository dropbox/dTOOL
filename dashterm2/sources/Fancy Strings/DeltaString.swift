//
//  DeltaString.swift
//  StyleMap
//
//  Created by George Nachman on 4/21/25.
//

import Foundation

@objc(iTermDeltaString)
class DeltaString: NSObject {
    @objc let unsafeString: NSString  // fast but lifetime-limited by this DeltaString object
    // BUG-1740: Use as? with fallback to original string
    @objc var string: NSString { (unsafeString.copy() as? NSString) ?? unsafeString }
    @objc let length: CInt

    // 1:1 with UTF-16 codepoints in `string`
    @objc var deltas: UnsafePointer<CInt> { UnsafePointer(deltasStore) }

    private let deltasStore: UnsafeMutablePointer<CInt>
    @objc let backingStore: UnsafeMutablePointer<unichar>?

    /// BUG-f561: Shared empty DeltaString for fallback when builder fails
    @objc static let empty: DeltaString = {
        // Allocate minimal buffers - will be owned by the DeltaString
        let dPtr = UnsafeMutablePointer<CInt>.allocate(capacity: 1)
        dPtr.initialize(to: 0)
        return DeltaString(string: "" as NSString, length: 0, deltasStore: dPtr, backingStore: nil)
    }()

    var safeDeltas: [CInt] {
        let buffer = UnsafeBufferPointer(start: deltas, count: Int(length))
        return Array(buffer)
    }

    @objc
    init(string: NSString,
         length: CInt,
         deltasStore: UnsafeMutablePointer<CInt>,
         backingStore: UnsafeMutablePointer<unichar>?) {
        self.unsafeString = string
        self.length = length
        self.deltasStore = deltasStore
        self.backingStore = backingStore
        super.init()
    }

    deinit {
        free(deltasStore)
        free(backingStore)
    }

    func cellIndexForUTF16Index(_ utf16Index: Int) -> Int {
        if utf16Index >= length {
            if length == 0 {
                return utf16Index
            }
            return utf16Index + Int(deltasStore[Int(length) - 1])
        }
        return utf16Index + Int(deltasStore[utf16Index])
    }
}

@objc
class DeltaStringBuilder: NSObject {
    private let length: CInt
    private let utf16Cap: Int
    private var deltaPtr: UnsafeMutablePointer<CInt>?
    private var uniPtr: UnsafeMutablePointer<unichar>?
    private var deltaIdx = 0
    private var uniIdx = 0
    private var runningΔ = 0
    private let maxParts = 20
    /// BUG-f551: Track whether builder failed initialization
    private let initFailed: Bool

    /// Pre-allocate both buffers.
    /// BUG-f551 to BUG-f556: Make initialization gracefully handle errors instead of crashing
    /// Returns a builder that may be in a failed state (check with build() returning nil)
    override init() {
        length = 0
        utf16Cap = 1
        initFailed = true
        super.init()
    }

    init(count: CInt) {
        length = count
        // BUG-f551: Check for integer overflow in size calculations - set failed state instead of crash
        let countInt = Int(count)
        let (utf16CapResult, utf16Overflow) = countInt.multipliedReportingOverflow(by: maxParts)
        let (utf16CapPlusOne, utf16CapOverflow) = utf16CapResult.addingReportingOverflow(1)
        if utf16Overflow || utf16CapOverflow {
            DLog("BUG-f551: Integer overflow calculating utf16Cap for count \(count)")
            utf16Cap = 1
            initFailed = true
            super.init()
            return
        }
        utf16Cap = utf16CapPlusOne

        let padded = count + 1
        // malloc so DeltaString can free()
        let (dBytes, dOverflow) = Int(padded).multipliedReportingOverflow(by: MemoryLayout<CInt>.size)
        if dOverflow {
            // BUG-f552: Handle deltaPtr overflow gracefully
            DLog("BUG-f552: Integer overflow calculating deltaPtr size for count \(count)")
            initFailed = true
            super.init()
            return
        }
        guard let dRaw = UnsafeMutableRawPointer(malloc(dBytes)) else {
            // BUG-f553: Handle malloc failure gracefully
            DLog("BUG-f553: malloc failed for deltaPtr (\(dBytes) bytes)")
            initFailed = true
            super.init()
            return
        }
        deltaPtr = dRaw.bindMemory(to: CInt.self, capacity: Int(padded))

        let (uBytes, uOverflow) = utf16Cap.multipliedReportingOverflow(by: MemoryLayout<unichar>.size)
        if uOverflow {
            // BUG-f554: Handle uniPtr overflow gracefully
            DLog("BUG-f554: Integer overflow calculating uniPtr size for utf16Cap \(utf16Cap)")
            free(deltaPtr)
            deltaPtr = nil
            initFailed = true
            super.init()
            return
        }
        guard let uRaw = UnsafeMutableRawPointer(malloc(uBytes)) else {
            // BUG-f555: Handle malloc failure gracefully
            DLog("BUG-f555: malloc failed for uniPtr (\(uBytes) bytes)")
            free(deltaPtr)
            deltaPtr = nil
            initFailed = true
            super.init()
            return
        }
        uniPtr = uRaw.bindMemory(to: unichar.self, capacity: utf16Cap)
        initFailed = false
        super.init()
    }

    deinit {
        free(deltaPtr)
        free(uniPtr)
    }

    /// append exactly one ASCII chunk
    /// BUG-f556: Return early instead of crashing if called after build() or on failed builder
    func append(ascii data: SubData) {
        guard !initFailed, let uPtr = uniPtr, let dPtr = deltaPtr else {
            DLog("BUG-f556: append(ascii:) called on invalid builder state")
            return
        }
        let bytes = data.data
        for b in bytes {
            uPtr[uniIdx] = unichar(b)
            dPtr[deltaIdx] = 0
            uniIdx += 1
            deltaIdx += 1
        }
    }

    /// append codes+complex exactly as before, carrying the running `delta` across chunks
    /// BUG-f557: Return early instead of crashing if called after build() or on failed builder
    func append(codes: SubArray<UInt16>, complex: IndexSet, range: NSRange) {
        guard !initFailed, let uPtr = uniPtr, let dPtr = deltaPtr else {
            DLog("BUG-f557: append(codes:complex:range:) called on invalid builder state")
            return
        }
        let start = range.location
        let count = range.length

        for offset in 0..<count {
            let i = start + offset
            let code = codes[i]
            let isComplexChar = complex.contains(i)

            runningΔ += 1

            // private‑use handling
            if !isComplexChar
                && (UInt16(ITERM2_PRIVATE_BEGIN)...UInt16(ITERM2_PRIVATE_END)).contains(code) {
                continue
            }

            if let partNS = ComplexCharRegistry.instance.string(for: code, isComplex: isComplexChar) {
                let part = partNS as String

                for cu in part.utf16 {
                    runningΔ -= 1
                    dPtr[deltaIdx] = CInt(runningΔ)
                    deltaIdx += 1

                    uPtr[uniIdx] = cu
                    uniIdx += 1
                }
            } else {
                dPtr[deltaIdx] = CInt(runningΔ)
                deltaIdx += 1
                uPtr[uniIdx] = unichar(code)
                uniIdx += 1
            }
        }
    }

    /// BUG-f558: Return early instead of crashing if called after build() or on failed builder
    func append(char: screen_char_t, repeated count: Int) {
        guard !initFailed, let uPtr = uniPtr, let dPtr = deltaPtr else {
            DLog("BUG-f558: append(char:repeated:) called on invalid builder state")
            return
        }
        let code = char.code
        let isComplexChar = char.complexChar != 0
        var partNS: NSString?
        if isComplexChar {
            partNS = ComplexCharRegistry.instance.string(for: code, isComplex: isComplexChar)
        }
        for offset in 0..<count {
            // private‑use handling
            if !isComplexChar
                && (UInt16(ITERM2_PRIVATE_BEGIN)...UInt16(ITERM2_PRIVATE_END)).contains(code) {
                runningΔ += 1
                dPtr[deltaIdx + offset] = CInt(runningΔ)
                continue
            }

            if let partNS {
                let part = partNS as String
                let utf16Len = part.utf16.count
                runningΔ += 1
                runningΔ -= utf16Len
                dPtr[deltaIdx + offset] = CInt(runningΔ)
                for cu in part.utf16 {
                    uPtr[uniIdx] = cu
                    uniIdx += 1
                }
            } else {
                dPtr[deltaIdx + offset] = CInt(runningΔ)
                uPtr[uniIdx] = unichar(code)
                uniIdx += 1
            }
        }

        deltaIdx += count
    }
    /// once you've appended *exactly* `deltaCount` entries and `utf16Cap` code units,
    /// tear down the builder and hand off ownership of both buffers to DeltaString
    /// BUG-f533: Use guard clauses instead of precondition crash for buffer overflow
    /// BUG-f559: Return nil instead of crashing if called twice or on failed builder
    func build() -> DeltaString? {
        guard !initFailed, let uPtr = uniPtr, let dPtr = deltaPtr else {
            DLog("BUG-f559: build() called on invalid builder state (initFailed=\(initFailed), ptrs nil)")
            return nil
        }
        // BUG-f533: Log and handle overflow conditions gracefully by clamping
        if deltaIdx > utf16Cap {
            DLog("DeltaStringBuilder.build: deltaIdx \(deltaIdx) exceeds utf16Cap \(utf16Cap), truncating")
        }
        if uniIdx > utf16Cap {
            DLog("DeltaStringBuilder.build: uniIdx \(uniIdx) exceeds utf16Cap \(utf16Cap), truncating")
        }
        // Clamp indices to buffer capacity
        let safeUniIdx = min(uniIdx, utf16Cap)
        let safeDeltaIdx = min(deltaIdx, utf16Cap)
        // Synchronize indices - use the minimum to ensure consistency
        let finalIdx = min(safeUniIdx, safeDeltaIdx)
        if deltaIdx != uniIdx {
            DLog("DeltaStringBuilder.build: deltaIdx \(deltaIdx) != uniIdx \(uniIdx), using \(finalIdx)")
        }

        // make NSString *without* copying or free‐on‐dealloc
        let ns = NSString(
            charactersNoCopy: uPtr,
            length: finalIdx,
            freeWhenDone: false
        )
        // steal the pointers
        let dp = dPtr
        let up = uPtr

        // prevent our deinit from freeing them
        deltaPtr = nil
        uniPtr   = nil

        return DeltaString(
            string: ns,
            length: length,
            deltasStore: dp,
            backingStore: up
        )
    }

    /// BUG-f560: Return early instead of crashing if called after build() or on failed builder
    func append(chars: UnsafePointer<screen_char_t>, count: CInt) {
        guard !initFailed, let uPtr = uniPtr, let dPtr = deltaPtr else {
            DLog("BUG-f560: append(chars:count:) called on invalid builder state")
            return
        }
        let n = Int(count)
        let privateRange = UInt16(ITERM2_PRIVATE_BEGIN)...UInt16(ITERM2_PRIVATE_END)
        for i in 0..<n {
            var cell = chars[i]
            let code = cell.code
            let isComplex = cell.complexChar != 0
            let isImage = cell.image != 0

            if isImage ||
                (!isComplex && privateRange.contains(code)) {
                // Skip private-use characters which signify things like double-width characters and
                // tab fillers.
                runningΔ += 1
                continue
            }

            // BUG-1474: Check bounds against utf16Cap for both uniIdx and deltaIdx
            // since both can expand up to maxParts per cell
            if uniIdx + maxParts > utf16Cap {
                // Would overflow buffer, stop processing
                break
            }
            let len = ExpandScreenChar(&cell, uPtr.advanced(by: uniIdx))
            uniIdx += Int(len)
            runningΔ += 1
            for _ in 0..<len {
                runningΔ -= 1
                // BUG-f624: Use guard instead of it_assert - assertions stripped in release builds
                // causing buffer overflow if deltaIdx >= utf16Cap
                guard deltaIdx < utf16Cap else {
                    DLog("DeltaString: deltaIdx \(deltaIdx) exceeds utf16Cap \(utf16Cap), stopping early")
                    break
                }
                dPtr[deltaIdx] = CInt(runningΔ)
                deltaIdx += 1
            }
        }
        // BUG-f625: Use guard instead of it_assert for final write
        guard deltaIdx <= utf16Cap else {
            DLog("DeltaString: final deltaIdx \(deltaIdx) exceeds utf16Cap \(utf16Cap)")
            return
        }
        dPtr[deltaIdx] = CInt(runningΔ)
    }
}
