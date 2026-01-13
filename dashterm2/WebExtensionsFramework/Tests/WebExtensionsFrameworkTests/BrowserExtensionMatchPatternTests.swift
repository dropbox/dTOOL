import XCTest
@testable import WebExtensionsFramework

final class BrowserExtensionMatchPatternTests: XCTestCase {

    // MARK: - Basic Match Pattern Parsing Tests
    // Based on Documentation/manifest-fields/host_permissions.md examples

    func testParseBasicHTTPSPattern() throws {
        // From docs: "https://example.com/*" - All paths on example.com over HTTPS
        let pattern = try BrowserExtensionMatchPattern("https://example.com/*")

        XCTAssertEqual(pattern.scheme, .https)
        XCTAssertEqual(pattern.host, "example.com")
        XCTAssertEqual(pattern.path, "/*")
    }

    func testParseWildcardSchemePattern() throws {
        // From docs: "*://*.example.com/*" - All subdomains of example.com over any protocol
        let pattern = try BrowserExtensionMatchPattern("*://*.example.com/*")

        XCTAssertEqual(pattern.scheme, .any)
        XCTAssertEqual(pattern.host, "*.example.com")
        XCTAssertEqual(pattern.path, "/*")
    }

    func testParseAllURLsPattern() throws {
        // From docs: "<all_urls>" - Special pattern matching all URLs
        let pattern = try BrowserExtensionMatchPattern("<all_urls>")

        XCTAssertEqual(pattern.scheme, .allURLs)
        XCTAssertEqual(pattern.host, "*")
        XCTAssertEqual(pattern.path, "/*")
    }

    // MARK: - URL Matching Tests

    func testMatchExactDomain() throws {
        let pattern = try BrowserExtensionMatchPattern("https://example.com/*")

        guard let url1 = URL(string: "https://example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url2 = URL(string: "https://example.com/path") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url3 = URL(string: "https://example.com/path/subpath") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url4 = URL(string: "http://example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url5 = URL(string: "https://sub.example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url6 = URL(string: "https://example.org/") else {
            XCTFail("Invalid URL")
            return
        }

        XCTAssertTrue(pattern.matches(url1))
        XCTAssertTrue(pattern.matches(url2))
        XCTAssertTrue(pattern.matches(url3))

        XCTAssertFalse(pattern.matches(url4)) // Wrong scheme
        XCTAssertFalse(pattern.matches(url5)) // Wrong host
        XCTAssertFalse(pattern.matches(url6)) // Wrong host
    }

    func testMatchWildcardSubdomain() throws {
        let pattern = try BrowserExtensionMatchPattern("https://*.example.com/*")

        guard let url1 = URL(string: "https://sub.example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url2 = URL(string: "https://deep.sub.example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url3 = URL(string: "https://api.example.com/v1/users") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url4 = URL(string: "https://example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url5 = URL(string: "http://sub.example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url6 = URL(string: "https://example.org/") else {
            XCTFail("Invalid URL")
            return
        }

        XCTAssertTrue(pattern.matches(url1))
        XCTAssertTrue(pattern.matches(url2))
        XCTAssertTrue(pattern.matches(url3))

        XCTAssertFalse(pattern.matches(url4)) // No subdomain
        XCTAssertFalse(pattern.matches(url5)) // Wrong scheme
        XCTAssertFalse(pattern.matches(url6)) // Wrong domain
    }

    func testMatchAnyScheme() throws {
        let pattern = try BrowserExtensionMatchPattern("*://example.com/*")

        guard let url1 = URL(string: "http://example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url2 = URL(string: "https://example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url3 = URL(string: "ftp://example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url4 = URL(string: "file://example.com/") else {
            XCTFail("Invalid URL")
            return
        }

        XCTAssertTrue(pattern.matches(url1))
        XCTAssertTrue(pattern.matches(url2))

        XCTAssertFalse(pattern.matches(url3)) // * only matches http/https
        XCTAssertFalse(pattern.matches(url4))
    }

    func testMatchAllURLs() throws {
        let pattern = try BrowserExtensionMatchPattern("<all_urls>")

        guard let url1 = URL(string: "https://example.com/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url2 = URL(string: "http://localhost:8080/api") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url3 = URL(string: "https://sub.domain.com/path?query=1") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url4 = URL(string: "file:///Users/test/file.txt") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url5 = URL(string: "ftp://ftp.example.com/") else {
            XCTFail("Invalid URL")
            return
        }

        XCTAssertTrue(pattern.matches(url1))
        XCTAssertTrue(pattern.matches(url2))
        XCTAssertTrue(pattern.matches(url3))
        XCTAssertTrue(pattern.matches(url4))
        XCTAssertTrue(pattern.matches(url5))
    }

    func testMatchSpecificPath() throws {
        let pattern = try BrowserExtensionMatchPattern("https://api.example.com/v1/*")

        guard let url1 = URL(string: "https://api.example.com/v1/users") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url2 = URL(string: "https://api.example.com/v1/posts/123") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url3 = URL(string: "https://api.example.com/v2/users") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url4 = URL(string: "https://api.example.com/") else {
            XCTFail("Invalid URL")
            return
        }

        XCTAssertTrue(pattern.matches(url1))
        XCTAssertTrue(pattern.matches(url2))

        XCTAssertFalse(pattern.matches(url3))
        XCTAssertFalse(pattern.matches(url4))
    }

    func testMatchFileURL() throws {
        let pattern = try BrowserExtensionMatchPattern("file:///*")

        guard let url1 = URL(string: "file:///Users/test/file.txt") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url2 = URL(string: "file:///C:/Windows/file.txt") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url3 = URL(string: "https://example.com/file.txt") else {
            XCTFail("Invalid URL")
            return
        }

        XCTAssertTrue(pattern.matches(url1))
        XCTAssertTrue(pattern.matches(url2))

        XCTAssertFalse(pattern.matches(url3))
    }

    func testMatchLocalhost() throws {
        // From docs example: "*://localhost/*"
        let pattern = try BrowserExtensionMatchPattern("*://localhost/*")

        guard let url1 = URL(string: "http://localhost/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url2 = URL(string: "https://localhost/api") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url3 = URL(string: "http://localhost:3000/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url4 = URL(string: "https://localhost:8443/secure") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url5 = URL(string: "http://example.com/") else {
            XCTFail("Invalid URL")
            return
        }

        XCTAssertTrue(pattern.matches(url1))
        XCTAssertTrue(pattern.matches(url2))
        XCTAssertTrue(pattern.matches(url3))
        XCTAssertTrue(pattern.matches(url4))

        XCTAssertFalse(pattern.matches(url5))
    }

    // MARK: - Invalid Pattern Tests

    func testInvalidPatternNoScheme() {
        XCTAssertThrowsError(try BrowserExtensionMatchPattern("example.com/*")) { error in
            XCTAssertEqual(error as? BrowserExtensionMatchPattern.ParseError, .invalidFormat)
        }
    }

    func testInvalidPatternNoDelimiter() {
        XCTAssertThrowsError(try BrowserExtensionMatchPattern("https:example.com")) { error in
            XCTAssertEqual(error as? BrowserExtensionMatchPattern.ParseError, .invalidFormat)
        }
    }

    func testInvalidPatternEmpty() {
        XCTAssertThrowsError(try BrowserExtensionMatchPattern("")) { error in
            XCTAssertEqual(error as? BrowserExtensionMatchPattern.ParseError, .invalidFormat)
        }
    }

    func testInvalidPatternInvalidScheme() {
        XCTAssertThrowsError(try BrowserExtensionMatchPattern("invalid://example.com/*")) { error in
            XCTAssertEqual(error as? BrowserExtensionMatchPattern.ParseError, .invalidScheme("invalid"))
        }
    }

    // MARK: - Port Handling Tests

    func testMatchWithPort() throws {
        let pattern = try BrowserExtensionMatchPattern("http://localhost:8080/*")

        guard let url1 = URL(string: "http://localhost:8080/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url2 = URL(string: "http://localhost:8080/api") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url3 = URL(string: "http://localhost/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url4 = URL(string: "http://localhost:3000/") else {
            XCTFail("Invalid URL")
            return
        }

        XCTAssertTrue(pattern.matches(url1))
        XCTAssertTrue(pattern.matches(url2))

        XCTAssertFalse(pattern.matches(url3)) // No port
        XCTAssertFalse(pattern.matches(url4)) // Wrong port
    }

    func testMatchWildcardPort() throws {
        let pattern = try BrowserExtensionMatchPattern("http://localhost:*/*")

        guard let url1 = URL(string: "http://localhost/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url2 = URL(string: "http://localhost:8080/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url3 = URL(string: "http://localhost:3000/") else {
            XCTFail("Invalid URL")
            return
        }
        guard let url4 = URL(string: "http://example.com/") else {
            XCTFail("Invalid URL")
            return
        }

        XCTAssertTrue(pattern.matches(url1)) // Default port
        XCTAssertTrue(pattern.matches(url2))
        XCTAssertTrue(pattern.matches(url3))

        XCTAssertFalse(pattern.matches(url4))
    }

    // MARK: - Static Helper Tests

    func testIsValidMatchPattern() {
        XCTAssertTrue(BrowserExtensionMatchPattern.isValid("<all_urls>"))
        XCTAssertTrue(BrowserExtensionMatchPattern.isValid("https://example.com/*"))
        XCTAssertTrue(BrowserExtensionMatchPattern.isValid("*://*.example.com/*"))
        XCTAssertTrue(BrowserExtensionMatchPattern.isValid("file:///*"))

        XCTAssertFalse(BrowserExtensionMatchPattern.isValid("storage"))
        XCTAssertFalse(BrowserExtensionMatchPattern.isValid("tabs"))
        XCTAssertFalse(BrowserExtensionMatchPattern.isValid("example.com"))
        XCTAssertFalse(BrowserExtensionMatchPattern.isValid(""))
    }

    // MARK: - Permission Parser Integration Tests

    func testPermissionParserDetectsHostPatterns() {
        // These should be detected as host patterns
        XCTAssertTrue(BrowserExtensionPermissionParser.isHostPattern("https://example.com/*"))
        XCTAssertTrue(BrowserExtensionPermissionParser.isHostPattern("*://*.example.com/*"))
        XCTAssertTrue(BrowserExtensionPermissionParser.isHostPattern("<all_urls>"))

        // These should NOT be detected as host patterns
        XCTAssertFalse(BrowserExtensionPermissionParser.isHostPattern("storage"))
        XCTAssertFalse(BrowserExtensionPermissionParser.isHostPattern("tabs"))
        XCTAssertFalse(BrowserExtensionPermissionParser.isHostPattern("system.cpu"))
    }
}
