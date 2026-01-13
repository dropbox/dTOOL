import { memo } from 'react';
import { colors, spacing, borderRadius, fontSize } from '../styles/tokens';

export interface GroupNodeData {
  label: string;
  count: number;
  backgroundColor: string;
  borderColor: string;
}

interface GroupNodeProps {
  data: GroupNodeData;
}

function GroupNodeComponent({ data }: GroupNodeProps) {
  return (
    <div
      style={{
        width: '100%',
        height: '100%',
        borderRadius: 10,
        backgroundColor: data.backgroundColor,
        border: `1px solid ${data.borderColor}`,
        boxSizing: 'border-box',
        pointerEvents: 'none',
      }}
    >
      <div
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: spacing[2],
          margin: spacing[3],
          padding: `${spacing[1]} ${spacing[2]}`,
          borderRadius: borderRadius.lg,
          backgroundColor: 'rgba(21, 21, 37, 0.75)',
          border: '1px solid rgba(255, 255, 255, 0.08)',
          color: colors.text.primary,
          fontSize: fontSize.sm,
          fontWeight: 600,
          letterSpacing: 0.3,
          textTransform: 'uppercase',
        }}
      >
        <span>{data.label}</span>
        <span style={{ color: colors.text.secondary, fontWeight: 500 }}>({data.count})</span>
      </div>
    </div>
  );
}

export const GroupNode = memo(GroupNodeComponent);
export default GroupNode;

