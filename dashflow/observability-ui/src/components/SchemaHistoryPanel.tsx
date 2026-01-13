// Schema History Panel - Track and compare graph schemas over time

import { useMemo, useState } from 'react';
import { GraphSchema, NodeSchema, EdgeSchema } from '../types/graph';
import { spacing, borderRadius, fontSize, colors } from '../styles/tokens';

// Schema observation record
export interface SchemaObservation {
  schemaId: string;
  graphName: string;
  schema: GraphSchema;
  firstSeen: number; // Unix timestamp ms
  lastSeen: number;
  runCount: number; // Number of runs using this schema
  threadIds: string[];
}

interface SchemaHistoryPanelProps {
  observations: SchemaObservation[];
  expectedSchemaId?: string;
  // M-113: Include graphName for per-graph baseline persistence
  onSetExpected?: (schemaId: string, graphName: string) => void;
  onCompare?: (schemaA: GraphSchema, schemaB: GraphSchema) => void;
}

// Format timestamp for display
function formatTimestamp(ts: number): string {
  const date = new Date(ts);
  return date.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

// Compare two schemas and return differences
function compareSchemas(a: GraphSchema, b: GraphSchema): SchemaDiff {
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

interface SchemaDiff {
  addedNodes: NodeSchema[];
  removedNodes: NodeSchema[];
  modifiedNodes: NodeSchema[];
  addedEdges: EdgeSchema[];
  removedEdges: EdgeSchema[];
  modifiedEdges: EdgeSchema[];
  hasChanges: boolean;
}

export function SchemaHistoryPanel({
  observations,
  expectedSchemaId,
  onSetExpected,
  onCompare: _onCompare,
}: SchemaHistoryPanelProps) {
  const [selectedForCompare, setSelectedForCompare] = useState<string[]>([]);
  const [showDiff, setShowDiff] = useState(false);

  // Sort observations by lastSeen (most recent first)
  const sortedObservations = useMemo(
    () => [...observations].sort((a, b) => b.lastSeen - a.lastSeen),
    [observations]
  );

  // Calculate diff if two schemas selected
  const diff = useMemo((): SchemaDiff | null => {
    if (selectedForCompare.length !== 2) return null;
    const [idA, idB] = selectedForCompare;
    const obsA = observations.find(o => o.schemaId === idA);
    const obsB = observations.find(o => o.schemaId === idB);
    if (!obsA || !obsB) return null;
    return compareSchemas(obsA.schema, obsB.schema);
  }, [selectedForCompare, observations]);

  const handleToggleSelect = (schemaId: string) => {
    setSelectedForCompare(prev => {
      if (prev.includes(schemaId)) {
        return prev.filter(id => id !== schemaId);
      }
      // Max 2 selections for comparison
      if (prev.length >= 2) {
        return [prev[1], schemaId];
      }
      return [...prev, schemaId];
    });
  };

  const handleKeyDown = (e: React.KeyboardEvent, schemaId: string) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleToggleSelect(schemaId);
    }
  };

  if (observations.length === 0) {
    return (
      <div style={{ padding: spacing[4], color: colors.status.neutral, textAlign: 'center' }}>
        No schemas observed yet. Run a graph to see schema history.
      </div>
    );
  }

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {/* Header - GN-14: Dark theme */}
      <div
        style={{
          padding: `${spacing[3]} ${spacing[4]}`,
          borderBottom: `1px solid ${colors.border.primary}`,
          background: colors.bg.primary,
        }}
      >
        <div style={{ fontWeight: 600, marginBottom: spacing[1], color: colors.text.primary }}>Schema History</div>
        <div style={{ fontSize: fontSize.base, color: colors.status.neutral }}>
          {observations.length} schema{observations.length !== 1 ? 's' : ''} observed
        </div>
      </div>

      {/* Compare controls - GN-14: Dark theme */}
      {selectedForCompare.length === 2 && (
        <div
          style={{
            padding: `${spacing[2]} ${spacing[4]}`,
            background: 'rgba(59, 130, 246, 0.15)',
            borderBottom: '1px solid rgba(59, 130, 246, 0.3)',
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
          }}
        >
          <span style={{ fontSize: '13px', color: colors.status.infoHover }}>
            2 schemas selected for comparison
          </span>
          <button
            type="button"
            onClick={() => setShowDiff(!showDiff)}
            style={{
              padding: `${spacing[1]} ${spacing[3]}`,
              fontSize: fontSize.base,
              background: colors.status.info,
              color: 'white',
              border: 'none',
              borderRadius: borderRadius.md,
              cursor: 'pointer',
            }}
          >
            {showDiff ? 'Hide Diff' : 'Show Diff'}
          </button>
        </div>
      )}

      {/* Diff view - GN-14: Dark theme */}
      {showDiff && diff && (
        <div
          style={{
            padding: `${spacing[3]} ${spacing[4]}`,
            background: 'rgba(250, 204, 21, 0.1)',
            borderBottom: '1px solid rgba(250, 204, 21, 0.3)',
            maxHeight: '200px',
            overflowY: 'auto',
          }}
        >
          <div style={{ fontWeight: 600, marginBottom: spacing[2], fontSize: '13px', color: colors.text.primary }}>
            Schema Differences
          </div>
          {!diff.hasChanges ? (
            <div style={{ color: colors.status.successLime, fontSize: fontSize.base }}>Schemas are identical</div>
          ) : (
            <div style={{ fontSize: fontSize.base }}>
              {diff.addedNodes.length > 0 && (
                <div style={{ color: colors.status.successDark, marginBottom: spacing[1] }}>
                  + {diff.addedNodes.length} node(s) added:{' '}
                  {diff.addedNodes.map(n => n.name).join(', ')}
                </div>
              )}
              {diff.removedNodes.length > 0 && (
                <div style={{ color: colors.status.errorDark, marginBottom: spacing[1] }}>
                  - {diff.removedNodes.length} node(s) removed:{' '}
                  {diff.removedNodes.map(n => n.name).join(', ')}
                </div>
              )}
              {diff.modifiedNodes.length > 0 && (
                <div style={{ color: colors.status.warningDark, marginBottom: spacing[1] }}>
                  ~ {diff.modifiedNodes.length} node(s) modified:{' '}
                  {diff.modifiedNodes.map(n => n.name).join(', ')}
                </div>
              )}
              {diff.addedEdges.length > 0 && (
                <div style={{ color: colors.status.successDark, marginBottom: spacing[1] }}>
                  + {diff.addedEdges.length} edge(s) added
                </div>
              )}
              {diff.removedEdges.length > 0 && (
                <div style={{ color: colors.status.errorDark, marginBottom: spacing[1] }}>
                  - {diff.removedEdges.length} edge(s) removed
                </div>
              )}
              {diff.modifiedEdges.length > 0 && (
                <div style={{ color: colors.status.warningDark, marginBottom: spacing[1] }}>
                  ~ {diff.modifiedEdges.length} edge(s) modified
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* Schema list */}
      <div style={{ flex: 1, overflowY: 'auto' }}>
        {sortedObservations.map(obs => {
          const isExpected = obs.schemaId === expectedSchemaId;
          const isSelected = selectedForCompare.includes(obs.schemaId);

          return (
            <div
              key={obs.schemaId}
              style={{
                padding: `${spacing[3]} ${spacing[4]}`,
                borderBottom: `1px solid ${colors.border.primary}`,
                background: isExpected ? 'rgba(34, 197, 94, 0.15)' : isSelected ? 'rgba(59, 130, 246, 0.15)' : colors.bg.secondary,
                cursor: 'pointer',
                outline: 'none',
              }}
              onClick={() => handleToggleSelect(obs.schemaId)}
              onKeyDown={(e) => handleKeyDown(e, obs.schemaId)}
              onFocus={(e) => { e.currentTarget.style.boxShadow = '0 0 0 2px rgba(59, 130, 246, 0.5) inset'; }}
              onBlur={(e) => { e.currentTarget.style.boxShadow = 'none'; }}
              tabIndex={0}
              role="button"
              aria-pressed={isSelected}
            >
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'flex-start',
                }}
              >
                <div>
                  <div
                    style={{
                      fontWeight: 500,
                      fontSize: fontSize.md,
                      display: 'flex',
                      alignItems: 'center',
                      gap: spacing[2],
                    }}
                  >
                    <span style={{ fontFamily: 'monospace', fontSize: fontSize.base, color: colors.status.neutral }}>
                      {obs.schemaId.slice(0, 8)}...
                    </span>
                    {isExpected && (
                      <span
                        style={{
                          fontSize: fontSize.xs,
                          padding: '2px 6px',
                          background: colors.status.success,
                          color: 'white',
                          borderRadius: borderRadius.md,
                        }}
                      >
                        EXPECTED
                      </span>
                    )}
                    {isSelected && (
                      <span
                        style={{
                          fontSize: fontSize.xs,
                          padding: '2px 6px',
                          background: colors.status.info,
                          color: 'white',
                          borderRadius: borderRadius.md,
                        }}
                      >
                        SELECTED
                      </span>
                    )}
                  </div>
                  <div style={{ fontSize: '13px', marginTop: spacing[1] }}>{obs.graphName}</div>
                </div>
                <div style={{ textAlign: 'right', fontSize: fontSize.base, color: colors.status.neutral }}>
                  <div>{obs.runCount} run{obs.runCount !== 1 ? 's' : ''}</div>
                  <div>{formatTimestamp(obs.lastSeen)}</div>
                </div>
              </div>
              <div
                style={{
                  display: 'flex',
                  gap: spacing[3],
                  marginTop: spacing[2],
                  fontSize: fontSize.base,
                  color: colors.status.neutral,
                }}
              >
                <span>{obs.schema.nodes.length} nodes</span>
                <span>{obs.schema.edges.length} edges</span>
                {obs.schema.version && <span>v{obs.schema.version}</span>}
              </div>
              {!isExpected && onSetExpected && (
                <button
                  type="button"
                  onClick={e => {
                    e.stopPropagation();
                    onSetExpected(obs.schemaId, obs.graphName);
                  }}
                  style={{
                    marginTop: spacing[2],
                    padding: `${spacing[1]} ${spacing[2]}`,
                    fontSize: fontSize.sm,
                    background: 'transparent',
                    border: `1px solid ${colors.border.primary}`,
                    borderRadius: borderRadius.md,
                    cursor: 'pointer',
                    color: colors.text.secondary,
                  }}
                >
                  Set as Expected
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
