//
//  ContentView.swift
//  DashTerm
//
//  Main content view with terminal and optional graph panel
//

import SwiftUI

struct ContentView: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        HSplitView {
            // Main terminal area
            terminalArea
                .frame(minWidth: 400)

            // Graph panel (optional)
            if appState.showGraphPanel {
                GraphPanelView()
                    .frame(minWidth: 300, idealWidth: 400)
            }
        }
        .frame(minWidth: 800, minHeight: 600)
        .background(Color.black)
    }

    @ViewBuilder
    private var terminalArea: some View {
        VStack(spacing: 0) {
            // Tab bar
            TabBarView()
                .frame(height: 36)

            // Terminal content
            if let activeId = appState.activeTerminalId,
               let session = appState.terminals.first(where: { $0.id == activeId }) {
                TerminalView(session: session)
            } else {
                EmptyTerminalView()
            }
        }
    }
}

struct TabBarView: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 0) {
                ForEach(appState.terminals) { terminal in
                    TabItemView(
                        terminal: terminal,
                        isActive: terminal.id == appState.activeTerminalId
                    )
                }

                // New tab button
                Button(action: { appState.createNewTerminal() }) {
                    Image(systemName: "plus")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundColor(.gray)
                        .frame(width: 28, height: 28)
                }
                .buttonStyle(.plain)

                Spacer()
            }
            .padding(.horizontal, 8)
        }
        .background(Color(white: 0.1))
    }
}

struct TabItemView: View {
    @EnvironmentObject var appState: AppState
    let terminal: TerminalSession
    let isActive: Bool

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "terminal")
                .font(.system(size: 10))

            Text(terminal.title)
                .font(.system(size: 12))
                .lineLimit(1)

            if appState.terminals.count > 1 {
                Button(action: { appState.closeTerminal(terminal.id) }) {
                    Image(systemName: "xmark")
                        .font(.system(size: 8, weight: .bold))
                        .foregroundColor(.gray)
                }
                .buttonStyle(.plain)
                .opacity(isActive ? 1 : 0)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(isActive ? Color(white: 0.2) : Color.clear)
        .cornerRadius(4)
        .foregroundColor(isActive ? .white : .gray)
        .onTapGesture {
            appState.activeTerminalId = terminal.id
        }
    }
}

struct EmptyTerminalView: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        VStack {
            Text("No Terminal")
                .font(.headline)
                .foregroundColor(.gray)

            Button("Create New Terminal") {
                appState.createNewTerminal()
            }
            .buttonStyle(.bordered)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.black)
    }
}

#Preview {
    ContentView()
        .environmentObject(AppState())
}
