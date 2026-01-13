//
//  iTermSubString.swift
//  StyleMap
//
//  Created by George Nachman on 4/21/25.
//

/// A lightweight “view” into an existing iTermString, masking off
/// a prefix or suffix by only exposing cells in `range`.
@objc
class iTermSubString: iTermBaseString, iTermString {
    private let base: iTermString
    private let range: Range<Int>
    private lazy var stringCache = SubStringCache()

    @objc(initWithBaseString:range:)
    convenience init?(base: iTermString, range: NSRange) {
        // BUG-1663: Use guard instead of force unwrap for Range conversion
        guard let swiftRange = Range(range) else { return nil }
        self.init(base: base, range: swiftRange)
    }

    required init(base: iTermString, range: Range<Int>) {
        // BUG-f630: Clamp range instead of it_assert crash - assertions stripped in release builds
        let clampedRange: Range<Int>
        if base.fullRange.contains(range) {
            clampedRange = range
        } else {
            DLog("iTermSubString: clamping range \(range) to base.fullRange \(base.fullRange)")
            let lower = Swift.max(base.fullRange.lowerBound, Swift.min(range.lowerBound, base.fullRange.upperBound))
            let upper = Swift.max(lower, Swift.min(range.upperBound, base.fullRange.upperBound))
            clampedRange = lower..<upper
        }
        if let sub = base as? iTermSubString {
            // unwrap nested substring
            self.base = sub.base
            let offset = sub.range.lowerBound
            let lower = offset + clampedRange.lowerBound
            let upper = offset + clampedRange.upperBound
            self.range = lower..<upper
        } else {
            // Make totally sure `base` is immutable!
            self.base = base.clone()
            self.range = clampedRange
        }
    }

    override var description: String {
        return "<iTermSubString: base=\(type(of: base)) @ \(((base as? NSObject)?.it_addressString).d) cells=\(cellCount) value=\(deltaString(range: fullRange).string.trimmingTrailingNulls.escapingControlCharactersAndBackslash())>"
    }

    func deltaString(range: NSRange) -> DeltaString {
        return stringCache.string(for: range) {
            _deltaString(range: range)
        }
    }

    var cellCount: Int { range.count }

    func character(at i: Int) -> screen_char_t {
        return base.character(at: range.lowerBound + i)
    }

    private func global(range nsRange: NSRange) -> NSRange {
        return NSRange(location: range.lowerBound + nsRange.location,
                       length: nsRange.length)
    }
    func hydrate(into msca: MutableScreenCharArray,
                 destinationIndex: Int,
                 sourceRange: NSRange) {
        base.hydrate(into: msca,
                     destinationIndex: destinationIndex,
                     sourceRange: global(range: sourceRange))
    }

    func hydrate(range nsRange: NSRange) -> ScreenCharArray {
        return base.hydrate(range: global(range: nsRange))
    }

    func buildString(range nsRange: NSRange, builder: DeltaStringBuilder) {
        base.buildString(range: global(range: nsRange), builder: builder)
    }

    func mutableClone() -> any iTermMutableStringProtocol {
        return _mutableClone()
    }

    func clone() -> any iTermString {
        return self
    }

    func externalAttributesIndex() -> (any iTermExternalAttributeIndexReading)? {
        return _externalAttributesIndex()
    }

    var screenCharArray: ScreenCharArray { _screenCharArray }

    func hasEqual(range: NSRange, to chars: UnsafePointer<screen_char_t>) -> Bool {
        return _hasEqual(range: range, to: chars)
    }

    func usedLength(range: NSRange) -> Int32 {
        return base.usedLength(range: global(range: range))
    }

    func isEmpty(range: NSRange) -> Bool {
        return base.isEmpty(range: global(range: range))
    }

    func substring(range: NSRange) -> any iTermString {
        guard let swiftRange = Range(global(range: range)) else {
            return self  // Return self if conversion fails
        }
        return iTermSubString(base: base, range: swiftRange)
    }

    func externalAttribute(at index: Int) -> iTermExternalAttribute? {
        return base.externalAttribute(at: global(range: NSRange(location: index, length: 1)).lowerBound)
    }

    func isEqual(to string: any iTermString) -> Bool {
        if cellCount != string.cellCount {
            return false
        }
        return isEqual(lhsRange: fullRange, toString: string, startingAtIndex: 0)
    }

    // This implements:
    // return self[lhsRange] == rhs[startIndex..<(startIndex+lhsRange.count)
    func isEqual(lhsRange lhsNSRange: NSRange, toString rhs: iTermString, startingAtIndex startIndex: Int) -> Bool {
        if cellCount < NSMaxRange(lhsNSRange) || rhs.cellCount < startIndex + lhsNSRange.length {
            return false
        }
        return base.isEqual(lhsRange: NSRange(location: lhsNSRange.location + self.range.lowerBound,
                                              length: lhsNSRange.length),
                            toString: rhs,
                            startingAtIndex: startIndex)
    }

    func stringBySettingRTL(in nsrange: NSRange, rtlIndexes: IndexSet?) -> any iTermString {
        let subrange = NSRange(location: self.range.lowerBound + nsrange.location,
                               length: nsrange.length)
        var shifted = rtlIndexes
        shifted?.shift(startingAt: 0, by: range.lowerBound)
        return base.stringBySettingRTL(in: subrange,
                                       rtlIndexes: shifted)
    }

    func doubleWidthIndexes(range nsrange: NSRange,
                            rebaseTo newBaseIndex: Int) -> IndexSet {
        return base.doubleWidthIndexes(range: NSRange(location: range.lowerBound + nsrange.location, length: nsrange.length), rebaseTo: newBaseIndex)
    }

    var mayContainDoubleWidthCharacter: Bool {
        base.mayContainDoubleWidthCharacter(in: NSRange(range))
    }
    func mayContainDoubleWidthCharacter(in nsrange: NSRange) -> Bool {
        let subrange = NSRange(location: self.range.lowerBound + nsrange.location,
                               length: nsrange.length)
        return base.mayContainDoubleWidthCharacter(in: subrange)
    }
    func hasExternalAttributes(range nsrange: NSRange) -> Bool {
        let subrange = NSRange(location: self.range.lowerBound + nsrange.location,
                               length: nsrange.length)
        return base.hasExternalAttributes(range: subrange)
    }
    enum CodingKeys: Int32, TLVTag {
        case stringType
        case stringData
    }
    func efficientlyEncodedData(range nsrange: NSRange, type: UnsafeMutablePointer<Int32>) -> Data {
        let subrange = NSRange(location: self.range.lowerBound + nsrange.location,
                               length: nsrange.length)

        type.pointee = iTermStringType.subString.rawValue

        var stringType = Int32(0)
        let stringData = base.efficientlyEncodedData(range: subrange, type: &stringType)

        var tlvEncoder = EfficientTLVEncoder<CodingKeys>()
        tlvEncoder.put(tag: .stringType, value: stringType)
        tlvEncoder.put(tag: .stringData, value: stringData)
        return tlvEncoder.data
    }
}

extension iTermSubString: EfficientDecodable, EfficientEncodable {
    static func create(efficientDecoder decoder: inout EfficientDecoder) throws -> Self {
        var tlvDecoder: EfficientTLVDecoder<CodingKeys> = decoder.tlvDecoder()
        let dict = try tlvDecoder.decodeAll(required: Set([.stringType, .stringData]))
        guard var stringTypeDecoder = dict[.stringType],
              var stringDataDecoder = dict[.stringData] else {
            throw NSError(domain: "iTermSubString", code: 2, userInfo: [NSLocalizedDescriptionKey: "Missing required fields"])
        }
        let stringType = try Int32.create(efficientDecoder: &stringTypeDecoder)
        let stringData = try Data.create(efficientDecoder: &stringDataDecoder)
        let base = try CreateString(type: iTermStringType(rawValue: stringType),
                                    stringData: stringData)
        // BUG-1666: Use guard instead of force unwrap for Range conversion
        guard let swiftRange = Range(base.fullRange) else {
            throw NSError(domain: "iTermSubString", code: 1, userInfo: [NSLocalizedDescriptionKey: "Invalid range"])
        }
        return Self(base: base, range: swiftRange)
    }

    func encodeEfficiently(encoder: inout EfficientEncoder) {
        var type = Int32(0)
        let data = efficientlyEncodedData(range: fullRange, type: &type)
        encoder.putRawBytes(data)
    }
}
