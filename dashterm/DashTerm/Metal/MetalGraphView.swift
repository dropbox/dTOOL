//
//  MetalGraphView.swift
//  DashTerm
//
//  NSViewRepresentable wrapper for Metal-based graph rendering
//

import SwiftUI
import MetalKit

/// SwiftUI wrapper for Metal-accelerated graph visualization
struct MetalGraphView: NSViewRepresentable {
    @ObservedObject var graph: GraphModel
    @Binding var selectedNode: String?
    @Binding var zoomLevel: CGFloat
    @Binding var panOffset: CGPoint
    var selectedGroup: Binding<String?>?
    var onGroupToggle: ((String) -> Void)?

    func makeNSView(context: Context) -> MTKView {
        guard let device = MTLCreateSystemDefaultDevice() else {
            fatalError("Metal is not supported on this device")
        }

        let mtkView = MTKView(frame: .zero, device: device)
        mtkView.clearColor = MTLClearColor(red: 0.08, green: 0.08, blue: 0.08, alpha: 1.0)
        mtkView.colorPixelFormat = .bgra8Unorm
        mtkView.enableSetNeedsDisplay = false
        mtkView.isPaused = false
        mtkView.preferredFramesPerSecond = 60

        // Create renderer
        if let renderer = GraphRenderer(device: device) {
            context.coordinator.renderer = renderer
            mtkView.delegate = renderer
            renderer.updateGraph(graph)
            renderer.setZoom(Float(zoomLevel))
            renderer.setPan(simd_float2(Float(panOffset.x), Float(panOffset.y)))
        }

        // Add gesture recognizers
        let panGesture = NSPanGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handlePan(_:)))
        mtkView.addGestureRecognizer(panGesture)

        let clickGesture = NSClickGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleClick(_:)))
        mtkView.addGestureRecognizer(clickGesture)

        // Double-click for toggling groups
        let doubleClickGesture = NSClickGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleDoubleClick(_:)))
        doubleClickGesture.numberOfClicksRequired = 2
        mtkView.addGestureRecognizer(doubleClickGesture)
        clickGesture.delaysPrimaryMouseButtonEvents = false

        // Right-click for context menu
        let rightClickGesture = NSClickGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleRightClick(_:)))
        rightClickGesture.buttonMask = 0x2  // Right mouse button
        mtkView.addGestureRecognizer(rightClickGesture)

        let magnifyGesture = NSMagnificationGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleMagnify(_:)))
        mtkView.addGestureRecognizer(magnifyGesture)

        context.coordinator.mtkView = mtkView
        context.coordinator.graph = graph

        return mtkView
    }

    func updateNSView(_ nsView: MTKView, context: Context) {
        context.coordinator.renderer?.updateGraph(graph)
        context.coordinator.renderer?.setZoom(Float(zoomLevel))
        context.coordinator.renderer?.setPan(simd_float2(Float(panOffset.x), Float(panOffset.y)))
        context.coordinator.renderer?.setSelectedNode(selectedNode, in: graph)
        context.coordinator.renderer?.setSelectedGroup(selectedGroup?.wrappedValue, in: graph)
        context.coordinator.graph = graph
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    class Coordinator: NSObject {
        var parent: MetalGraphView
        var renderer: GraphRenderer?
        var mtkView: MTKView?
        var graph: GraphModel?

        // Group dragging state
        private var draggingGroupId: String?
        private var dragStartGroupPosition: CGPoint?

        init(_ parent: MetalGraphView) {
            self.parent = parent
        }

        @objc func handlePan(_ gesture: NSPanGestureRecognizer) {
            guard let view = mtkView else { return }

            switch gesture.state {
            case .began:
                // Check if pan started on a collapsed group
                let location = gesture.location(in: view)
                let graphPoint = screenToGraph(location, in: view)
                if let clickedGroup = findGroupAt(graphPoint),
                   let groupPos = clickedGroup.position {
                    // Start dragging the group
                    draggingGroupId = clickedGroup.id
                    dragStartGroupPosition = CGPoint(x: groupPos.x, y: groupPos.y)
                } else {
                    draggingGroupId = nil
                    dragStartGroupPosition = nil
                }

            case .changed:
                let translation = gesture.translation(in: view)

                if let groupId = draggingGroupId,
                   let startPos = dragStartGroupPosition {
                    // Drag the group: move group position by translation in graph coordinates
                    let deltaX = translation.x / parent.zoomLevel
                    let deltaY = translation.y / parent.zoomLevel
                    let newPosition = Position(
                        x: startPos.x + deltaX,
                        y: startPos.y + deltaY
                    )
                    graph?.setGroupPosition(groupId, position: newPosition)
                } else {
                    // Pan the view
                    parent.panOffset.x += translation.x
                    parent.panOffset.y += translation.y
                    gesture.setTranslation(.zero, in: view)
                }

            case .ended, .cancelled:
                draggingGroupId = nil
                dragStartGroupPosition = nil

            default:
                break
            }
        }

        /// Convert screen coordinates to graph coordinates
        private func screenToGraph(_ location: CGPoint, in view: MTKView) -> CGPoint {
            let viewCenter = CGPoint(x: view.bounds.midX, y: view.bounds.midY)
            let graphX = (location.x - viewCenter.x - parent.panOffset.x) / parent.zoomLevel
            let graphY = (location.y - viewCenter.y - parent.panOffset.y) / parent.zoomLevel
            return CGPoint(x: graphX, y: graphY)
        }

        /// Find clicked node at graph coordinates (skip nodes in collapsed groups)
        private func findNodeAt(_ graphPoint: CGPoint) -> GraphNode? {
            guard let graph = graph else { return nil }
            let collapsedGroupIds = Set(graph.groups.filter { $0.collapsed }.map { $0.id })

            return graph.nodes.first { node in
                // Skip nodes in collapsed groups
                if let groupId = node.groupId, collapsedGroupIds.contains(groupId) {
                    return false
                }
                guard let pos = node.position else { return false }
                let dx = abs(pos.x - graphPoint.x)
                let dy = abs(pos.y - graphPoint.y)
                return dx < 50 && dy < 25 // Node hit area
            }
        }

        /// Find clicked collapsed group at graph coordinates
        private func findGroupAt(_ graphPoint: CGPoint) -> GraphGroup? {
            guard let graph = graph else { return nil }

            return graph.groups.filter { $0.collapsed }.first { group in
                guard let pos = group.position else { return false }
                let dx = abs(pos.x - graphPoint.x)
                let dy = abs(pos.y - graphPoint.y)
                return dx < 60 && dy < 30 // Group hit area (slightly larger than nodes)
            }
        }

        @objc func handleClick(_ gesture: NSClickGestureRecognizer) {
            guard let view = mtkView else { return }

            let location = gesture.location(in: view)
            let graphPoint = screenToGraph(location, in: view)

            // Check for collapsed groups first (they overlay nodes)
            if let clickedGroup = findGroupAt(graphPoint) {
                parent.selectedNode = nil
                parent.selectedGroup?.wrappedValue = clickedGroup.id
                return
            }

            // Find clicked node
            if let clickedNode = findNodeAt(graphPoint) {
                parent.selectedGroup?.wrappedValue = nil
                parent.selectedNode = clickedNode.id
            } else {
                parent.selectedNode = nil
                parent.selectedGroup?.wrappedValue = nil
            }
        }

        @objc func handleDoubleClick(_ gesture: NSClickGestureRecognizer) {
            guard let view = mtkView else { return }

            let location = gesture.location(in: view)
            let graphPoint = screenToGraph(location, in: view)

            // Double-click on a collapsed group toggles it
            if let clickedGroup = findGroupAt(graphPoint) {
                parent.onGroupToggle?(clickedGroup.id)
                return
            }

            // Double-click on a node that's in a group could toggle that group
            if let clickedNode = findNodeAt(graphPoint),
               let groupId = clickedNode.groupId {
                parent.onGroupToggle?(groupId)
            }
        }

        @objc func handleRightClick(_ gesture: NSClickGestureRecognizer) {
            guard let view = mtkView else { return }

            let location = gesture.location(in: view)
            let graphPoint = screenToGraph(location, in: view)

            // Check for group first
            if let clickedGroup = findGroupAt(graphPoint) {
                showGroupContextMenu(for: clickedGroup, at: location, in: view)
                return
            }

            // Check for node
            if let clickedNode = findNodeAt(graphPoint),
               let groupId = clickedNode.groupId {
                // Show context menu for node's group
                if let group = graph?.groups.first(where: { $0.id == groupId }) {
                    showGroupContextMenu(for: group, at: location, in: view)
                }
            }
        }

        private func showGroupContextMenu(for group: GraphGroup, at location: CGPoint, in view: NSView) {
            let menu = NSMenu()

            // Expand/Collapse item
            let toggleItem = NSMenuItem(
                title: group.collapsed ? "Expand Group" : "Collapse Group",
                action: #selector(toggleGroupFromMenu(_:)),
                keyEquivalent: ""
            )
            toggleItem.target = self
            toggleItem.representedObject = group.id
            menu.addItem(toggleItem)

            menu.addItem(NSMenuItem.separator())

            // Rename item
            let renameItem = NSMenuItem(
                title: "Rename Group...",
                action: #selector(renameGroupFromMenu(_:)),
                keyEquivalent: ""
            )
            renameItem.target = self
            renameItem.representedObject = group.id
            menu.addItem(renameItem)

            // Info item
            let infoItem = NSMenuItem(
                title: "\(group.nodeCount) items",
                action: nil,
                keyEquivalent: ""
            )
            infoItem.isEnabled = false
            menu.addItem(infoItem)

            // Show the menu
            menu.popUp(positioning: nil, at: location, in: view)
        }

        @objc private func toggleGroupFromMenu(_ sender: NSMenuItem) {
            guard let groupId = sender.representedObject as? String else { return }
            parent.onGroupToggle?(groupId)
        }

        @objc private func renameGroupFromMenu(_ sender: NSMenuItem) {
            guard let groupId = sender.representedObject as? String,
                  let group = graph?.groups.first(where: { $0.id == groupId }),
                  let window = mtkView?.window else { return }

            // Create a simple rename alert
            let alert = NSAlert()
            alert.messageText = "Rename Group"
            alert.informativeText = "Enter a new name for the group:"
            alert.addButton(withTitle: "Rename")
            alert.addButton(withTitle: "Cancel")

            let textField = NSTextField(frame: NSRect(x: 0, y: 0, width: 200, height: 24))
            textField.stringValue = group.label
            alert.accessoryView = textField

            alert.beginSheetModal(for: window) { [weak self] response in
                if response == .alertFirstButtonReturn {
                    let newName = textField.stringValue
                    if !newName.isEmpty {
                        self?.renameGroup(groupId, to: newName)
                    }
                }
            }
        }

        private func renameGroup(_ groupId: String, to newLabel: String) {
            guard let graph = graph,
                  let index = graph.groups.firstIndex(where: { $0.id == groupId }) else { return }
            graph.groups[index].label = newLabel
        }

        @objc func handleMagnify(_ gesture: NSMagnificationGestureRecognizer) {
            let newZoom = parent.zoomLevel * (1.0 + gesture.magnification)
            parent.zoomLevel = max(0.25, min(4.0, newZoom))
            gesture.magnification = 0
        }
    }
}
