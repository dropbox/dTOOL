import { NodeSchema, NodeExecution, NODE_TYPE_STYLES } from '../types/graph';
import { StateViewer } from './StateViewer';

interface NodeDetailsPanelProps {
  node: NodeSchema | null;
  execution?: NodeExecution;
  currentState?: Record<string, unknown>;
  previousState?: Record<string, unknown>;
}

export function NodeDetailsPanel({
  node,
  execution,
  currentState,
  previousState,
}: NodeDetailsPanelProps) {
  // V-08: Improved empty details panel with helpful info
  if (!node) {
    return (
      <div className="h-full flex items-center justify-center text-gray-300">
        <div className="text-center max-w-xs">
          <div className="text-4xl mb-3 opacity-60">🔍</div>
          <div className="text-lg font-medium text-gray-200 mb-2">Node Details</div>
          <div className="text-sm text-gray-400 mb-4">Click a node in the graph to view its details</div>
          <div className="text-xs text-gray-400 space-y-1">
            <div>⌨️ Use Tab/Arrow keys to navigate</div>
            <div>⏎ Press Enter to select</div>
          </div>
        </div>
      </div>
    );
  }

  const style = NODE_TYPE_STYLES[node.node_type] || NODE_TYPE_STYLES.transform;

  const statusBadges = {
    pending: { className: 'bg-gray-500/15 text-gray-200 ring-1 ring-inset ring-gray-500/30', label: 'Pending' },
    active: { className: 'bg-blue-500/20 text-blue-200 ring-1 ring-inset ring-blue-500/30', label: 'Running...' },
    completed: { className: 'bg-green-500/20 text-green-200 ring-1 ring-inset ring-green-500/30', label: 'Completed' },
    error: { className: 'bg-red-500/20 text-red-200 ring-1 ring-inset ring-red-500/30', label: 'Error' },
  } as const;

  const rawStatus = execution?.status;
  const status: keyof typeof statusBadges =
    rawStatus && rawStatus in statusBadges ? (rawStatus as keyof typeof statusBadges) : 'pending';

  const statusBadge = statusBadges[status];

  return (
    <div className="h-full overflow-y-auto">
      {/* Header */}
      <div className="p-4 border-b border-gray-700">
        <div className="flex items-center gap-2 mb-2">
          {/* M-472: Icon with role="img" and aria-label for screen readers */}
          <span className="text-2xl" role="img" aria-label={style.ariaLabel}>{style.icon}</span>
          <h3 className="text-lg font-semibold text-gray-100">{node.name}</h3>
        </div>
        {/* V-09: Improved badge styling with proper spacing and visual distinction */}
        <div className="flex items-center gap-3">
          <span
            className="px-2.5 py-1 rounded-md text-xs font-semibold uppercase tracking-wide ring-1 ring-inset ring-white/10"
            style={{ backgroundColor: style.bgColor, color: style.color }}
          >
            {node.node_type}
          </span>
          <span className={`px-2.5 py-1 rounded-md text-xs font-semibold ${statusBadge.className}`}>
            {statusBadge.label}
          </span>
        </div>
      </div>

      {/* I-01, I-03: Description with improved visual hierarchy */}
      {node.description && (
        <div className="p-4 border-b-2 border-gray-700">
          <h4 className="text-sm font-semibold text-gray-200 mb-2 uppercase tracking-wide">
            Description
          </h4>
          <p className="text-sm text-gray-300 leading-relaxed">{node.description}</p>
        </div>
      )}

      {/* I-01, I-02, I-03: Input/Output Fields with proper formatting and visual hierarchy */}
      <div className="p-4 border-b-2 border-gray-700">
        <div className="grid grid-cols-2 gap-6">
          <div>
            <h4 className="text-sm font-semibold text-gray-200 mb-3 uppercase tracking-wide flex items-center gap-2">
              <span className="text-blue-400">→</span> Inputs
            </h4>
            {node.input_fields.length > 0 ? (
              <ul className="space-y-1">
                {node.input_fields.map((field) => (
                  <li
                    key={field}
                    className="flex items-center gap-2 text-sm"
                  >
                    <span className="w-1.5 h-1.5 rounded-full bg-blue-400" />
                    <code className="px-2 py-0.5 bg-blue-500/15 text-blue-200 text-xs rounded font-mono ring-1 ring-inset ring-blue-500/20">
                      {field}
                    </code>
                  </li>
                ))}
              </ul>
            ) : (
              <span className="text-xs text-gray-400 italic">None specified</span>
            )}
          </div>
          <div>
            <h4 className="text-sm font-semibold text-gray-200 mb-3 uppercase tracking-wide flex items-center gap-2">
              <span className="text-green-400">←</span> Outputs
            </h4>
            {node.output_fields.length > 0 ? (
              <ul className="space-y-1">
                {node.output_fields.map((field) => (
                  <li
                    key={field}
                    className="flex items-center gap-2 text-sm"
                  >
                    <span className="w-1.5 h-1.5 rounded-full bg-green-400" />
                    <code className="px-2 py-0.5 bg-green-500/15 text-green-200 text-xs rounded font-mono ring-1 ring-inset ring-green-500/20">
                      {field}
                    </code>
                  </li>
                ))}
              </ul>
            ) : (
              <span className="text-xs text-gray-400 italic">None specified</span>
            )}
          </div>
        </div>
      </div>

      {/* Execution Metrics - V-10: Improved spacing and formatting */}
      {execution && (
        <div className="p-4 border-b border-gray-700">
          <h4 className="text-sm font-medium text-gray-300 mb-3">Execution</h4>
          <div className="space-y-2 text-sm">
            {execution.duration_ms !== undefined && (
              <div className="flex items-center justify-between">
                <span className="text-gray-400">Duration</span>
                <span className="font-mono font-medium text-gray-100">{execution.duration_ms}ms</span>
              </div>
            )}
            {execution.start_time && (
              <div className="flex items-center justify-between">
                <span className="text-gray-400">Started</span>
                <span className="font-mono text-gray-300">
                  {new Date(execution.start_time).toLocaleTimeString()}
                </span>
              </div>
            )}
            {execution.end_time && (
              <div className="flex items-center justify-between">
                <span className="text-gray-400">Ended</span>
                <span className="font-mono text-gray-300">
                  {new Date(execution.end_time).toLocaleTimeString()}
                </span>
              </div>
            )}
            {execution.error && (
              <div className="mt-2 p-2 bg-red-500/10 rounded border border-red-500/30">
                <span className="text-red-200 text-xs font-medium">Error: </span>
                <span className="text-red-200 text-xs">{execution.error}</span>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Current State */}
      {currentState && Object.keys(currentState).length > 0 && (
        <div className="p-4">
          <h4 className="text-sm font-medium text-gray-300 mb-2">State After Node</h4>
          <StateViewer
            state={currentState}
            previousState={previousState}
            highlightChanges={true}
          />
        </div>
      )}
    </div>
  );
}

export default NodeDetailsPanel;
