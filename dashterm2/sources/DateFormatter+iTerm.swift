//
//  DateFormatter+iTerm.swift
//  DashTerm2
//
//  Created by George Nachman on 5/27/25.
//

// BUG-2739: Cache key includes locale to prevent drift when user changes locale
// nonisolated(unsafe) because MutableAtomicObject provides thread-safe access
nonisolated(unsafe) private var cachedDateFormatters = MutableAtomicObject([String: DateFormatter]())

@objc
extension DateFormatter {
    @objc static func cacheableFormatter(template: String) -> DateFormatter {
        let currentLocale = Locale.current
        let cacheKey = "\(currentLocale.identifier):\(template)"
        return cachedDateFormatters.mutableAccess { dict in
            if let value = dict[cacheKey] {
                return value
            }
            let dateFormat = DateFormatter.dateFormat(fromTemplate: template, options: 0, locale: currentLocale)
            let formatter = DateFormatter()
            formatter.dateFormat = dateFormat
            formatter.locale = currentLocale
            dict[cacheKey] = formatter
            return formatter
        }
    }
}
