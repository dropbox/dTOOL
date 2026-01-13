import { memo, useState, useCallback } from 'react';
import { Handle, Position } from '@xyflow/react';
import type { NodeStatus, NodeType } from '../types/graph';
import { DiffStatus } from '../types/schemaDiff';
import {
  borderWidthForDiffStatus,
  formatDurationText,
  getAnimationClass,
  getDurationPillStyle,
  getGraphNodeAriaLabel,
  getGraphNodeDiffPresentation,
  getNodeTypeInfo,
  getRingClass,
  getStatusBadgeStyles,
  getStatusIndicator,
  shouldShowActiveIndicator,
  shouldShowDuration,
} from './GraphNodeLogic';

export interface GraphNodeData {
  label: string;
  description?: string;
  nodeType: NodeType;
  status: NodeStatus;
  isActive: boolean;
  isSelected: boolean;
  // M-463: Keyboard focus state for accessibility
  isFocused?: boolean;
  duration?: number;
  icon: string;
  // M-472: Accessible label for the icon (screen reader text)
  iconAriaLabel?: string;
  bgColor: string;
  borderColor: string;
  inputFields: string[];
  outputFields: string[];
  // M-39: Diff status for schema comparison highlighting
  diffStatus?: DiffStatus;
  // TYP-07: Optional source code location
  sourceLocation?: string;
}

interface GraphNodeProps {
  data: GraphNodeData;
}

function GraphNodeComponent({ data }: GraphNodeProps) {
  const {
    label,
    description,
    nodeType, // TYP-08: Node type for tooltip
    status,
    isActive,
    isSelected,
    isFocused = false, // M-463: Keyboard focus state
    duration,
    icon,
    iconAriaLabel, // M-472: Accessible label for the icon
    bgColor,
    borderColor,
    diffStatus = 'unchanged',
    sourceLocation, // TYP-07: Source code location
  } = data;

  // TYP-07: State for showing copy feedback
  const [showCopied, setShowCopied] = useState(false);

  // TYP-07: Copy source location to clipboard
  const handleCopySource = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation(); // Don't trigger node selection
    if (!sourceLocation) return;

    try {
      await navigator.clipboard.writeText(sourceLocation);
      setShowCopied(true);
      setTimeout(() => setShowCopied(false), 1500);
    } catch (err) {
      console.error('Failed to copy source location:', err);
    }
  }, [sourceLocation]);

  // TYP-08: Get node type info
  const nodeTypeInfo = getNodeTypeInfo(nodeType);

  // TYP-06: Status indicator symbols
  const statusIndicator = getStatusIndicator(status);

  // TYP-06: Status badge styling - proper visual treatment
  const statusBadgeStyles = getStatusBadgeStyles(status);

  // Determine animation class based on status
  const animationClass = getAnimationClass(status, isActive);

  const showActiveIndicator = shouldShowActiveIndicator(status, isActive);

  // M-39: Get diff status styling
  const { diffStyle, effectiveBorderColor, effectiveBgColor } = getGraphNodeDiffPresentation(
    diffStatus,
    borderColor,
    bgColor,
  );

  return (
    <>
      <Handle type="target" position={Position.Top} className="w-3 h-3" />
      {/* Compact node design: 160px width for better graph layout */}
      {/* TYP-07: group class enables hover effects for source link icon */}
      <div
        className={`
          group
          px-3 py-2.5 rounded-lg shadow-md w-[160px]
          transition-all duration-200 cursor-pointer relative
          ${getRingClass(isSelected, isFocused)}
          ${animationClass}
          node-status-${status}
        `}
        // M-463: ARIA attributes for accessibility
        role="treeitem"
        aria-selected={isSelected}
        aria-label={getGraphNodeAriaLabel({ label, status, description, duration })}
        style={{
          background: effectiveBgColor,
          borderWidth: borderWidthForDiffStatus(diffStatus),
          borderStyle: diffStyle.borderStyle,
          borderColor: effectiveBorderColor,
        }}
      >
        {/* M-39: Diff status badge */}
        {diffStyle.badge && (
          <div
            className="absolute -top-2 -left-2 w-5 h-5 rounded-full flex items-center justify-center text-xs font-bold text-white z-10"
            style={{ backgroundColor: diffStyle.badgeColor }}
            title={`Node ${diffStatus}: ${diffStatus === 'added' ? 'not in expected schema' : diffStatus === 'removed' ? 'missing from observed' : diffStatus === 'modified' ? 'changed from expected' : 'not in declared schema'}`}
          >
            {diffStyle.badge}
          </div>
        )}
        {/* TYP-07: Source code link icon - visible on hover */}
        {sourceLocation && (
          <button
            onClick={handleCopySource}
            className="absolute top-1.5 right-1.5 opacity-0 group-hover:opacity-100 transition-opacity duration-200
                       p-1 rounded hover:bg-gray-600/50 text-gray-400 hover:text-gray-200"
            title={showCopied ? 'Copied!' : `Source: ${sourceLocation}`}
            aria-label={`Copy source location: ${sourceLocation}`}
          >
            {showCopied ? (
              <span className="text-green-400 text-xs">✓</span>
            ) : (
              <span className="text-xs font-mono">&lt;/&gt;</span>
            )}
          </button>
        )}
        {/* Compact header: icon + type badge + status */}
        <div className="flex items-center justify-between mb-1">
          <div className="flex items-center gap-1.5">
            <span className="text-sm" role="img" aria-label={iconAriaLabel}>{icon}</span>
            <span
              className="text-[9px] px-1 py-0.5 rounded bg-gray-700/60 text-gray-400 font-medium uppercase"
              title={nodeTypeInfo.description}
            >
              {nodeTypeInfo.badge}
            </span>
          </div>
          <div className="relative">
            <div className={`w-4 h-4 rounded-full border flex items-center justify-center ${statusBadgeStyles}`}>
              <span className="text-[10px]">{statusIndicator}</span>
            </div>
            {showActiveIndicator && (
              <div className="absolute inset-0 w-4 h-4 rounded-full bg-blue-500/30 status-ping" />
            )}
          </div>
        </div>

        {/* Node name - prominent */}
        <div className="font-semibold text-gray-100 text-sm truncate" title={label}>
          {label}
        </div>

        {/* Description - single line, subtle */}
        {description && (
          <div className="text-[10px] text-gray-500 mt-0.5 truncate" title={description}>
            {description}
          </div>
        )}

        {/* Duration - compact pill */}
        {shouldShowDuration(duration, status) && (
          <div className="flex justify-end mt-1">
            <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-mono ${getDurationPillStyle(status)}`}>
              {formatDurationText(duration, status)}
            </span>
          </div>
        )}

        {/* Active indicator animation - enhanced glow dot */}
        {showActiveIndicator && (
          <div className="absolute -top-1.5 -right-1.5">
            <div className="w-3 h-3 bg-blue-500 rounded-full" />
            <div className="absolute inset-0 w-3 h-3 bg-blue-500 rounded-full status-ping" />
          </div>
        )}
      </div>
      <Handle type="source" position={Position.Bottom} className="w-3 h-3" />
    </>
  );
}

export const GraphNode = memo(GraphNodeComponent);
export default GraphNode;
