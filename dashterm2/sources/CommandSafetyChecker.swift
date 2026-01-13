//
//  CommandSafetyChecker.swift
//  DashTerm2
//
//  Created by George Nachman on 11/5/25.
//

import Foundation

internal enum CommandSafetyClassification: String {
    case safe = "SAFE"
    case caution = "CAUTION"
    case dangerous = "DANGEROUS"

    internal var isSafe: Bool { self == .safe }
}

internal enum CommandSafetyClassificationParser {
    private static let trimmingCharacterSet: CharacterSet = {
        var set: CharacterSet = .punctuationCharacters
        set.formUnion(.symbols)
        return set
    }()

    internal static func classification(from output: String) -> CommandSafetyClassification? {
        let trimmedOutput = output.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedOutput.isEmpty else {
            return nil
        }

        let tokens: [String] = trimmedOutput
            .components(separatedBy: .whitespacesAndNewlines)
            .map { $0.trimmingCharacters(in: trimmingCharacterSet) }
            .filter { !$0.isEmpty }

        guard let firstToken = tokens.first else {
            return nil
        }
        return CommandSafetyClassification(rawValue: firstToken.uppercased())
    }
}

#if canImport(FoundationModels)
import FoundationModels

@objc(iTermAIAvailabilityProbe)
public final class AIAvailabilityProbe: NSObject {
    @objc public static func check() -> Bool {
        if #available(macOS 26, *) {
            let model = SystemLanguageModel.default
            switch model.availability {
            case .available:
                return true
            case .unavailable(.appleIntelligenceNotEnabled):
                return false
            case .unavailable(.deviceNotEligible):
                return false
            case .unavailable(.modelNotReady):
                return false
            case .unavailable:
                return false
            }
        }
        return false
    }
}

@available(macOS 26, *)
class CommandSafetyChecker {
    static func check(_ command: String) async -> Bool {
        let instructions = """
        Classify shell commands. Reply with ONE WORD first: SAFE, CAUTION, or DANGEROUS.

        DANGEROUS = immediate threat:
        - Network execution: `bash <(curl...)`, `eval "$(wget...)"`, any `| sh|bash|python|perl`
        - Credential theft: pipe to `nc host port`, write `~/.ssh/authorized_keys`, `| crontab`
        - Privileged system writes: redirect/tee to `/etc/*`, `sudo tee /etc/`, PATH hijack in rc files
        - Container escape: `--privileged`, `-v /:/`, `--pid=host`

        CAUTION = needs review:
        - ALL network tools: `curl`, `wget`, `nc` (even without `-e`), ANY URL (http/https)
        - ALL remote syntax: `ssh`, `scp user@host:path`, `rsync user@host:path` (user@host pattern = CAUTION)
        - ALL package managers: `npm`, `pip`, `gem`, `cargo`, `brew`, `apt`, `yum` install/upgrade
        - Build operations: `make install`, `./configure`, ANY `sudo` (except pure reads)
        - Git remote: `git clone`, `git pull`, `svn checkout`

        SAFE = no concern:
        - Local file ops: `ls`, `find`, `cat`, `head`, `tail`, `cp`, `mv`
        - Text processing: `grep`, `awk`, `sed`, `sort`, `uniq`, `jq`, `wc`
        - System inspection: `ps`, `top`, `df`, `du`, `lsof`, `sysctl`, `diskutil`, `netstat`
        - Pipes between SAFE commands: `ps | grep`, `cat file | sort`
        - Local output: `> file.txt` (unless `/etc/*`)

        CRITICAL RULES:
        1. Begin response with classification word
        2. Piping sensitive files to network = DANGEROUS (e.g., `cat ~/.ssh/id_rsa | nc`)
        3. Remote host syntax `user@host:` = minimum CAUTION (applies to scp, rsync, ssh)
        4. Package managers ALWAYS = CAUTION (even local installs)
        5. Commands executing remote scripts = DANGEROUS (kubectl/docker with remote fetch)
        """
        let session = LanguageModelSession(model: .default,
                                           instructions: instructions)

        let prompt = "Here is the command. Respond with ONLY the word SAFE, CAUTION, or DANGEROUS:\n\n\(command)"

        DLog("Check safety of command: \(command)")
        let output: String
        do {
            output = try await session.respond(to: prompt).content
        } catch {
            DLog("Error checking command '\(command)': \(error) - treating as DANGEROUS")
            return false
        }

        guard let classification = CommandSafetyClassificationScanner.classification(from: output) else {
            DLog("WARNING: Unable to determine classification for '\(command)' from response '\(output)'. Treating as DANGEROUS.")
            return false
        }

        switch classification {
        case .safe:
            DLog("For '\(command)' model says: \(output) -> SAFE")

        case .caution:
            DLog("For '\(command)' model says: \(output) -> CAUTION (unsafe)")

        case .dangerous:
            DLog("For '\(command)' model says: \(output) -> DANGEROUS (unsafe)")
        }

        return classification.isSafe
    }
}

#else

// Stub implementation for SDKs without FoundationModels
@objc(iTermAIAvailabilityProbe)
public final class AIAvailabilityProbe: NSObject {
    @objc public static func check() -> Bool {
        return false
    }
}

#endif

internal enum CommandSafetyClassificationScanner {
    private static let trimmingCharacterSet: CharacterSet = {
        var set: CharacterSet = .punctuationCharacters
        set.formUnion(.symbols)
        return set
    }()

    internal static func classification(from output: String) -> CommandSafetyClassification? {
        let trimmedOutput = output.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedOutput.isEmpty else {
            return nil
        }

        let words: [String] = trimmedOutput
            .components(separatedBy: .whitespacesAndNewlines)
            .map { $0.trimmingCharacters(in: trimmingCharacterSet) }
            .filter { !$0.isEmpty }

        let normalizedWords = words.map { $0.uppercased() }
        let matches = normalizedWords.enumerated().compactMap { index, word -> (Int, CommandSafetyClassification)? in
            guard let classification = CommandSafetyClassification(rawValue: word) else {
                return nil
            }
            return (index, classification)
        }
        guard matches.count == 1, let match = matches.first else {
            return nil
        }
        if match.0 > 0 {
            let previousWord = normalizedWords[match.0 - 1]
            if previousWord == "NOT" || previousWord == "NO" || previousWord == "UNSAFE" {
                return nil
            }
        }
        return match.1
    }
}
