//
//  iTermBrowserTemplateLoader.swift
//  DashTerm2
//
//  Created by George Nachman on 6/18/25.
//

import Foundation

@available(macOS 11.0, *)
@objc(iTermBrowserTemplateLoader)
class iTermBrowserTemplateLoader: NSObject {
    static func load(template templateName: String, substitutions: [String: String] = [:]) -> String {
        let base = templateName.deletingPathExtension
        let ext = templateName.pathExtension
        return loadTemplate(named: base, type: ext, substitutions: substitutions)
    }

    static func loadTemplate(named templateName: String,
                             type: String,
                             substitutions: [String: String] = [:]) -> String {
        // BUG-f534: Return empty string instead of crashing if template is missing
        guard let path = Bundle.main.path(forResource: templateName, ofType: type),
              let template = try? String(contentsOfFile: path) else {
            DLog("iTermBrowserTemplateLoader: Template '\(templateName).\(type)' not found in bundle")
            return "<!-- Template '\(templateName).\(type)' not found -->"
        }

        return performSubstitutions(template: template, substitutions: substitutions)
    }
    
    private static func performSubstitutions(template: String, substitutions: [String: String]) -> String {
        var result = template
        
        // Replace {{COMMON_CSS}} with the actual CSS content
        if result.contains("{{COMMON_CSS}}") {
            let commonCSS = iTermBrowserCSSLoader.loadCommonCSS()
            result = result.replacingOccurrences(of: "{{COMMON_CSS}}", with: commonCSS)
        }
        
        // Handle {{INCLUDE:filename}} patterns
        let includePattern = "\\{\\{INCLUDE:([^}]+)\\}\\}"
        // BUG-1603: Use guard with try? instead of try! for regex compilation
        guard let regex = try? NSRegularExpression(pattern: includePattern) else {
            return result
        }
        let range = NSRange(location: 0, length: result.utf16.count)

        // Find all include matches and replace them
        let matches = regex.matches(in: result, range: range).reversed() // Reverse to avoid index issues
        for match in matches {
            if let filenameRange = Range(match.range(at: 1), in: result) {
                let filename = String(result[filenameRange])

                // Extract name and extension from filename
                let components = filename.split(separator: ".")
                // BUG-1530: Use guard to safely access .last instead of force unwrap
                if components.count >= 2, let lastComponent = components.last {
                    let name = String(components.dropLast().joined(separator: "."))
                    let ext = String(lastComponent)

                    // Load the included file
                    if let includePath = Bundle.main.path(forResource: name, ofType: ext),
                       let includeContent = try? String(contentsOfFile: includePath),
                       // BUG-1604: Use guard to safely convert range instead of force unwrap
                       let fullRange = Range(match.range(at: 0), in: result) {
                        result.replaceSubrange(fullRange, with: includeContent)
                    } else {
                        // BUG-f535: Replace include directive with error comment instead of crashing
                        DLog("iTermBrowserTemplateLoader: Could not load included file \(name).\(ext)")
                        if let fullRange = Range(match.range(at: 0), in: result) {
                            result.replaceSubrange(fullRange, with: "<!-- Include '\(name).\(ext)' not found -->")
                        }
                    }
                }
            }
        }
        
        // Replace all other {{KEY}} substitutions
        for (key, value) in substitutions {
            let placeholder = "{{\(key)}}"
            result = result.replacingOccurrences(of: placeholder, with: value)
        }
        
        return result
    }
}
