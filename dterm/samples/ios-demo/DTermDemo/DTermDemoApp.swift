/*
 * DTermDemoApp.swift - Sample iOS app using dterm-swift
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 *
 * This sample app demonstrates how to integrate dterm-core into an iOS/iPadOS
 * application using SwiftUI. It shows:
 *
 * - Terminal state management
 * - Cell rendering with colors and attributes
 * - Cursor display
 * - Input handling (simulated)
 * - Scrollback navigation
 */

import SwiftUI
import DTermCore

@main
struct DTermDemoApp: App {
    var body: some Scene {
        WindowGroup {
#if os(macOS)
            TabView {
                MacContentView()
                    .tabItem {
                        Label("Terminal", systemImage: "terminal")
                    }
                UIBridgeDemoView()
                    .tabItem {
                        Label("UI Bridge", systemImage: "arrow.triangle.2.circlepath")
                    }
            }
#else
            TabView {
                ContentView()
                    .tabItem {
                        Label("Terminal", systemImage: "terminal")
                    }
                UIBridgeDemoView()
                    .tabItem {
                        Label("UI Bridge", systemImage: "arrow.triangle.2.circlepath")
                    }
            }
#endif
        }
    }
}
