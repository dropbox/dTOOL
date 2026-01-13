// swift-tools-version: 5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "DTermDemo",
    platforms: [
        .iOS(.v16),
        .macOS(.v13)
    ],
    products: [
        .executable(
            name: "DTermDemo",
            targets: ["DTermDemo"]
        ),
    ],
    dependencies: [
        .package(path: "../../packages/dterm-swift"),
    ],
    targets: [
        .executableTarget(
            name: "DTermDemo",
            dependencies: [
                .product(name: "DTermCore", package: "dterm-swift"),
            ],
            path: "DTermDemo",
            linkerSettings: [
                .unsafeFlags(["-L/Users/ayates/dterm/target/release", "-ldterm_core"]),
            ]
        )
    ]
)
