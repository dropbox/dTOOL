import { GraphSchema, NodeSchema } from '../types/graph';

export type GroupingMode = 'none' | 'node_type' | 'attribute';

export interface GroupingOptions {
  mode: GroupingMode;
  attributeKey?: string;
}

export interface NodeGroup {
  key: string;
  label: string;
  nodes: NodeSchema[];
}

function normalizeAttributeKey(attributeKey: string | undefined): string {
  return (attributeKey || 'group').trim();
}

function nodeTypeLabel(nodeType: string): string {
  switch (nodeType) {
    case 'llm':
      return 'LLM';
    case 'human_in_loop':
      return 'Human-in-Loop';
    default:
      return nodeType.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
  }
}

export function computeNodeGroups(schema: GraphSchema, grouping: GroupingOptions): NodeGroup[] {
  if (grouping.mode === 'none') return [];

  const groups = new Map<string, NodeGroup>();
  const attributeKey = normalizeAttributeKey(grouping.attributeKey);

  for (const node of schema.nodes) {
    let key: string;
    let label: string;

    if (grouping.mode === 'node_type') {
      key = `node_type:${node.node_type}`;
      label = nodeTypeLabel(node.node_type);
    } else {
      const raw = node.attributes?.[attributeKey];
      const value = (raw ?? '').trim();
      key = `attr:${attributeKey}:${value || '(missing)'}`;
      label = value || '(missing)';
    }

    const existing = groups.get(key);
    if (existing) {
      existing.nodes.push(node);
    } else {
      groups.set(key, { key, label, nodes: [node] });
    }
  }

  const result = Array.from(groups.values());
  for (const group of result) {
    group.nodes.sort((a, b) => a.name.localeCompare(b.name));
  }

  result.sort((a, b) => a.label.localeCompare(b.label) || a.key.localeCompare(b.key));
  return result;
}

