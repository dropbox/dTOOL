// swift-tools-version:5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

// Path to the dterm-core static library
let dtermLibPath = "/Users/ayates/dterm/target/release"

let package = Package(
    name: "DTermCore",
    platforms: [
        .iOS(.v15),
        .macOS(.v12),
        .tvOS(.v15),
        .watchOS(.v8),
        .visionOS(.v1)
    ],
    products: [
        // The main Swift library wrapping dterm-core
        .library(
            name: "DTermCore",
            targets: ["DTermCore"]
        ),
    ],
    targets: [
        // C bridging target for dterm-core FFI
        .target(
            name: "CDTermCore",
            path: "Sources/CDTermCore",
            publicHeadersPath: "include",
            cSettings: [
                .define("DTERM_FFI", to: "1"),
                .define("DTERM_GPU", to: "1")
            ],
            linkerSettings: [
                .unsafeFlags(["-L\(dtermLibPath)", "-ldterm_core"]),
            ]
        ),
        // Swift wrapper for dterm-core
        .target(
            name: "DTermCore",
            dependencies: ["CDTermCore"],
            path: "Sources/DTermCore"
        ),
        // Tests
        .testTarget(
            name: "DTermCoreTests",
            dependencies: ["DTermCore"]
        ),
    ]
)
