import type { NodeStatus, NodeType } from '../types/graph';
import { DiffStatus, DIFF_STATUS_STYLES } from '../types/schemaDiff';

export const NODE_TYPE_DESCRIPTIONS: Record<NodeType, { badge: string; description: string }> = {
  transform: { badge: 'Transform', description: 'Pure function that transforms state without external calls' },
  llm: { badge: 'LLM', description: 'Language model node that processes input through an AI model' },
  tool: { badge: 'Tool', description: 'External tool call (API, database, file system, etc.)' },
  router: { badge: 'Router', description: 'Conditional routing node that directs flow based on state' },
  aggregator: { badge: 'Aggregator', description: 'Collects and combines results from multiple nodes' },
  validator: { badge: 'Validator', description: 'Validates state against rules or constraints' },
  human_in_loop: { badge: 'Human', description: 'Pauses execution for human review or input' },
  checkpoint: { badge: 'Checkpoint', description: 'Persists state for recovery or branching' },
  custom: { badge: 'Custom', description: 'Custom node with user-defined behavior' },
};

export function getNodeTypeInfo(nodeType: NodeType): { badge: string; description: string } {
  return NODE_TYPE_DESCRIPTIONS[nodeType] || NODE_TYPE_DESCRIPTIONS.custom;
}

const STATUS_INDICATORS: Record<NodeStatus, string> = {
  pending: '○',
  active: '●',
  completed: '✓',
  error: '✗',
};

export function getStatusIndicator(status: NodeStatus): string {
  return STATUS_INDICATORS[status];
}

const STATUS_BADGE_STYLES: Record<NodeStatus, string> = {
  pending: 'bg-gray-500/20 border-gray-500/50 text-gray-400',
  active: 'bg-blue-500/20 border-blue-500/50 text-blue-400',
  completed: 'bg-green-500/20 border-green-500/50 text-green-400',
  error: 'bg-red-500/20 border-red-500/50 text-red-400',
};

export function getStatusBadgeStyles(status: NodeStatus): string {
  return STATUS_BADGE_STYLES[status];
}

export function getAnimationClass(status: NodeStatus, isActive: boolean): string {
  return status === 'active' || isActive ? 'node-running' : '';
}

export function getRingClass(isSelected: boolean, isFocused: boolean): string {
  if (isSelected) return 'ring-2 ring-blue-500 ring-offset-2 ring-offset-slate-900';
  if (isFocused && !isSelected) return 'ring-2 ring-purple-500 ring-offset-2 ring-offset-slate-900';
  return '';
}

export function getGraphNodeAriaLabel(args: {
  label: string;
  status: NodeStatus;
  description?: string;
  duration?: number;
}): string {
  const { label, status, description, duration } = args;
  let ariaLabel = `${label}, status: ${status}`;
  if (description) ariaLabel += `, ${description}`;
  if (duration !== undefined) ariaLabel += `, duration: ${duration}ms`;
  return ariaLabel;
}

export function shouldShowDuration(duration: number | undefined, status: NodeStatus): duration is number {
  return duration !== undefined && (status === 'completed' || status === 'active');
}

export function formatDurationText(duration: number, status: NodeStatus): string {
  return status === 'active' ? `${duration}ms...` : `${duration}ms`;
}

export function shouldShowActiveIndicator(status: NodeStatus, isActive: boolean): boolean {
  return status === 'active' || isActive;
}

export function borderWidthForDiffStatus(diffStatus: DiffStatus): number {
  return diffStatus !== 'unchanged' ? 3 : 2;
}

export function getDurationPillStyle(status: NodeStatus): string {
  if (status === 'active') {
    return 'bg-blue-500/20 text-blue-300 border border-blue-500/30';
  }
  return 'bg-black/30 text-gray-300 border border-gray-600/30';
}

export type GraphNodeDiffPresentation = {
  diffStyle: (typeof DIFF_STATUS_STYLES)[DiffStatus];
  effectiveBorderColor: string;
  effectiveBgColor: string;
};

export function getGraphNodeDiffPresentation(
  diffStatus: DiffStatus,
  borderColor: string,
  bgColor: string
): GraphNodeDiffPresentation {
  const diffStyle = DIFF_STATUS_STYLES[diffStatus];
  const effectiveBorderColor = diffStyle.borderColor || borderColor;
  const effectiveBgColor = diffStyle.bgOverlay
    ? `linear-gradient(${diffStyle.bgOverlay}, ${diffStyle.bgOverlay}), ${bgColor}`
    : bgColor;
  return { diffStyle, effectiveBorderColor, effectiveBgColor };
}
