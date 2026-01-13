//
//  SwiftyStringWithBackreferencesEvaluator.swift
//  DashTerm2SharedARC
//
//  Created by George Nachman on 11/16/21.
//

import Foundation

// Evaluates a swifty string when matches (which can be used as backreferences) are present. Adds
// them as \(matches[0]) and such.
@objc(iTermSwiftyStringWithBackreferencesEvaluator)
class SwiftyStringWithBackreferencesEvaluator: NSObject {
    private var cachedSwiftyString: iTermSwiftyString? = nil
    @objc var expression: String

    @objc(initWithExpression:) init(_ expression: String) {
        self.expression = expression
    }

    @objc func evaluate(additionalContext: [String: AnyObject],
                        scope: iTermVariableScope,
                        owner: iTermObject,
                        completion: @escaping (String?, NSError?) -> ()) {
        let myScope = amendedScope(scope,
                                   owner: owner,
                                   values: additionalContext)
        if cachedSwiftyString?.swiftyString != expression {
            cachedSwiftyString = iTermSwiftyString(string: expression,
                                                   scope: myScope,
                                                   sideEffectsAllowed: false,
                                                   observer: nil)
        }
        // BUG-7197: Capture in local variable to prevent race condition where
        // cachedSwiftyString could be modified between the check above and usage below.
        // Without this, the completion handler might never be called.
        guard let swiftyString = cachedSwiftyString else {
            completion(nil, nil)
            return
        }
        swiftyString.evaluateSynchronously(false,
                                           sideEffectsAllowed: false,
                                           with: myScope) { value, error, missing in
            if let error = error {
                completion(nil, error as NSError)
                return
            }
            completion(value, nil)
        }
    }

    private func amendedScope(_ scope: iTermVariableScope,
                              owner: iTermObject,
                              values: [String: AnyObject]) -> iTermVariableScope {
        let matchesFrame = iTermVariables(context: [], owner: owner)
        // BUG-1707: Use guard with as? instead of as! for scope.copy() cast
        guard let myScope = scope.copy() as? iTermVariableScope else {
            return scope
        }
        myScope.add(matchesFrame, toScopeNamed: nil)
        for (key, value) in values {
            myScope.setValue(value, forVariableNamed: key)
        }
        return myScope
    }
}
