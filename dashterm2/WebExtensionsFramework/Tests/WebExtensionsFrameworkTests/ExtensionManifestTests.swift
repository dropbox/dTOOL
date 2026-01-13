import XCTest
@testable import WebExtensionsFramework

final class ExtensionManifestTests: XCTestCase {

    // MARK: - manifest_version tests (manifest_version.md)

    func testManifestVersionDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertEqual(manifest.manifestVersion, 3)
    }


    func testManifestVersionRequired() {
        let json = """
        {
        }
        """

        let data = Data(json.utf8)

        XCTAssertThrowsError(try JSONDecoder().decode(ExtensionManifest.self, from: data))
    }

    // MARK: - name tests (name.md)

    func testNameDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Red Box",
            "version": "1.0"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertEqual(manifest.name, "Red Box")
    }

    func testNameRequired() {
        let json = """
        {
            "manifest_version": 3
        }
        """

        let data = Data(json.utf8)

        XCTAssertThrowsError(try JSONDecoder().decode(ExtensionManifest.self, from: data))
    }

    // MARK: - version tests (version.md)

    func testVersionDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Red Box",
            "version": "1.0"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertEqual(manifest.version, "1.0")
    }

    func testVersionRequired() {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension"
        }
        """

        let data = Data(json.utf8)

        XCTAssertThrowsError(try JSONDecoder().decode(ExtensionManifest.self, from: data))
    }

    // MARK: - description tests (description.md)

    func testDescriptionDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Red Box",
            "version": "1.0",
            "description": "Adds a red box to the top of every page"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertEqual(manifest.description, "Adds a red box to the top of every page")
    }

    func testDescriptionOptional() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNil(manifest.description)
    }

    // MARK: - content_scripts tests (content_scripts.md)

    func testContentScriptsDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Red Box",
            "version": "1.0",
            "content_scripts": [{
                "matches": ["<all_urls>"],
                "js": ["content.js"],
                "run_at": "document_end"
            }]
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertEqual(manifest.contentScripts?.count, 1)
        guard let contentScripts = manifest.contentScripts, !contentScripts.isEmpty else {
            XCTFail("Expected content scripts")
            return
        }
        let contentScript = contentScripts[0]
        XCTAssertEqual(contentScript.matches, ["<all_urls>"])
        XCTAssertEqual(contentScript.js, ["content.js"])
        XCTAssertEqual(contentScript.runAt, .documentEnd)
    }

    func testContentScriptsOptional() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNil(manifest.contentScripts)
    }

    func testContentScriptsMinimalRequired() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0",
            "content_scripts": [{
                "matches": ["https://example.com/*"]
            }]
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertEqual(manifest.contentScripts?.count, 1)
        guard let contentScripts = manifest.contentScripts, !contentScripts.isEmpty else {
            XCTFail("Expected content scripts")
            return
        }
        let contentScript = contentScripts[0]
        XCTAssertEqual(contentScript.matches, ["https://example.com/*"])
        XCTAssertNil(contentScript.js)
        XCTAssertNil(contentScript.css)
        XCTAssertNil(contentScript.runAt)
    }

    // MARK: - background tests (background.md)

    func testBackgroundServiceWorkerDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0",
            "background": {
                "service_worker": "background.js"
            }
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNotNil(manifest.background)
        XCTAssertEqual(manifest.background?.serviceWorker, "background.js")
        XCTAssertNil(manifest.background?.scripts)
        XCTAssertNil(manifest.background?.persistent)
        XCTAssertNil(manifest.background?.type)
    }

    func testBackgroundLegacyScriptsDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0",
            "background": {
                "scripts": ["background.js", "utils.js"],
                "persistent": false
            }
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNotNil(manifest.background)
        XCTAssertNil(manifest.background?.serviceWorker)
        XCTAssertEqual(manifest.background?.scripts, ["background.js", "utils.js"])
        XCTAssertEqual(manifest.background?.persistent, false)
        XCTAssertNil(manifest.background?.type)
    }

    func testBackgroundWithTypeDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0",
            "background": {
                "service_worker": "background.js",
                "type": "module"
            }
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNotNil(manifest.background)
        XCTAssertEqual(manifest.background?.serviceWorker, "background.js")
        XCTAssertEqual(manifest.background?.type, "module")
    }

    func testBackgroundOptional() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNil(manifest.background)
    }

    // MARK: - permissions tests

    func testPermissionsDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0",
            "permissions": ["storage", "tabs", "activeTab"]
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNotNil(manifest.permissions)
        XCTAssertEqual(manifest.permissions, ["storage", "tabs", "activeTab"])
    }

    func testPermissionsOptional() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNil(manifest.permissions)
    }

    func testPermissionsEmpty() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0",
            "permissions": []
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNotNil(manifest.permissions)
        XCTAssertEqual(manifest.permissions, [])
    }

    func testHostPermissionsDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0",
            "host_permissions": ["https://*.example.com/*", "*://localhost/*"]
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNotNil(manifest.hostPermissions)
        XCTAssertEqual(manifest.hostPermissions, ["https://*.example.com/*", "*://localhost/*"])
    }

    func testHostPermissionsOptional() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNil(manifest.hostPermissions)
    }

    func testOptionalPermissionsDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0",
            "optional_permissions": ["geolocation", "bookmarks"]
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNotNil(manifest.optionalPermissions)
        XCTAssertEqual(manifest.optionalPermissions, ["geolocation", "bookmarks"])
    }

    func testOptionalPermissionsOptional() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNil(manifest.optionalPermissions)
    }

    func testOptionalHostPermissionsDecoding() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0",
            "optional_host_permissions": ["https://*.google.com/*"]
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNotNil(manifest.optionalHostPermissions)
        XCTAssertEqual(manifest.optionalHostPermissions, ["https://*.google.com/*"])
    }

    func testOptionalHostPermissionsOptional() throws {
        let json = """
        {
            "manifest_version": 3,
            "name": "Test Extension",
            "version": "1.0"
        }
        """

        let data = Data(json.utf8)
        let manifest = try JSONDecoder().decode(ExtensionManifest.self, from: data)

        XCTAssertNil(manifest.optionalHostPermissions)
    }
}
