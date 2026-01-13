// Mermaid text mode view for graph visualization
// Uses same viewModel as Canvas, just different rendering

import { useCallback, useMemo, useState } from 'react';
import { GraphViewModel } from '../hooks/useRunStateStore';
import {
  renderViewModelToMermaid,
  MermaidOptions,
  copyMermaidToClipboard,
  downloadMermaid,
} from '../utils/mermaidRenderer';
import { colors, spacing, borderRadius, fontSize } from '../styles/tokens';

interface MermaidViewProps {
  viewModel: GraphViewModel | null;
  options?: MermaidOptions;
}

export function MermaidView({ viewModel, options }: MermaidViewProps) {
  const [copyFeedback, setCopyFeedback] = useState<string | null>(null);

  // Generate Mermaid text from viewModel
  const mermaidText = useMemo(() => {
    if (!viewModel) return null;
    return renderViewModelToMermaid(viewModel, options);
  }, [viewModel, options]);

  // Handle copy to clipboard
  const handleCopy = useCallback(async () => {
    if (!mermaidText) return;

    const success = await copyMermaidToClipboard(mermaidText);
    if (success) {
      setCopyFeedback('Copied!');
      setTimeout(() => setCopyFeedback(null), 2000);
    } else {
      setCopyFeedback('Copy failed');
      setTimeout(() => setCopyFeedback(null), 2000);
    }
  }, [mermaidText]);

  // Handle download
  const handleDownload = useCallback(() => {
    if (!mermaidText) return;

    const filename = viewModel?.schema?.name
      ? `${viewModel.schema.name.replace(/\s+/g, '_')}.mmd`
      : 'graph.mmd';

    downloadMermaid(mermaidText, filename);
  }, [mermaidText, viewModel]);

  if (!viewModel) {
    return (
      <div style={{
        height: '100%',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: colors.bg.primary,
        borderRadius: borderRadius.lg,
        border: `1px solid ${colors.border.primary}`,
        color: colors.text.faint,
      }}>
        <div style={{ textAlign: 'center' }}>
          <div style={{ fontSize: '1.2rem', marginBottom: spacing[2] }}>Mermaid View</div>
          <div style={{ fontSize: '0.875rem' }}>No graph selected</div>
        </div>
      </div>
    );
  }

  if (!mermaidText) {
    return (
      <div style={{
        height: '100%',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: colors.bg.primary,
        borderRadius: borderRadius.lg,
        border: `1px solid ${colors.border.primary}`,
        color: colors.text.faint,
      }}>
        <div style={{ textAlign: 'center' }}>
          <div style={{ fontSize: '1.2rem', marginBottom: spacing[2] }}>Mermaid View</div>
          <div style={{ fontSize: '0.875rem' }}>No schema available</div>
        </div>
      </div>
    );
  }

  return (
    <div style={{
      height: '100%',
      display: 'flex',
      flexDirection: 'column',
      backgroundColor: colors.bg.primary,
      borderRadius: borderRadius.lg,
      border: `1px solid ${colors.border.primary}`,
      overflow: 'hidden',
    }}>
      {/* Header with controls */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: `${spacing[2]} ${spacing[3]}`,
        borderBottom: `1px solid ${colors.border.primary}`,
        backgroundColor: colors.bg.secondary,
      }}>
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: spacing[2],
        }}>
          <span style={{
            fontSize: fontSize.base,
            fontWeight: 600,
            color: colors.text.muted,
            textTransform: 'uppercase',
          }}>
            Mermaid Text Mode
          </span>
          {viewModel.schemaId && (
            <span style={{
              fontSize: fontSize.xs,
              color: colors.accent.cyan,
              fontFamily: 'monospace',
              backgroundColor: colors.bg.secondary,
              padding: `2px ${spacing[1]}`,
              borderRadius: borderRadius.md,
              border: `1px solid ${colors.border.primary}`,
            }}>
              {viewModel.schemaId.slice(0, 8)}
            </span>
          )}
        </div>

        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: spacing[2],
        }}>
          {/* Copy button */}
          <button
            type="button"
            onClick={handleCopy}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: spacing[1],
              backgroundColor: colors.bg.secondary,
              border: `1px solid ${colors.border.primary}`,
              borderRadius: borderRadius.md,
              color: copyFeedback === 'Copied!' ? colors.status.success : colors.text.muted,
              padding: `${spacing[1]} ${spacing[2]}`,
              fontSize: fontSize.sm,
              cursor: 'pointer',
            }}
            title="Copy Mermaid text to clipboard"
          >
            {copyFeedback || 'Copy'}
          </button>

          {/* Download button */}
          <button
            type="button"
            onClick={handleDownload}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: spacing[1],
              backgroundColor: colors.bg.secondary,
              border: `1px solid ${colors.border.primary}`,
              borderRadius: borderRadius.md,
              color: colors.text.muted,
              padding: `${spacing[1]} ${spacing[2]}`,
              fontSize: fontSize.sm,
              cursor: 'pointer',
            }}
            title="Download Mermaid file"
          >
            Download .mmd
          </button>
        </div>
      </div>

      {/* Mermaid text content */}
      <div style={{
        flex: 1,
        overflow: 'auto',
        padding: spacing[3],
      }}>
        <pre style={{
          margin: 0,
          fontFamily: "'SF Mono', 'Monaco', 'Inconsolata', monospace",
          fontSize: fontSize.base,
          lineHeight: 1.5,
          color: colors.text.primary,
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-word',
        }}>
          {mermaidText}
        </pre>
      </div>

      {/* Footer with info */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: `${spacing[1]} ${spacing[3]}`,
        borderTop: `1px solid ${colors.border.primary}`,
        backgroundColor: colors.bg.secondary,
        fontSize: fontSize.xs,
        color: colors.text.faint,
      }}>
        <span>
          {viewModel.schema?.nodes.length || 0} nodes, {viewModel.schema?.edges.length || 0} edges
        </span>
        <span>
          {viewModel.isLive ? 'LIVE' : `seq: ${viewModel.cursor.seq}`}
        </span>
      </div>
    </div>
  );
}

export default MermaidView;
