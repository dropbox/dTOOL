// Mermaid renderer for graph visualization
// Produces Mermaid diagram text from GraphViewModel
// Uses same viewModel as Canvas, just different rendering

import { GraphViewModel, NodeState } from '../hooks/useRunStateStore';
import { GraphSchema, NodeStatus, NodeType } from '../types/graph';
import { computeNodeGroups, GroupingOptions } from './grouping';

// Mermaid node shape based on node type
function getNodeShape(nodeType: NodeType): { open: string; close: string } {
  switch (nodeType) {
    case 'llm':
      return { open: '([', close: '])' }; // Stadium (rounded)
    case 'tool':
      return { open: '{{', close: '}}' }; // Hexagon
    case 'router':
      return { open: '{', close: '}' }; // Diamond
    case 'aggregator':
      return { open: '[/', close: '/]' }; // Parallelogram (alt)
    case 'validator':
      return { open: '[[', close: ']]' }; // Subroutine
    case 'human_in_loop':
      return { open: '(((', close: ')))' }; // Circle (double)
    case 'checkpoint':
      return { open: '[(', close: ')]' }; // Cylinder
    case 'transform':
    case 'custom':
    default:
      return { open: '[', close: ']' }; // Rectangle
  }
}

// Status indicator for node label
function getStatusIndicator(status: NodeStatus): string {
  switch (status) {
    case 'active':
      return ' ⚡';
    case 'completed':
      return ' ✓';
    case 'error':
      return ' ✗';
    case 'pending':
    default:
      return '';
  }
}

// Escape special characters for Mermaid (M-454: comprehensive escaping)
// Uses whitelist approach: only allow safe characters, encode everything else.
// Mermaid uses #code; format for HTML entities.
//
// Characters that MUST be escaped inside quoted strings:
// - " (quotes) - closes the label string
// - \n \r (newlines) - breaks out to new line (syntax injection)
// - # (hash) - entity prefix in Mermaid
//
// Characters that are additionally dangerous for Mermaid syntax:
// - ] [ (brackets) - closes/opens node shapes
// - ; (semicolon) - statement separator
// - | (pipe) - edge label separator |"label"|
// - { } (braces) - diamond/hexagon shapes
// - ( ) (parens) - stadium/circle shapes
// - -- (double dash) - edge arrow syntax
//
// We use a whitelist of safe characters (alphanumeric, space, basic punctuation)
// and encode everything else to prevent any possible injection.
const SAFE_CHARS = /^[a-zA-Z0-9 _.,:!?'@$%^&*+=~`/-]$/;

function escapeMermaid(text: string): string {
  // Replace newlines with space first (most common injection vector)
  const withoutNewlines = text.replace(/[\r\n]+/g, ' ');

  // Build escaped string character by character
  let result = '';
  for (const char of withoutNewlines) {
    if (SAFE_CHARS.test(char)) {
      // Safe character - pass through
      result += char;
    } else {
      // Unsafe character - encode to Mermaid entity format #code;
      result += `#${char.charCodeAt(0)};`;
    }
  }
  return result;
}

// Generate safe node ID (Mermaid doesn't like some characters)
function baseSafeId(name: string): string {
  const sanitized = name.replace(/[^a-zA-Z0-9_]/g, '_');
  return sanitized.length > 0 ? sanitized : 'node';
}

function buildUniqueIdMap(names: string[]): Map<string, string> {
  const sorted = [...names].sort((a, b) => a.localeCompare(b));
  const used = new Map<string, number>();
  const mapping = new Map<string, string>();

  for (const name of sorted) {
    const base = baseSafeId(name);
    const count = (used.get(base) || 0) + 1;
    used.set(base, count);
    mapping.set(name, count === 1 ? base : `${base}_${count}`);
  }

  return mapping;
}

// Arrow style based on edge type
function getArrowStyle(edgeType: 'direct' | 'conditional' | 'parallel'): string {
  switch (edgeType) {
    case 'conditional':
      return '-.->'; // Dotted arrow
    case 'parallel':
      return '===>'; // Thick arrow
    case 'direct':
    default:
      return '-->'; // Normal arrow
  }
}

export interface MermaidOptions {
  direction?: 'TD' | 'TB' | 'BT' | 'LR' | 'RL';
  showDurations?: boolean;
  showStatusIndicators?: boolean;
  includeStyleDefs?: boolean;
  theme?: 'default' | 'forest' | 'dark' | 'neutral';
  grouping?: GroupingOptions;
}

const DEFAULT_OPTIONS: MermaidOptions = {
  direction: 'TD',
  showDurations: true,
  showStatusIndicators: true,
  includeStyleDefs: true,
  theme: 'default',
};

/**
 * Render a GraphSchema to Mermaid flowchart text
 */
export function renderSchemaToMermaid(
  schema: GraphSchema,
  nodeStates?: Map<string, NodeState>,
  currentNode?: string,
  options: MermaidOptions = {},
): string {
  const opts = { ...DEFAULT_OPTIONS, ...options };
  const lines: string[] = [];
  const nodeIdMap = buildUniqueIdMap(schema.nodes.map((n) => n.name));

  // Header
  lines.push(`flowchart ${opts.direction}`);
  lines.push('');

  // Style definitions
  if (opts.includeStyleDefs) {
    lines.push('  %% Status styles');
    lines.push('  classDef pending fill:#f9fafb,stroke:#d1d5db,color:#374151');
    lines.push('  classDef active fill:#dbeafe,stroke:#3b82f6,color:#1e40af,stroke-width:2px');
    lines.push('  classDef completed fill:#dcfce7,stroke:#22c55e,color:#166534');
    lines.push('  classDef error fill:#fee2e2,stroke:#ef4444,color:#b91c1c');
    lines.push('  classDef current stroke:#fbbf24,stroke-width:3px');
    lines.push('');
    lines.push('  %% Node type styles');
    lines.push('  classDef llmNode fill:#ede9fe,stroke:#7c3aed,color:#5b21b6');
    lines.push('  classDef toolNode fill:#dbeafe,stroke:#2563eb,color:#1d4ed8');
    lines.push('  classDef routerNode fill:#fee2e2,stroke:#dc2626,color:#b91c1c');
    lines.push('');
  }

  // Nodes
  lines.push('  %% Nodes');
  for (const node of schema.nodes) {
    const safeId = nodeIdMap.get(node.name)!;
    const shape = getNodeShape(node.node_type);
    const nodeState = nodeStates?.get(node.name);
    const status = nodeState?.status || 'pending';

    // Build label
    let label = escapeMermaid(node.name);
    if (opts.showStatusIndicators) {
      label += getStatusIndicator(status);
    }
    if (opts.showDurations && nodeState?.durationMs !== undefined) {
      label += ` (${nodeState.durationMs}ms)`;
    }

    lines.push(`  ${safeId}${shape.open}"${label}"${shape.close}`);
  }
  lines.push('');

  // Groups (subgraphs)
  if (opts.grouping && opts.grouping.mode !== 'none') {
    const groups = computeNodeGroups(schema, opts.grouping).filter((g) => g.nodes.length >= 2);
    if (groups.length > 0) {
      const groupIdMap = buildUniqueIdMap(groups.map((g) => g.key));
      lines.push('  %% Groups');
      for (const group of groups) {
        const groupId = groupIdMap.get(group.key)!;
        lines.push(`  subgraph ${groupId}["${escapeMermaid(group.label)}"]`);
        for (const member of group.nodes) {
          const memberId = nodeIdMap.get(member.name);
          if (memberId) lines.push(`    ${memberId}`);
        }
        lines.push('  end');
        lines.push('');
      }
    }
  }

  // Edges
  lines.push('  %% Edges');
  for (const edge of schema.edges) {
    const fromId = nodeIdMap.get(edge.from) || baseSafeId(edge.from);
    const toId = nodeIdMap.get(edge.to) || baseSafeId(edge.to);
    const arrow = getArrowStyle(edge.edge_type);

    if (edge.label) {
      lines.push(`  ${fromId} ${arrow}|"${escapeMermaid(edge.label)}"|${toId}`);
    } else {
      lines.push(`  ${fromId} ${arrow} ${toId}`);
    }
  }
  lines.push('');

  // Apply status classes
  if (nodeStates && nodeStates.size > 0) {
    lines.push('  %% Apply status classes');
    const statusGroups: Record<string, string[]> = {
      pending: [],
      active: [],
      completed: [],
      error: [],
    };

    for (const node of schema.nodes) {
      const safeId = nodeIdMap.get(node.name)!;
      const nodeState = nodeStates.get(node.name);
      const status = nodeState?.status || 'pending';
      statusGroups[status].push(safeId);
    }

    for (const [status, nodes] of Object.entries(statusGroups)) {
      if (nodes.length > 0) {
        lines.push(`  class ${nodes.join(',')} ${status}`);
      }
    }
  }

  // Mark current node
  if (currentNode) {
    const safeId = nodeIdMap.get(currentNode) || baseSafeId(currentNode);
    lines.push(`  class ${safeId} current`);
  }

  return lines.join('\n');
}

/**
 * Render a GraphViewModel to Mermaid flowchart text
 */
export function renderViewModelToMermaid(
  viewModel: GraphViewModel | null | undefined,
  options: MermaidOptions = {},
): string | null {
  if (!viewModel || !viewModel.schema) {
    return null;
  }

  return renderSchemaToMermaid(
    viewModel.schema,
    viewModel.nodeStates,
    viewModel.currentNode,
    options,
  );
}

/**
 * Generate a minimal Mermaid diagram showing just the schema structure
 */
export function renderSchemaStructure(schema: GraphSchema): string {
  return renderSchemaToMermaid(schema, undefined, undefined, {
    showDurations: false,
    showStatusIndicators: false,
    includeStyleDefs: false,
  });
}

/**
 * Copy Mermaid text to clipboard
 */
export async function copyMermaidToClipboard(mermaidText: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(mermaidText);
    return true;
  } catch (e) {
    console.error('[mermaidRenderer] Failed to copy to clipboard:', e);
    return false;
  }
}

/**
 * Generate a download blob for Mermaid text
 */
export function createMermaidBlob(mermaidText: string): Blob {
  return new Blob([mermaidText], { type: 'text/plain' });
}

/**
 * Trigger download of Mermaid file
 */
export function downloadMermaid(mermaidText: string, filename: string = 'graph.mmd'): void {
  const blob = createMermaidBlob(mermaidText);
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
