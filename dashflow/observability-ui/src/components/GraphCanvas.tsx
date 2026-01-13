import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ReactFlow,
  Node,
  Edge,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  MarkerType,
  Position,
  NodeTypes,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import dagre from 'dagre';
import { GraphNode } from './GraphNode';
import { GraphCanvasPlaceholder } from './GraphCanvasPlaceholder';
import {
  GraphSchema,
  NodeExecution,
  NodeStatus,
  NODE_TYPE_STYLES,
  NODE_STATUS_STYLES,
} from '../types/graph';
import { computeNodeGroups, GroupingOptions } from '../utils/grouping';
import { GroupNode } from './GroupNode';
import { SchemaDiff, getNodeDiffStatus } from '../types/schemaDiff';
import { colors } from '../styles/tokens';

// Properly typed node types for React Flow
const nodeTypes: NodeTypes = {
  graphNode: GraphNode,
  groupNode: GroupNode,
};

// Node dimensions must match actual rendered size to prevent overlap
// GraphNode has w-[160px] fixed width and ~100px height with compact content
const nodeWidth = 170;
const nodeHeight = 110;
const groupPadding = 28;

const GROUP_PALETTE = [
  { bg: 'rgba(59, 130, 246, 0.06)', border: 'rgba(59, 130, 246, 0.25)' }, // blue
  { bg: 'rgba(34, 197, 94, 0.06)', border: 'rgba(34, 197, 94, 0.22)' }, // green
  { bg: 'rgba(245, 158, 11, 0.06)', border: 'rgba(245, 158, 11, 0.25)' }, // amber
  { bg: 'rgba(139, 92, 246, 0.06)', border: 'rgba(139, 92, 246, 0.25)' }, // violet
  { bg: 'rgba(236, 72, 153, 0.06)', border: 'rgba(236, 72, 153, 0.25)' }, // pink
];

// Create fresh dagre graph on each layout to avoid stale nodes/edges
// contaminating layout after schema changes
function getLayoutedElements(
  nodes: Node[],
  edges: Edge[],
  direction = 'TB'
): { nodes: Node[]; edges: Edge[] } {
  // Create a fresh graph for each layout to prevent contamination
  const dagreGraph = new dagre.graphlib.Graph();
  dagreGraph.setDefaultEdgeLabel(() => ({}));

  const isHorizontal = direction === 'LR';
  // Increased spacing to prevent overlap and improve readability
  // nodesep: horizontal gap between nodes in same rank
  // ranksep: vertical gap between ranks (levels)
  dagreGraph.setGraph({ rankdir: direction, nodesep: 80, ranksep: 120 });

  nodes.forEach((node) => {
    dagreGraph.setNode(node.id, { width: nodeWidth, height: nodeHeight });
  });

  edges.forEach((edge) => {
    dagreGraph.setEdge(edge.source, edge.target);
  });

  dagre.layout(dagreGraph);

  const layoutedNodes = nodes.map((node) => {
    const nodeWithPosition = dagreGraph.node(node.id);
    return {
      ...node,
      targetPosition: isHorizontal ? Position.Left : Position.Top,
      sourcePosition: isHorizontal ? Position.Right : Position.Bottom,
      position: {
        x: nodeWithPosition.x - nodeWidth / 2,
        y: nodeWithPosition.y - nodeHeight / 2,
      },
    };
  });

  return { nodes: layoutedNodes, edges };
}

interface GraphCanvasProps {
  schema: GraphSchema | null;
  nodeExecutions: Record<string, NodeExecution>;
  currentNode?: string;
  onNodeClick?: (nodeName: string) => void;
  selectedNode?: string;
  grouping?: GroupingOptions;
  // M-39: Schema diff for highlighting expected vs observed differences
  schemaDiff?: SchemaDiff;
  // M-39: Set of node names that were observed (referenced in events) but not in schema
  outOfSchemaNodes?: Set<string>;
}

export function GraphCanvas({
  schema,
  nodeExecutions,
  currentNode,
  onNodeClick,
  selectedNode,
  grouping = { mode: 'none' },
  schemaDiff,
  outOfSchemaNodes,
}: GraphCanvasProps) {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  // M-463: Track focused node for keyboard navigation
  const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // M-458: Compute a stable schema structure key that only changes when nodes/edges change.
  // This prevents expensive dagre layout on every execution state update.
  const schemaStructureKey = useMemo(() => {
    if (!schema) return '';
    const nodeNames = schema.nodes.map((n) => n.name).sort().join(',');
    const edgeKeys = schema.edges
      .map((e) => {
        const targets = e.conditional_targets?.sort().join('+') || e.to;
        return `${e.from}->${targets}`;
      })
      .sort()
      .join(';');
    return `${nodeNames}|${edgeKeys}|${grouping.mode}|${grouping.attributeKey || ''}`;
  }, [schema, grouping.mode, grouping.attributeKey]);

  // Store layouted positions so we can update node data without relayout
  const layoutedPositionsRef = useRef<Map<string, { x: number; y: number }>>(new Map());
  const groupNodesRef = useRef<Node[]>([]);

  // M-458: Effect 1 - Layout computation. Only runs when schema STRUCTURE changes.
  // This is the expensive dagre operation.
  useEffect(() => {
    if (!schema) {
      layoutedPositionsRef.current = new Map();
      groupNodesRef.current = [];
      setNodes([]);
      setEdges([]);
      return;
    }

    // Create nodes with placeholder data (will be updated by Effect 2)
    const flowNodes: Node[] = schema.nodes.map((node) => ({
      id: node.name,
      type: 'graphNode',
      position: { x: 0, y: 0 },
      data: {
        label: node.name,
        description: node.description,
        nodeType: node.node_type,
        status: 'pending' as NodeStatus,
        isActive: false,
        isSelected: false,
        duration: undefined,
        icon: (NODE_TYPE_STYLES[node.node_type] || NODE_TYPE_STYLES.transform).icon,
        // M-472: Pass aria-label for screen reader accessibility
        iconAriaLabel: (NODE_TYPE_STYLES[node.node_type] || NODE_TYPE_STYLES.transform).ariaLabel,
        bgColor: (NODE_TYPE_STYLES[node.node_type] || NODE_TYPE_STYLES.transform).bgColor,
        borderColor: NODE_STATUS_STYLES.pending.borderColor,
        inputFields: node.input_fields,
        outputFields: node.output_fields,
        diffStatus: undefined,
        // TYP-07: Pass source location from node attributes if available
        sourceLocation: node.attributes?.source || node.attributes?.file || undefined,
      },
    }));

    // Create edges (structural only - animation state updated in Effect 2)
    // M-470: Use stable IDs based on edge content, not array index, to prevent
    // React key changes when schema edges are reordered.
    const flowEdges: Edge[] = schema.edges.flatMap((edge) => {
      const isConditional = edge.edge_type === 'conditional';
      const isParallel = edge.edge_type === 'parallel';
      const edgeType = edge.edge_type || 'default';

      if (edge.conditional_targets && edge.conditional_targets.length > 0) {
        // M-470: idx is stable within conditional_targets array
        // V-06: Enlarged arrow markers for better visibility (width/height 20px)
        // Edge visibility: smoothstep for curved routing, thicker strokes, brighter default color
        const edgeColor = isParallel ? colors.graph.parallel : isConditional ? colors.graph.conditional : '#94a3b8';
        return edge.conditional_targets.map((target, idx) => ({
          id: `${edge.from}-${target}-${edgeType}-${idx}`,
          source: edge.from,
          target: target,
          type: 'smoothstep',
          animated: isConditional,
          className: '',
          style: {
            stroke: edgeColor,
            strokeWidth: 3,
          },
          markerEnd: {
            type: MarkerType.ArrowClosed,
            color: edgeColor,
            width: 20,
            height: 20,
          },
          label: isConditional ? edge.label : undefined,
        }));
      }

      // M-470: Include edge type in ID to handle multiple edges between same nodes
      // V-06: Enlarged arrow markers for better visibility (width/height 20px)
      // Edge visibility: smoothstep for curved routing, thicker strokes, brighter default color
      const defaultEdgeColor = '#94a3b8'; // slate-400 for better visibility on dark background
      return [{
        id: `${edge.from}-${edge.to}-${edgeType}`,
        source: edge.from,
        target: edge.to,
        type: 'smoothstep',
        animated: false,
        className: '',
        style: { stroke: defaultEdgeColor, strokeWidth: 3 },
        markerEnd: { type: MarkerType.ArrowClosed, color: defaultEdgeColor, width: 20, height: 20 },
        label: undefined,
      }];
    });

    // Apply dagre layout (expensive, only on structure change)
    const { nodes: layoutedNodes, edges: layoutedEdges } = getLayoutedElements(
      flowNodes,
      flowEdges
    );

    // Store positions for reuse
    layoutedPositionsRef.current = new Map(
      layoutedNodes.map((n) => [n.id, n.position])
    );

    // Compute group nodes if grouping enabled
    if (grouping.mode !== 'none') {
      const groups = computeNodeGroups(schema, grouping).filter((g) => g.nodes.length >= 2);
      const groupNodes: Node[] = [];

      for (let idx = 0; idx < groups.length; idx++) {
        const group = groups[idx];
        const palette = GROUP_PALETTE[idx % GROUP_PALETTE.length];
        const groupNodeId = `__group__${group.key.replace(/[^a-zA-Z0-9_]/g, '_')}`;

        let minX = Number.POSITIVE_INFINITY;
        let minY = Number.POSITIVE_INFINITY;
        let maxX = Number.NEGATIVE_INFINITY;
        let maxY = Number.NEGATIVE_INFINITY;

        for (const member of group.nodes) {
          const pos = layoutedPositionsRef.current.get(member.name);
          if (!pos) continue;
          minX = Math.min(minX, pos.x);
          minY = Math.min(minY, pos.y);
          maxX = Math.max(maxX, pos.x + nodeWidth);
          maxY = Math.max(maxY, pos.y + nodeHeight);
        }

        if (!Number.isFinite(minX) || !Number.isFinite(minY)) continue;

        groupNodes.push({
          id: groupNodeId,
          type: 'groupNode',
          position: { x: minX - groupPadding, y: minY - groupPadding },
          draggable: false,
          selectable: false,
          connectable: false,
          focusable: false,
          data: {
            label: group.label,
            count: group.nodes.length,
            backgroundColor: palette.bg,
            borderColor: palette.border,
          },
          style: {
            width: (maxX - minX) + groupPadding * 2,
            height: (maxY - minY) + groupPadding * 2,
            zIndex: 0,
          },
        });
      }
      groupNodesRef.current = groupNodes;
    } else {
      groupNodesRef.current = [];
    }

    setEdges(layoutedEdges);
    // Don't setNodes here - Effect 2 will handle it with proper execution state
  }, [schemaStructureKey, setEdges]); // Only depends on structure, not execution state

  // M-458: Effect 2 - Update node visual state. Runs on execution state changes.
  // This is cheap - just updates node data without recomputing layout.
  useEffect(() => {
    if (!schema || layoutedPositionsRef.current.size === 0) {
      return;
    }

    const flowNodes: Node[] = schema.nodes.map((node) => {
      const execution = nodeExecutions[node.name];
      const status: NodeStatus = execution?.status || 'pending';
      const isActive = currentNode === node.name;
      const style = NODE_TYPE_STYLES[node.node_type] || NODE_TYPE_STYLES.transform;
      const statusStyle = NODE_STATUS_STYLES[status];
      const diffStatus = getNodeDiffStatus(node.name, schemaDiff, outOfSchemaNodes);
      const position = layoutedPositionsRef.current.get(node.name) || { x: 0, y: 0 };
      // M-463: Track keyboard focus state
      const isFocused = focusedNodeId === node.name;

      return {
        id: node.name,
        type: 'graphNode',
        position,
        data: {
          label: node.name,
          description: node.description,
          nodeType: node.node_type,
          status,
          isActive,
          isSelected: selectedNode === node.name,
          isFocused, // M-463: Pass focus state to node
          duration: execution?.duration_ms,
          icon: style.icon,
          // M-472: Pass aria-label for screen reader accessibility
          iconAriaLabel: style.ariaLabel,
          bgColor: style.bgColor,
          borderColor: isActive ? statusStyle.pulseColor || statusStyle.borderColor : statusStyle.borderColor,
          inputFields: node.input_fields,
          outputFields: node.output_fields,
          diffStatus,
          // TYP-07: Pass source location from node attributes if available
          sourceLocation: node.attributes?.source || node.attributes?.file || undefined,
        },
        ...(grouping.mode !== 'none' ? { style: { zIndex: 1 } } : {}),
      };
    });

    // Update edge animation state based on execution
    setEdges((currentEdges) =>
      currentEdges.map((edge) => {
        const sourceExecution = nodeExecutions[edge.source];
        const targetExecution = nodeExecutions[edge.target];
        const isEdgeActive = sourceExecution?.status === 'completed' &&
          (targetExecution?.status === 'active' || edge.target === currentNode);

        const isConditional = edge.style?.stroke === colors.graph.conditional;
        const isParallel = edge.style?.stroke === colors.graph.parallel;

        // V-06: Enlarged arrow markers for better visibility
        // Edge visibility: brighter default color, thicker strokes
        const baseColor = '#94a3b8'; // slate-400 for better visibility on dark background
        const edgeColor = isEdgeActive ? colors.graph.active : isParallel ? colors.graph.parallel : isConditional ? colors.graph.conditional : baseColor;
        return {
          ...edge,
          animated: isEdgeActive || isConditional,
          className: isEdgeActive ? 'edge-flowing edge-active' : '',
          style: {
            stroke: edgeColor,
            strokeWidth: isEdgeActive ? 4 : 3,
          },
          markerEnd: {
            type: MarkerType.ArrowClosed,
            color: edgeColor,
            width: 20,
            height: 20,
          },
        };
      })
    );

    // Combine group nodes with data nodes
    const allNodes = grouping.mode !== 'none'
      ? [...groupNodesRef.current, ...flowNodes]
      : flowNodes;

    setNodes(allNodes);
  }, [schema, nodeExecutions, currentNode, selectedNode, schemaDiff, outOfSchemaNodes, grouping.mode, focusedNodeId, setNodes, setEdges]);

  const handleNodeClick = useCallback(
    (_: React.MouseEvent, node: Node) => {
      if (node.type === 'groupNode') return;
      onNodeClick?.(node.id);
    },
    [onNodeClick]
  );

  // M-463: Get sorted list of node names for keyboard navigation
  const nodeNames = useMemo(() => {
    if (!schema) return [];
    return schema.nodes.map(n => n.name);
  }, [schema]);

  // M-463: Keyboard navigation handler
  const handleKeyDown = useCallback((event: React.KeyboardEvent) => {
    if (nodeNames.length === 0) return;

    const currentIndex = focusedNodeId ? nodeNames.indexOf(focusedNodeId) : -1;

    switch (event.key) {
      case 'Tab': {
        event.preventDefault();
        if (event.shiftKey) {
          // Shift+Tab: move to previous node
          const prevIndex = currentIndex <= 0 ? nodeNames.length - 1 : currentIndex - 1;
          setFocusedNodeId(nodeNames[prevIndex]);
        } else {
          // Tab: move to next node
          const nextIndex = currentIndex < 0 || currentIndex >= nodeNames.length - 1 ? 0 : currentIndex + 1;
          setFocusedNodeId(nodeNames[nextIndex]);
        }
        break;
      }
      case 'ArrowDown':
      case 'ArrowRight': {
        event.preventDefault();
        const nextIndex = currentIndex < 0 || currentIndex >= nodeNames.length - 1 ? 0 : currentIndex + 1;
        setFocusedNodeId(nodeNames[nextIndex]);
        break;
      }
      case 'ArrowUp':
      case 'ArrowLeft': {
        event.preventDefault();
        const prevIndex = currentIndex <= 0 ? nodeNames.length - 1 : currentIndex - 1;
        setFocusedNodeId(nodeNames[prevIndex]);
        break;
      }
      case 'Home': {
        event.preventDefault();
        setFocusedNodeId(nodeNames[0]);
        break;
      }
      case 'End': {
        event.preventDefault();
        setFocusedNodeId(nodeNames[nodeNames.length - 1]);
        break;
      }
      case 'Enter':
      case ' ': {
        event.preventDefault();
        if (focusedNodeId) {
          onNodeClick?.(focusedNodeId);
        }
        break;
      }
      case 'Escape': {
        event.preventDefault();
        setFocusedNodeId(null);
        break;
      }
    }
  }, [nodeNames, focusedNodeId, onNodeClick]);

  // M-463: Handle focus when container receives focus
  const handleFocus = useCallback(() => {
    if (!focusedNodeId && nodeNames.length > 0) {
      // Focus first node when container is focused
      setFocusedNodeId(nodeNames[0]);
    }
  }, [focusedNodeId, nodeNames]);

  if (!schema) {
    return <GraphCanvasPlaceholder />;
  }

  return (
    // M-463: Accessible container with keyboard navigation
    <div
      ref={containerRef}
      style={{ height: '100%', width: '100%' }}
      tabIndex={0}
      role="application"
      aria-label={`Graph visualization with ${nodeNames.length} nodes. Use arrow keys to navigate, Enter to select, Escape to clear focus.`}
      onKeyDown={handleKeyDown}
      onFocus={handleFocus}
    >
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onNodeClick={handleNodeClick}
        nodeTypes={nodeTypes}
        fitView
        // V-07: Add padding to center graph with space around edges
        fitViewOptions={{ padding: 0.15, minZoom: 0.5, maxZoom: 1.5 }}
        attributionPosition="bottom-left"
      >
        {/* V-03: Reduced visual noise - subtle lines instead of dots, larger gap */}
        {/* GN-03: Dark subtle grid lines that blend with canvas */}
        <Background variant={BackgroundVariant.Lines} color={colors.border.muted} gap={32} />
        <Controls />
        {/* V-02: Enhanced minimap with larger nodes and better styling */}
        <MiniMap
          nodeColor={(node) => {
            const status = node.data?.status as NodeStatus;
            if (status === 'active') return colors.graph.active;
            if (status === 'completed') return colors.graph.completed;
            if (status === 'error') return colors.graph.error;
            return colors.graph.pending;
          }}
          nodeStrokeColor={(node) => {
            const status = node.data?.status as NodeStatus;
            if (status === 'active') return colors.graph.activeStroke;
            if (status === 'completed') return colors.graph.completedStroke;
            if (status === 'error') return colors.graph.errorStroke;
            return colors.graph.pendingStroke;
          }}
          nodeStrokeWidth={2}
          nodeBorderRadius={4}
          // GN-05: Higher contrast mask to clearly show viewport area
          maskColor="rgba(0, 0, 0, 0.6)"
          // GN-04: Dark background matching canvas theme
          style={{ backgroundColor: colors.bg.primary }}
        />
      </ReactFlow>
      {/* M-463: Screen reader announcement for focused node */}
      {focusedNodeId && (
        <div
          role="status"
          aria-live="polite"
          style={{ position: 'absolute', left: '-9999px', width: '1px', height: '1px', overflow: 'hidden' }}
        >
          {`Node ${focusedNodeId} focused${nodeExecutions[focusedNodeId]?.status ? `, status: ${nodeExecutions[focusedNodeId].status}` : ''}`}
        </div>
      )}
    </div>
  );
}

export default GraphCanvas;
