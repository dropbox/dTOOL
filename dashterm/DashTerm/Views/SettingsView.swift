//
//  SettingsView.swift
//  DashTerm
//
//  Application settings
//

import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var appState: AppState
    @AppStorage("terminalFontSize") private var fontSize: Double = 14
    @AppStorage("terminalFontName") private var fontName: String = "SF Mono"
    @AppStorage("terminalOpacity") private var opacity: Double = 1.0
    @AppStorage("cursorStyle") private var cursorStyle: String = "block"
    @AppStorage("cursorBlink") private var cursorBlink: Bool = true

    var body: some View {
        TabView {
            // Appearance tab
            Form {
                Section("Font") {
                    Picker("Font Family", selection: $fontName) {
                        Text("SF Mono").tag("SF Mono")
                        Text("Menlo").tag("Menlo")
                        Text("Monaco").tag("Monaco")
                        Text("Courier New").tag("Courier New")
                    }

                    HStack {
                        Text("Font Size")
                        Slider(value: $fontSize, in: 10...24, step: 1)
                        Text("\(Int(fontSize)) pt")
                            .frame(width: 40)
                    }
                }

                Section("Window") {
                    HStack {
                        Text("Opacity")
                        Slider(value: $opacity, in: 0.5...1.0)
                        Text("\(Int(opacity * 100))%")
                            .frame(width: 40)
                    }
                }

                Section("Cursor") {
                    Picker("Style", selection: $cursorStyle) {
                        Text("Block").tag("block")
                        Text("Underline").tag("underline")
                        Text("Bar").tag("bar")
                    }

                    Toggle("Blink", isOn: $cursorBlink)
                }
            }
            .tabItem {
                Label("Appearance", systemImage: "paintpalette")
            }

            // Shell tab
            Form {
                Section("Default Shell") {
                    Text("Using system default shell")
                        .foregroundColor(.gray)
                }

                Section("Environment") {
                    Text("Additional environment variables can be configured here")
                        .foregroundColor(.gray)
                }
            }
            .tabItem {
                Label("Shell", systemImage: "terminal")
            }

            // Graph tab
            Form {
                Section("Graph Visualization") {
                    Toggle("Auto-layout nodes", isOn: .constant(true))
                    Toggle("Animate transitions", isOn: .constant(true))
                    Toggle("Show timing information", isOn: .constant(true))
                }

                Section("Performance") {
                    Text("Metal acceleration: Enabled")
                        .foregroundColor(.gray)
                }
            }
            .tabItem {
                Label("Graph", systemImage: "point.3.connected.trianglepath.dotted")
            }
        }
        .padding()
        .frame(width: 450, height: 350)
    }
}

#Preview {
    SettingsView()
        .environmentObject(AppState())
}
