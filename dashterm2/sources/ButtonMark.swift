//
//  ButtonMark.swift
//  DashTerm2SharedARC
//
//  Created by George Nachman on 9/28/23.
//

import Foundation

@objc(iTermButtonMarkReading)
protocol ButtonMarkReading: AnyObject, iTermMarkProtocol {
    @objc var copyBlockID: String? { get }
    @objc var channelUID: String? { get }
    @objc var buttonID: Int { get }
}

enum ButtonType {
    static func fromDictionary(_ dict: [AnyHashable : Any]) -> ButtonType? {
        switch dict["type"] as? String {
        case "copy":
            if let block = dict["block ID"], let blockID = block as? String {
                return .copy(block: blockID)
            }
            break
        default:
            break
        }
        return nil
    }

    var dictionary: NSDictionary {
        switch self {
        case .copy(block: let blockID):
            return ["type": "copy",
                    "block ID": blockID]
        case .channel(uid: let uid):
            return ["type": "channel",
                    "uid": uid]
        }
    }
    case copy(block: String)
    case channel(uid: String)
}

@objc(iTermButtonMark)
class ButtonMark: iTermMark, ButtonMarkReading {
    var buttonType: ButtonType?
    var buttonID: Int

    var copyBlockID: String? {
        get {
            switch buttonType {
            case .copy(block: let blockID):
                return blockID
            case .none, .channel:
                break
            }
            return nil
        }
        set {
            if let newValue {
                buttonType = .copy(block: newValue)
            } else {
                buttonType = nil
            }
        }
    }

    @objc
    var channelUID: String? {
        get {
            switch buttonType {
            case .copy, .none:
                nil
            case .channel(uid: let uid):
                uid
            }
        }
        set {
            if let newValue {
                buttonType = .channel(uid: newValue)
            } else {
                buttonType = nil
            }
        }
    }

    @objc static func generateID() -> Int {
        defer {
            nextId += 1
        }
        return nextId
    }
    // nonisolated(unsafe) because this is only used in sequential id generation
    nonisolated(unsafe) private static var nextId = 0

    @objc
    override init() {
        buttonID = Self.generateID()
        super.init()
    }

    required init!(dictionary dict: [AnyHashable : Any]!) {
        buttonType = .fromDictionary(dict)
        buttonID = dict["id"] as? Int ?? Self.generateID()
        super.init(dictionary: dict)
    }

    override func dictionaryValue() -> [AnyHashable : Any]! {
        // BUG-1693: Use guard instead of force unwrap for super.dictionaryValue()
        guard var dict = super.dictionaryValue() else {
            return nil
        }
        if let local = buttonType?.dictionary as? [AnyHashable: Any] {
            // BUG-f851: Use existing value on merge conflict instead of crashing
            dict.merge(local) { existing, _ in
                DLog("Unexpected merge conflict in ButtonMark.dictionaryValue(), keeping existing value")
                return existing
            }
        }
        dict["id"] = buttonID
        return dict
    }
}

@objc(iTermTerminalButtonPlace)
class TerminalButtonPlace: NSObject {
    @objc var id: Int { mark.buttonID }
    @objc var mark: ButtonMarkReading
    @objc var coord: VT100GridAbsCoord

    @objc
    init(mark: ButtonMarkReading, coord: VT100GridAbsCoord) {
        self.mark = mark
        self.coord = coord
    }
}
