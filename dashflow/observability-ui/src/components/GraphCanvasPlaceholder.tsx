// GN-14: Dark theme placeholder
import { colors, spacing, borderRadius, fontSize } from '../styles/tokens';

export function GraphCanvasPlaceholder() {
  return (
    <div
      style={{
        height: '100%',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: colors.bg.primary,
        borderRadius: borderRadius.lg,
        border: `2px dashed ${colors.border.primary}`,
      }}
    >
      <div style={{ textAlign: 'center' }}>
        <div style={{ fontSize: '2.5rem', marginBottom: spacing[2] }}>📊</div>
        <div style={{ color: colors.status.neutral }}>Waiting for graph execution...</div>
        <div style={{ fontSize: fontSize.md, color: colors.status.neutralDark, marginTop: spacing[1] }}>
          Run a demo app with node descriptions to see the graph
        </div>
      </div>
    </div>
  );
}

export default GraphCanvasPlaceholder;
