// M-39: Schema diff types for expected vs observed graph comparison
// These types are used by SchemaHistoryPanel, GraphCanvas, and GraphNode

import { NodeSchema, EdgeSchema, GraphSchema } from './graph';

// Diff status for individual nodes/edges
export type DiffStatus = 'unchanged' | 'added' | 'removed' | 'modified' | 'out-of-schema';

// Schema diff result
export interface SchemaDiff {
  addedNodes: NodeSchema[];
  removedNodes: NodeSchema[];
  modifiedNodes: NodeSchema[];
  addedEdges: EdgeSchema[];
  removedEdges: EdgeSchema[];
  modifiedEdges: EdgeSchema[];
  hasChanges: boolean;
}

// Compare two schemas and return differences
export function compareSchemas(a: GraphSchema, b: GraphSchema): SchemaDiff {
  const addedNodes = b.nodes.filter(n => !a.nodes.some(an => an.name === n.name));
  const removedNodes = a.nodes.filter(n => !b.nodes.some(bn => bn.name === n.name));
  const modifiedNodes = b.nodes.filter(bn => {
    const an = a.nodes.find(n => n.name === bn.name);
    return an && JSON.stringify(an) !== JSON.stringify(bn);
  });

  const addedEdges = b.edges.filter(e => !a.edges.some(ae => ae.from === e.from && ae.to === e.to));
  const removedEdges = a.edges.filter(e => !b.edges.some(be => be.from === e.from && be.to === e.to));
  const modifiedEdges = b.edges.filter(be => {
    const ae = a.edges.find(e => e.from === be.from && e.to === be.to);
    return ae && JSON.stringify(ae) !== JSON.stringify(be);
  });

  return {
    addedNodes,
    removedNodes,
    modifiedNodes,
    addedEdges,
    removedEdges,
    modifiedEdges,
    hasChanges:
      addedNodes.length > 0 ||
      removedNodes.length > 0 ||
      modifiedNodes.length > 0 ||
      addedEdges.length > 0 ||
      removedEdges.length > 0 ||
      modifiedEdges.length > 0,
  };
}

// M-39: Diff status styling for visual schema comparison
export const DIFF_STATUS_STYLES: Record<DiffStatus, {
  borderColor: string;
  bgOverlay?: string;
  borderStyle: 'solid' | 'dashed' | 'dotted';
  badge?: string;
  badgeColor?: string;
}> = {
  unchanged: { borderColor: '', bgOverlay: undefined, borderStyle: 'solid' },
  added: { borderColor: '#22c55e', bgOverlay: 'rgba(34, 197, 94, 0.08)', borderStyle: 'solid', badge: '+', badgeColor: '#22c55e' },
  removed: { borderColor: '#ef4444', bgOverlay: 'rgba(239, 68, 68, 0.08)', borderStyle: 'dashed', badge: '-', badgeColor: '#ef4444' },
  modified: { borderColor: '#f59e0b', bgOverlay: 'rgba(245, 158, 11, 0.08)', borderStyle: 'solid', badge: '~', badgeColor: '#f59e0b' },
  'out-of-schema': { borderColor: '#dc2626', bgOverlay: 'rgba(220, 38, 38, 0.12)', borderStyle: 'dotted', badge: '!', badgeColor: '#dc2626' },
};

// Helper to compute diff status for a node
export function getNodeDiffStatus(
  nodeName: string,
  schemaDiff?: SchemaDiff,
  outOfSchemaNodes?: Set<string>
): DiffStatus {
  if (outOfSchemaNodes?.has(nodeName)) return 'out-of-schema';
  if (!schemaDiff) return 'unchanged';
  if (schemaDiff.addedNodes.some(n => n.name === nodeName)) return 'added';
  if (schemaDiff.removedNodes.some(n => n.name === nodeName)) return 'removed';
  if (schemaDiff.modifiedNodes.some(n => n.name === nodeName)) return 'modified';
  return 'unchanged';
}
